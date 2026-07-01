mod background;
mod browser;
mod config;
mod filter;
mod state;
mod window_monitor;

use chrono::Utc;
use clap::{Arg, Command};
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, Duration};

use background::BackgroundManager;
use browser::BrowserManager;
use config::Config;
use filter::Filter;
use state::AppState;
use window_monitor::WindowMonitor;

#[tokio::main]
async fn main() {
    let matches = Command::new("Inappropriate Video Handler")
        .version("1.0")
        .author("Your Name")
        .about("Monitors window titles and manages browser access")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets the config file to use")
                .default_value("config.yaml"),
        )
        .arg(
            Arg::new("start-browser")
                .long("start-browser")
                .help("Start browser with configured URL")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("log-level")
                .long("log-level")
                .value_name("LEVEL")
                .help("Log verbosity level")
                .value_parser(["error", "warn", "info", "debug", "trace"])
                .default_value("warn"),
        )
        .get_matches();

    let log_level = matches.get_one::<String>("log-level").map(String::as_str).unwrap_or("warn");
    env_logger::Builder::new()
        .filter_level(log_level.parse().unwrap_or(log::LevelFilter::Warn))
        .init();

    let config_path = matches.get_one::<String>("config").unwrap();
    info!("Loading config from '{}'", config_path);
    let config = match Config::load(config_path) {
        Ok(config) => {
            info!("Config loaded successfully");
            debug!("  browser.executable = '{}'", config.browser.executable);
            debug!("  browser.process_name = '{}'", config.browser.process_name);
            debug!("  browser.url = '{}'", config.browser.url);
            debug!("  monitoring.check_frequency_seconds = {}", config.monitoring.check_frequency_seconds);
            debug!("  timeouts.blacklist_timeout_minutes = {}", config.timeouts.blacklist_timeout_minutes);
            debug!("  timeouts.bathroom_break_minutes = {}", config.timeouts.bathroom_break_minutes);
            debug!("  timeouts.bathroom_break_interval_hours = {}", config.timeouts.bathroom_break_interval_hours);
            debug!("  files.blacklist = '{}'", config.files.blacklist);
            debug!("  files.whitelist = '{}'", config.files.whitelist);
            debug!("  files.state_file = '{}'", config.files.state_file);
            config
        }
        Err(e) => {
            warn!("Failed to load config ({}), using defaults", e);
            Config::default()
        }
    };

    let start_browser = matches.get_flag("start-browser");
    info!("Mode: start_browser={}", start_browser);

    if start_browser {
        if let Err(e) = handle_start_browser(&config).await {
            error!("Error starting browser: {}", e);
        }
    } else {
        if let Err(e) = run_daemon(&config).await {
            error!("Error running daemon: {}", e);
        }
    }
}

async fn handle_start_browser(config: &Config) -> anyhow::Result<()> {
    info!("Loading state from '{}'", config.files.state_file);
    let mut state = AppState::load(&config.files.state_file)?;

    info!("State loaded: blocked={} in_bathroom_break={}", state.is_blocked(), state.in_bathroom_break);
    debug!("  next_bathroom_break = {}", state.next_bathroom_break);
    if let Some(until) = state.blocked_until {
        debug!("  blocked_until = {}", until);
    }
    if let Some(until) = state.bathroom_break_until {
        debug!("  bathroom_break_until = {}", until);
    }

    if state.is_blocked() {
        println!("Browser is currently blocked");
        info!("Browser blocked until {:?}", state.blocked_until);
        let bg = BackgroundManager::new();
        bg.set_blocked_background(&config.backgrounds.blocked)?;
        return Ok(());
    }

    let bg = BackgroundManager::new();

    if state.is_bathroom_break_time(config.timeouts.bathroom_break_interval_hours) {
        if !state.in_bathroom_break {
            info!("Starting bathroom break: duration={}m interval={}h",
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours);
            state.start_bathroom_break(
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours,
            );
            state.save(&config.files.state_file)?;
        }

        if state.in_bathroom_break {
            if let Some(until) = state.bathroom_break_until {
                if Utc::now() < until {
                    println!("It's bathroom break time");
                    info!("Bathroom break active until {}", until);
                    bg.set_bathroom_break_background(&config.backgrounds.bathroom_break)?;
                    return Ok(());
                } else {
                    info!("Bathroom break expired, ending break");
                    state.end_bathroom_break();
                    state.save(&config.files.state_file)?;
                }
            }
        }
    }

    bg.set_normal_background(&config.backgrounds.normal)?;

    let browser_manager = BrowserManager::new(
        config.browser.executable.clone(),
        config.browser.process_name.clone(),
    );

    match browser_manager.start_browser(&config.browser.url) {
        Ok(_) => println!("Browser started successfully"),
        Err(e) => error!("Failed to start browser: {}", e),
    }

    Ok(())
}

