mod background;
mod browser;
mod config;
mod filter;
mod state;
mod window_monitor;

use chrono::Utc;
use clap::Parser;
use log::{debug, error, info, warn};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use tokio::signal::unix::{SignalKind, signal};
use tokio::time::{Duration, sleep};

use background::BackgroundManager;
use browser::BrowserManager;
use config::Config;
use filter::Filter;
use state::{AppPhase, AppState};
use window_monitor::WindowMonitor;

fn init_logging(log_level: &str, log_file: &str) -> anyhow::Result<()> {
    let level_filter = log_level.parse().unwrap_or(log::LevelFilter::Warn);

    if let Some(parent) = Path::new(log_file).parent() {
        fs::create_dir_all(parent)?;
    }

    let file = fern::log_file(log_file)?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(level_filter)
        .chain(std::io::stderr())
        .chain(file)
        .apply()?;

    Ok(())
}

/// Background wallpaper state used to avoid redundant feh calls.
#[derive(PartialEq)]
enum BgMode {
    Normal,
    Blocked,
    Break,
}

#[derive(Parser)]
#[command(
    name = "Inappropriate Video Handler",
    version,
    about = "Monitors window titles and manages browser access"
)]
struct Cli {
    /// Sets the config file to use
    #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
    config: String,

    /// Start browser with configured URL
    #[arg(long)]
    start_browser: bool,

    /// Log verbosity level
    #[arg(
        long,
        value_name = "LEVEL",
        value_parser = ["error", "warn", "info", "debug", "trace"],
        default_value = "info"
    )]
    log_level: String,
}

fn record_titles(titles_file: &str, titles: &[String]) -> anyhow::Result<()> {
    let mut existing: BTreeSet<String> = BTreeSet::new();

    if let Ok(content) = fs::read_to_string(titles_file) {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                existing.insert(trimmed.to_string());
            }
        }
    }

    let before = existing.len();
    existing.extend(titles.iter().cloned());

    if existing.len() != before {
        if let Some(parent) = Path::new(titles_file).parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(titles_file)?;
        for title in &existing {
            writeln!(file, "{}", title)?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config = match Config::load(&cli.config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load config ({}), using defaults", e);
            Config::default()
        }
    };

    if let Err(e) = init_logging(&cli.log_level, &config.files.log_file) {
        eprintln!("Failed to initialise logger: {}", e);
    }

    info!("Config loaded from '{}'", cli.config);
    debug!("  browser.executable = '{}'", config.browser.executable);
    debug!("  browser.process_name = '{}'", config.browser.process_name);
    debug!("  browser.url = '{}'", config.browser.url);
    debug!(
        "  monitoring.check_frequency_seconds = {}",
        config.monitoring.check_frequency_seconds
    );
    debug!(
        "  timeouts.blacklist_timeout_minutes = {}",
        config.timeouts.blacklist_timeout_minutes
    );
    debug!(
        "  timeouts.grace_retries = {}",
        config.timeouts.grace_retries
    );
    debug!(
        "  timeouts.hard_lock_minutes = {}",
        config.timeouts.hard_lock_minutes
    );
    debug!(
        "  timeouts.retry_reset_minutes = {}",
        config.timeouts.retry_reset_minutes
    );
    debug!(
        "  timeouts.bathroom_break_minutes = {}",
        config.timeouts.bathroom_break_minutes
    );
    debug!(
        "  timeouts.bathroom_break_interval_hours = {}",
        config.timeouts.bathroom_break_interval_hours
    );
    debug!("  files.blacklist = '{}'", config.files.blacklist);
    debug!("  files.whitelist = '{}'", config.files.whitelist);
    debug!("  files.state_file = '{}'", config.files.state_file);
    debug!("  files.log_file = '{}'", config.files.log_file);
    debug!("  files.titles_file = '{}'", config.files.titles_file);

    info!("Mode: start_browser={}", cli.start_browser);

    if cli.start_browser {
        if let Err(e) = handle_start_browser(&config).await {
            error!("Error starting browser: {}", e);
        }
    } else if let Err(e) = run_daemon(&config).await {
        error!("Error running daemon: {}", e);
    }
}

