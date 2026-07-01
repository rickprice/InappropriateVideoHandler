// Integration tests that don't require X11 or display environment
use inappropriate_video_handler::background::BackgroundManager;
use inappropriate_video_handler::browser::BrowserManager;
use inappropriate_video_handler::config::{
    BackgroundConfig, BrowserConfig, Config, FileConfig, MonitoringConfig, TimeoutConfig,
};
use inappropriate_video_handler::filter::Filter;
use inappropriate_video_handler::state::AppState;

use chrono::{Duration, Utc};
use serial_test::serial;
use std::fs;
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

fn create_test_config() -> Config {
    Config {
        browser: BrowserConfig {
            executable: "echo".to_string(),
            url: "https://test.com".to_string(),
            process_name: "test-process".to_string(),
        },
        monitoring: MonitoringConfig {
            check_frequency_seconds: 1,
        },
        timeouts: TimeoutConfig {
            blacklist_timeout_minutes: 5,
            bathroom_break_minutes: 2,
            bathroom_break_interval_hours: 1,
            grace_retries: 3,
            hard_lock_minutes: 40,
            retry_reset_minutes: 20,
        },
        backgrounds: BackgroundConfig {
            normal: "/tmp/test_normal.jpg".to_string(),
            blocked: "/tmp/test_blocked.jpg".to_string(),
            bathroom_break: "/tmp/test_break.jpg".to_string(),
        },
        files: FileConfig {
            blacklist: "test_blacklist.txt".to_string(),
            whitelist: "test_whitelist.txt".to_string(),
            state_file: "/tmp/ivh_test/state.json".to_string(),
            log_file: "/tmp/ivh_test/ivh.log".to_string(),
            titles_file: "/tmp/ivh_test/window-titles.txt".to_string(),
        },
    }
}

fn create_temp_filter_files() -> (NamedTempFile, NamedTempFile) {
    let mut blacklist_file = NamedTempFile::new().unwrap();
    blacklist_file
        .write_all(b".*porn.*\n.*adult.*\n.*xxx.*")
        .unwrap();

    let mut whitelist_file = NamedTempFile::new().unwrap();
    whitelist_file
        .write_all(b".*education.*\n.*medical.*")
        .unwrap();

    (blacklist_file, whitelist_file)
}

#[test]
fn test_config_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");

    let original_config = create_test_config();

    let yaml_content = serde_yaml::to_string(&original_config).unwrap();
    fs::write(&config_path, yaml_content).unwrap();

    let loaded_config = Config::load(&config_path).unwrap();

    assert_eq!(
        loaded_config.browser.executable,
        original_config.browser.executable
    );
    assert_eq!(loaded_config.browser.url, original_config.browser.url);
    assert_eq!(
        loaded_config.monitoring.check_frequency_seconds,
        original_config.monitoring.check_frequency_seconds
    );
    assert_eq!(
        loaded_config.timeouts.blacklist_timeout_minutes,
        original_config.timeouts.blacklist_timeout_minutes
    );
}

#[test]
fn test_state_persistence_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("test_state.json");

    let mut state = AppState::default();
    assert!(!state.is_blocked());
    assert!(!state.in_bathroom_break);

    state.block_browser(10);
    assert!(state.is_blocked());
    state.save(&state_file).unwrap();

    let loaded_state = AppState::load(&state_file).unwrap();
    assert!(loaded_state.is_blocked());

    let mut state = loaded_state;
    state.start_bathroom_break(5, 2);
    assert!(state.in_bathroom_break);
    state.save(&state_file).unwrap();

    let final_state = AppState::load(&state_file).unwrap();
    assert!(final_state.in_bathroom_break);
}

#[test]
fn test_filter_integration() {
    let (blacklist_file, whitelist_file) = create_temp_filter_files();

    let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

    assert!(filter.is_blacklisted("free porn videos"));
    assert!(filter.is_blacklisted("adult content"));
    assert!(filter.is_blacklisted("xxx movies"));

    assert!(!filter.is_blacklisted("sex education documentary"));
    assert!(!filter.is_blacklisted("medical adult content"));

    assert!(!filter.is_blacklisted("cooking tutorial"));

    let titles = vec![
        "cooking tutorial".to_string(),
        "news update".to_string(),
        "free porn videos".to_string(),
    ];
    assert!(filter.check_titles(&titles));

    let clean_titles = vec![
        "cooking tutorial".to_string(),
        "news update".to_string(),
        "educational video".to_string(),
    ];
    assert!(!filter.check_titles(&clean_titles));
}

#[test]
fn test_filter_find_blacklisted_title_integration() {
    let (blacklist_file, whitelist_file) = create_temp_filter_files();
    let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

    let titles = vec![
        "cooking tutorial".to_string(),
        "free porn videos".to_string(),
        "news update".to_string(),
    ];

    let result = filter.find_blacklisted_title(&titles);
    assert!(result.is_some());
    let (title, pattern) = result.unwrap();
    assert_eq!(title, "free porn videos");
    assert_eq!(pattern, ".*porn.*");
}

#[test]
fn test_filter_find_blacklisted_title_whitelisted() {
    let (blacklist_file, whitelist_file) = create_temp_filter_files();
    let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

    let titles = vec![
        "sex education documentary".to_string(),
        "medical adult content".to_string(),
    ];
    assert!(filter.find_blacklisted_title(&titles).is_none());
}