async fn run_daemon(config: &Config) -> anyhow::Result<()> {
    info!("Initialising window monitor");
    let window_monitor = Arc::new(WindowMonitor::new()?);

    info!("Loading filter patterns from '{}' (blacklist) and '{}' (whitelist)",
        config.files.blacklist, config.files.whitelist);
    let filter = Arc::new(Filter::new(
        &config.files.blacklist,
        &config.files.whitelist,
    )?);

    info!("Filter loaded: {} blacklist pattern(s), {} whitelist pattern(s)",
        filter.blacklist_len(), filter.whitelist_len());

    let browser_manager = Arc::new(BrowserManager::new(
        config.browser.executable.clone(),
        config.browser.process_name.clone(),
    ));

    let mut sigterm = signal(SignalKind::terminate())?;

    println!("Starting daemon mode...");

    loop {
        debug!("--- daemon tick ---");
        let mut state = AppState::load(&config.files.state_file)?;

        debug!("State: blocked={} in_bathroom_break={} next_break={}",
            state.is_blocked(), state.in_bathroom_break, state.next_bathroom_break);

        let browser_pids = browser_manager.get_pids();
        if let Ok(titles) = window_monitor.get_browser_window_titles(&browser_pids) {
            info!("Checking {} browser window title(s) against filter", titles.len());
            if let Some((matched_title, matched_pattern)) = filter.find_blacklisted_title(&titles) {
                println!("Blacklisted content detected, killing browser");
                info!("Blacklist hit: title='{}' matched pattern='{}'",
                    matched_title, matched_pattern);
                browser_manager.kill_browser_processes()?;
                state.block_browser(config.timeouts.blacklist_timeout_minutes);
                info!("Browser blocked for {} minute(s)", config.timeouts.blacklist_timeout_minutes);
                state.save(&config.files.state_file)?;
                let bg = BackgroundManager::new();
                bg.set_blocked_background(&config.backgrounds.blocked)?;
            }
        }

        if state.is_bathroom_break_time(config.timeouts.bathroom_break_interval_hours)
            && !state.in_bathroom_break
        {
            println!("Initiating bathroom break");
            info!("Bathroom break: duration={}m next_interval={}h",
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours);
            browser_manager.kill_browser_processes()?;
            state.start_bathroom_break(
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours,
            );
            state.save(&config.files.state_file)?;
            let bg = BackgroundManager::new();
            bg.set_bathroom_break_background(&config.backgrounds.bathroom_break)?;
        }

        if state.in_bathroom_break {
            if let Some(until) = state.bathroom_break_until {
                if Utc::now() >= until {
                    println!("Bathroom break ended");
                    info!("Bathroom break expired at {}", until);
                    state.end_bathroom_break();
                    state.save(&config.files.state_file)?;
                }
            }
        }

        debug!("Sleeping {} second(s) until next check", config.monitoring.check_frequency_seconds);
        tokio::select! {
            _ = sleep(Duration::from_secs(config.monitoring.check_frequency_seconds)) => {}
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down");
                return Ok(());
            }
        }
    }
}