async fn handle_start_browser(config: &Config) -> anyhow::Result<()> {
    info!("Loading state from '{}'", config.files.state_file);
    let mut state = AppState::load(&config.files.state_file)?;

    info!(
        "State loaded: blocked={} in_bathroom_break={} violation_count={}",
        state.is_blocked(),
        state.is_in_bathroom_break(),
        state.violation_count
    );
    debug!("  next_bathroom_break = {}", state.next_bathroom_break);
    if let Some(until) = state.blocked_until() {
        debug!("  blocked_until = {}", until);
    }
    if let Some(until) = state.bathroom_break_until() {
        debug!("  bathroom_break_until = {}", until);
    }

    let bg = BackgroundManager::new();
    let browser_manager = BrowserManager::new(
        config.browser.executable.clone(),
        config.browser.process_name.clone(),
    );

    if state.is_blocked() {
        println!("Browser is currently blocked");
        info!("Browser blocked until {:?}", state.blocked_until());
        browser_manager.kill_browser_processes().await?;
        bg.set_blocked_background(&config.backgrounds.blocked)?;
        return Ok(());
    }

    if state.is_bathroom_break_time() {
        if !matches!(state.phase, AppPhase::BathroomBreak { .. }) {
            info!(
                "Starting bathroom break: duration={}m interval={}h",
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours
            );
            browser_manager.kill_browser_processes().await?;
            state.start_bathroom_break(
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours,
            );
            state.save(&config.files.state_file)?;
        }

        if let AppPhase::BathroomBreak { until } = state.phase {
            if Utc::now() < until {
                println!("It's bathroom break time");
                info!("Bathroom break active until {}", until);
                browser_manager.kill_browser_processes().await?;
                bg.set_bathroom_break_background(&config.backgrounds.bathroom_break)?;
                return Ok(());
            } else {
                info!("Bathroom break expired, ending break");
                state.end_bathroom_break();
                state.save(&config.files.state_file)?;
            }
        }
    }

    bg.set_normal_background(&config.backgrounds.normal)?;

    // Reset the violation counter only if the configured quiet window has elapsed
    // since the last violation — not unconditionally on every browser start.
    if state.violation_count > 0 {
        let quiet_elapsed = state.violation_window_start.is_none_or(|last| {
            (Utc::now() - last).num_minutes() >= config.timeouts.retry_reset_minutes as i64
        });
        if quiet_elapsed {
            info!(
                "Resetting violation count ({} -> 0) after {} minutes quiet",
                state.violation_count, config.timeouts.retry_reset_minutes
            );
            state.violation_count = 0;
            state.violation_window_start = None;
            state.save(&config.files.state_file)?;
        }
    }

    info!(
        "Starting browser: executable='{}' url='{}'",
        config.browser.executable, config.browser.url
    );

    match browser_manager.start_browser(&config.browser.url) {
        Ok(_) => {
            println!("Browser started successfully");
            info!("Browser started successfully");
        }
        Err(e) => error!("Failed to start browser: {}", e),
    }

    Ok(())
}