#[test]
#[serial]
fn test_browser_manager_integration() {
    let manager = BrowserManager::new(
        "echo".to_string(),
        "nonexistent-process".to_string(),
    );

    let result = manager.start_browser("test-url");
    assert!(result.is_ok());

    assert!(!manager.has_running_processes());

    let result = manager.kill_browser_processes();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_background_manager_integration() {
    let bg = BackgroundManager::new();

    assert!(bg.set_normal_background("/tmp/test_normal.jpg").is_ok());
    assert!(bg.set_blocked_background("/tmp/test_blocked.jpg").is_ok());
    assert!(bg.set_bathroom_break_background("/tmp/test_break.jpg").is_ok());
}

#[test]
fn test_complete_workflow_simulation() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("workflow_state.json");
    let (blacklist_file, whitelist_file) = create_temp_filter_files();

    let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();
    let manager = BrowserManager::new("echo".to_string(), "test-process".to_string());
    let mut state = AppState::default();

    assert!(!state.is_blocked());

    let bad_titles = vec!["inappropriate porn content".to_string()];
    if let Some((title, pattern)) = filter.find_blacklisted_title(&bad_titles) {
        assert_eq!(title, "inappropriate porn content");
        assert!(!pattern.is_empty());

        let _ = manager.kill_browser_processes();
        state.block_browser(10);
        let _ = BackgroundManager::new().set_blocked_background("/tmp/blocked.jpg");
    }

    assert!(state.is_blocked());

    state.save(&state_file).unwrap();

    let loaded_state = AppState::load(&state_file).unwrap();
    assert!(loaded_state.is_blocked());

    let mut state = loaded_state;
    state.start_bathroom_break(5, 2);
    assert!(state.in_bathroom_break);

    state.save(&state_file).unwrap();

    let final_state = AppState::load(&state_file).unwrap();
    assert!(final_state.in_bathroom_break);
}

#[test]
fn test_timeout_expiration_logic() {
    let mut state = AppState::default();

    state.block_browser(0);
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(!state.is_blocked());

    state.start_bathroom_break(0, 1);
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(!state.is_bathroom_break_time(1));
}

#[test]
fn test_edge_case_handling() {
    let temp_dir = TempDir::new().unwrap();

    let empty_blacklist = temp_dir.path().join("empty_blacklist.txt");
    let empty_whitelist = temp_dir.path().join("empty_whitelist.txt");
    fs::write(&empty_blacklist, "").unwrap();
    fs::write(&empty_whitelist, "").unwrap();

    let filter = Filter::new(&empty_blacklist, &empty_whitelist).unwrap();
    assert!(!filter.is_blacklisted("any content"));
    assert!(!filter.check_titles(&["any content".to_string()]));
    assert!(filter.find_blacklisted_title(&["any content".to_string()]).is_none());

    let manager = BrowserManager::new("some_executable".to_string(), "".to_string());
    assert!(!manager.has_running_processes());

    let corrupted_state_file = temp_dir.path().join("corrupted_state.json");
    fs::write(&corrupted_state_file, "invalid json content").unwrap();
    let result = AppState::load(&corrupted_state_file);
    assert!(result.is_err());
}

#[test]
fn test_concurrent_operations() {
    use std::sync::Arc;
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let state_file = Arc::new(temp_dir.path().join("concurrent_state.json"));

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let state_file = Arc::clone(&state_file);
            thread::spawn(move || {
                let mut state = AppState::default();
                state.block_browser(i + 1);
                let _ = state.save(&*state_file);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_state = AppState::load(&*state_file);
    assert!(final_state.is_ok());
}

#[test]
fn test_configuration_validation() {
    let config = create_test_config();

    assert!(!config.browser.executable.is_empty());
    assert!(!config.browser.url.is_empty());
    assert!(!config.browser.process_name.is_empty());
    assert!(config.monitoring.check_frequency_seconds > 0);
    assert!(config.timeouts.blacklist_timeout_minutes > 0);
    assert!(config.timeouts.bathroom_break_minutes > 0);
    assert!(config.timeouts.bathroom_break_interval_hours > 0);
    assert!(!config.backgrounds.normal.is_empty());
    assert!(!config.backgrounds.blocked.is_empty());
    assert!(!config.backgrounds.bathroom_break.is_empty());
    assert!(!config.files.blacklist.is_empty());
    assert!(!config.files.whitelist.is_empty());
    assert!(!config.files.state_file.is_empty());
}

#[test]
fn test_time_calculations() {
    let mut state = AppState::default();

    let start_time = Utc::now();
    state.block_browser(5);

    if let Some(blocked_until) = state.blocked_until {
        let duration = blocked_until - start_time;
        assert!(duration.num_minutes() >= 4 && duration.num_minutes() <= 6);
    } else {
        panic!("blocked_until should be set");
    }

    state.start_bathroom_break(3, 2);

    let expected_next = Utc::now() + Duration::hours(2);
    let time_diff = (state.next_bathroom_break - expected_next)
        .num_seconds()
        .abs();
    assert!(time_diff < 5);
}
