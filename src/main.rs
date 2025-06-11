mod config;
mod state;
mod window_monitor;
mod filter;
mod browser;
mod background;

use clap::{Arg, Command};
use tokio::time::{sleep, Duration};
use chrono::Utc;
use std::sync::Arc;

use config::Config;
use state::AppState;
use window_monitor::WindowMonitor;
use filter::Filter;
use browser::BrowserManager;
use background::BackgroundManager;

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
            Arg::new("daemon")
                .short('d')
                .long("daemon")
                .help("Run in daemon mode (monitor windows)")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config").unwrap();
    let config = match Config::load(config_path) {
        Ok(config) => config,
        Err(_) => {
            eprintln!("Failed to load config, using defaults");
            Config::default()
        }
    };

    let start_browser = matches.get_flag("start-browser");
    let daemon_mode = matches.get_flag("daemon");

    if start_browser {
        if let Err(e) = handle_start_browser(&config).await {
            eprintln!("Error starting browser: {}", e);
        }
    } else if daemon_mode {
        if let Err(e) = run_daemon(&config).await {
            eprintln!("Error running daemon: {}", e);
        }
    } else {
        eprintln!("Use --start-browser to start browser or --daemon to monitor windows");
    }
}

async fn handle_start_browser(config: &Config) -> anyhow::Result<()> {
    let mut state = AppState::load(&config.files.state_file)?;

    if state.is_blocked() {
        println!("Browser is currently blocked");
        BackgroundManager::set_blocked_background(&config.backgrounds.blocked)?;
        return Ok(());
    }

    if state.is_bathroom_break_time(config.timeouts.bathroom_break_interval_hours) {
        if !state.in_bathroom_break {
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
                    BackgroundManager::set_bathroom_break_background(&config.backgrounds.bathroom_break)?;
                    return Ok(());
                } else {
                    state.end_bathroom_break();
                    state.save(&config.files.state_file)?;
                }
            }
        }
    }

    BackgroundManager::set_normal_background(&config.backgrounds.normal)?;
    
    let browser_manager = BrowserManager::new(
        config.browser.executable.clone(),
        config.browser.process_name.clone(),
    );

    match browser_manager.start_browser(&config.browser.url) {
        Ok(_) => println!("Browser started successfully"),
        Err(e) => eprintln!("Failed to start browser: {}", e),
    }

    Ok(())
}

async fn run_daemon(config: &Config) -> anyhow::Result<()> {
    let window_monitor = Arc::new(WindowMonitor::new()?);
    let filter = Arc::new(Filter::new(&config.files.blacklist, &config.files.whitelist)?);
    let browser_manager = Arc::new(BrowserManager::new(
        config.browser.executable.clone(),
        config.browser.process_name.clone(),
    ));

    println!("Starting daemon mode...");

    loop {
        let mut state = AppState::load(&config.files.state_file)?;

        if let Ok(titles) = window_monitor.get_all_window_titles() {
            if filter.check_titles(&titles) {
                println!("Blacklisted content detected, killing browser");
                browser_manager.kill_browser_processes()?;
                state.block_browser(config.timeouts.blacklist_timeout_minutes);
                state.save(&config.files.state_file)?;
                BackgroundManager::set_blocked_background(&config.backgrounds.blocked)?;
            }
        }

        if state.is_bathroom_break_time(config.timeouts.bathroom_break_interval_hours) && !state.in_bathroom_break {
            println!("Initiating bathroom break");
            browser_manager.kill_browser_processes()?;
            state.start_bathroom_break(
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours,
            );
            state.save(&config.files.state_file)?;
            BackgroundManager::set_bathroom_break_background(&config.backgrounds.bathroom_break)?;
        }

        if state.in_bathroom_break {
            if let Some(until) = state.bathroom_break_until {
                if Utc::now() >= until {
                    println!("Bathroom break ended");
                    state.end_bathroom_break();
                    state.save(&config.files.state_file)?;
                }
            }
        }

        sleep(Duration::from_secs(config.monitoring.check_frequency_seconds)).await;
    }
}