async fn run_daemon(config: &Config) -> anyhow::Result<()> {
    info!("Initialising window monitor");
    let window_monitor = WindowMonitor::new()?;

    info!(
        "Loading filter patterns from '{}' (blacklist) and '{}' (whitelist)",
        config.files.blacklist, config.files.whitelist
    );
    let filter = Filter::new(&config.files.blacklist, &config.files.whitelist)?;

    info!(
        "Filter loaded: {} blacklist pattern(s), {} whitelist pattern(s)",
        filter.blacklist_len(),
        filter.whitelist_len()
    );

    let browser_manager = BrowserManager::new(
        config.browser.executable.clone(),
        config.browser.process_name.clone(),
    );

    let mut sigterm = signal(SignalKind::terminate())?;
    let bg = BackgroundManager::new();

    println!("Starting daemon mode...");
    info!("Daemon started");

    let mut current_bg: Option<BgMode> = None;
    // Cache of the last state we read, keyed by the state file's mtime, so an
    // unchanged file doesn't get re-read and re-parsed on every tick. Anything
    // external (e.g. a `--start-browser` invocation) that writes the file
    // changes its mtime and is picked up on the next tick regardless.
    let mut cached: Option<(AppState, std::time::SystemTime)> = None;

    loop {
        debug!("--- daemon tick ---");
        let mut state =
            match AppState::load_if_changed(&config.files.state_file, cached.map(|(_, m)| m))? {
                Some((state, _)) => state,
                None => {
                    cached
                        .expect("cache is populated whenever load_if_changed can return None")
                        .0
                }
            };
        let mut state_dirty = false;

        debug!(
            "State: blocked={} in_bathroom_break={} violation_count={} next_break={}",
            state.is_blocked(),
            state.is_in_bathroom_break(),
            state.violation_count,
            state.next_bathroom_break
        );

        // End any expired bathroom break first so the rest of the loop sees clean state.
        if let AppPhase::BathroomBreak { until } = state.phase
            && Utc::now() >= until
        {
            info!("Bathroom break expired at {}", until);
            println!("Bathroom break ended");
            state.end_bathroom_break();
            state_dirty = true;
        }

        // Check window titles for violations — skipped during a bathroom break since
        // the browser should not be running and the user should be away from the screen.
        if !state.is_in_bathroom_break() {
            let browser_pids = browser_manager.get_pids();
            if let Ok(titles) = window_monitor.get_browser_window_titles(&browser_pids) {
                if !titles.is_empty() {
                    info!(
                        "Checking {} browser window title(s) against filter",
                        titles.len()
                    );
                    if let Err(e) = record_titles(&config.files.titles_file, &titles) {
                        warn!("Failed to record window titles: {}", e);
                    }
                }

                if let Some((matched_title, matched_pattern)) =
                    filter.find_blacklisted_title(&titles)
                {
                    warn!(
                        "Blacklist hit: title='{}' matched pattern='{}'",
                        matched_title, matched_pattern
                    );
                    browser_manager.kill_browser_processes().await?;

                    let hard_locked = state.record_violation(
                        config.timeouts.grace_retries,
                        config.timeouts.retry_reset_minutes,
                        config.timeouts.blacklist_timeout_minutes,
                        config.timeouts.hard_lock_minutes,
                    );

                    if hard_locked {
                        warn!(
                            "Grace retries exhausted ({} violations) — hard locking for {} minutes",
                            state.violation_count, config.timeouts.hard_lock_minutes
                        );
                        println!(
                            "Blacklisted content detected — grace retries exhausted, hard locking for {} minutes",
                            config.timeouts.hard_lock_minutes
                        );
                    } else {
                        warn!(
                            "Grace retry {}/{} — browser killed, blocking for {} minute(s)",
                            state.violation_count,
                            config.timeouts.grace_retries,
                            config.timeouts.blacklist_timeout_minutes
                        );
                        println!(
                            "Blacklisted content detected — grace retry {}/{}, browser killed",
                            state.violation_count, config.timeouts.grace_retries
                        );
                    }
                    state_dirty = true;
                }
            }

            // Proactively reset the violation counter once enough quiet time has passed.
            // This handles the case where no further violations occur after the reset window.
            if !state.is_blocked()
                && let Some(last) = state.violation_window_start
                && (Utc::now() - last).num_minutes() >= config.timeouts.retry_reset_minutes as i64
                && state.violation_count > 0
            {
                info!(
                    "Resetting violation count ({} -> 0) after {} minutes quiet",
                    state.violation_count, config.timeouts.retry_reset_minutes
                );
                state.violation_count = 0;
                state.violation_window_start = None;
                state_dirty = true;
            }
        }

        // Initiate a scheduled bathroom break — only when the browser is not already
        // in a cooldown block, to keep the two states distinct.
        if !state.is_blocked()
            && !state.is_in_bathroom_break()
            && Utc::now() >= state.next_bathroom_break
        {
            info!(
                "Bathroom break: duration={}m next_interval={}h",
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours
            );
            println!("Initiating bathroom break");
            browser_manager.kill_browser_processes().await?;
            state.start_bathroom_break(
                config.timeouts.bathroom_break_minutes,
                config.timeouts.bathroom_break_interval_hours,
            );
            state_dirty = true;
        }

        if state_dirty {
            state.save(&config.files.state_file)?;
        }

        // Remember what's now on disk (whichever tick wrote it) so an unchanged
        // file can be skipped on the next iteration instead of re-read.
        cached = fs::metadata(&config.files.state_file)
            .and_then(|m| m.modified())
            .ok()
            .map(|mtime| (state, mtime));

        // Sync background to current state; only call feh when the mode actually changes.
        let new_bg = if state.is_in_bathroom_break() {
            BgMode::Break
        } else if state.is_blocked() {
            BgMode::Blocked
        } else {
            BgMode::Normal
        };
        if current_bg.as_ref() != Some(&new_bg) {
            match new_bg {
                BgMode::Normal => bg.set_normal_background(&config.backgrounds.normal)?,
                BgMode::Blocked => bg.set_blocked_background(&config.backgrounds.blocked)?,
                BgMode::Break => {
                    bg.set_bathroom_break_background(&config.backgrounds.bathroom_break)?
                }
            }
            current_bg = Some(new_bg);
        }

        debug!(
            "Sleeping {} second(s) until next check",
            config.monitoring.check_frequency_seconds
        );
        tokio::select! {
            _ = sleep(Duration::from_secs(config.monitoring.check_frequency_seconds)) => {}
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down");
                info!("Daemon stopped via SIGTERM");
                return Ok(());
            }
        }
    }
}
