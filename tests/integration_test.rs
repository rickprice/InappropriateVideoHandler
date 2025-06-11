// Integration tests that don't require X11 or display environment
use inappropriate_video_handler::config::{Config, BrowserConfig, MonitoringConfig, TimeoutConfig, BackgroundConfig, FileConfig};
use inappropriate_video_handler::state::AppState;
use inappropriate_video_handler::filter::Filter;
use inappropriate_video_handler::browser::BrowserManager;
use inappropriate_video_handler::background::BackgroundManager;

use tempfile::{NamedTempFile, TempDir};
use std::io::Write;
use std::fs;
use chrono::{Utc, Duration};
use serial_test::serial;

fn create_test_config() -> Config {
    Config {
        browser: BrowserConfig {
            executable: "echo".to_string(), // Use echo instead of real browser for testing
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
        },
        backgrounds: BackgroundConfig {
            normal: "/tmp/test_normal.jpg".to_string(),
            blocked: "/tmp/test_blocked.jpg".to_string(),
            bathroom_break: "/tmp/test_break.jpg".to_string(),
        },
        files: FileConfig {
            blacklist: "test_blacklist.txt".to_string(),
            whitelist: "test_whitelist.txt".to_string(),
            state_file: "/tmp/test_state.json".to_string(),
        },
    }
}

fn create_temp_filter_files() -> (NamedTempFile, NamedTempFile) {
    let mut blacklist_file = NamedTempFile::new().unwrap();
    blacklist_file.write_all(b".*porn.*\n.*adult.*\n.*xxx.*").unwrap();
    
    let mut whitelist_file = NamedTempFile::new().unwrap();
    whitelist_file.write_all(b".*education.*\n.*medical.*").unwrap();
    
    (blacklist_file, whitelist_file)
}

#[test]
fn test_config_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");
    
    let original_config = create_test_config();
    
    // Save config to YAML
    let yaml_content = serde_yaml::to_string(&original_config).unwrap();
    fs::write(&config_path, yaml_content).unwrap();
    
    // Load config from YAML
    let loaded_config = Config::load(&config_path).unwrap();
    
    // Verify all fields match
    assert_eq!(loaded_config.browser.executable, original_config.browser.executable);
    assert_eq!(loaded_config.browser.url, original_config.browser.url);
    assert_eq!(loaded_config.monitoring.check_frequency_seconds, original_config.monitoring.check_frequency_seconds);
    assert_eq!(loaded_config.timeouts.blacklist_timeout_minutes, original_config.timeouts.blacklist_timeout_minutes);
}

#[test]
fn test_state_persistence_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("test_state.json");
    
    // Create initial state
    let mut state = AppState::default();
    assert!(!state.is_blocked());
    assert!(!state.in_bathroom_break);
    
    // Block browser
    state.block_browser(10);
    assert!(state.is_blocked());
    state.save(&state_file).unwrap();
    
    // Load state and verify block persists
    let loaded_state = AppState::load(&state_file).unwrap();
    assert!(loaded_state.is_blocked());
    
    // Start bathroom break
    let mut state = loaded_state;
    state.start_bathroom_break(5, 2);
    assert!(state.in_bathroom_break);
    state.save(&state_file).unwrap();
    
    // Load state and verify bathroom break persists
    let final_state = AppState::load(&state_file).unwrap();
    assert!(final_state.in_bathroom_break);
}

#[test]
fn test_filter_integration() {
    let (blacklist_file, whitelist_file) = create_temp_filter_files();
    
    let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();
    
    // Test blacklist matching
    assert!(filter.is_blacklisted("free porn videos"));
    assert!(filter.is_blacklisted("adult content"));
    assert!(filter.is_blacklisted("xxx movies"));
    
    // Test whitelist override
    assert!(!filter.is_blacklisted("sex education documentary"));
    assert!(!filter.is_blacklisted("medical adult content"));
    
    // Test clean content
    assert!(!filter.is_blacklisted("cooking tutorial"));
    
    // Test multiple titles
    let titles = vec![
        "cooking tutorial".to_string(),
        "news update".to_string(),
        "free porn videos".to_string(), // This should trigger
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
#[serial]
fn test_browser_manager_integration() {
    let manager = BrowserManager::new(
        "echo".to_string(), // Use echo for safe testing
        "nonexistent-process".to_string(),
    );
    
    // Test starting "browser" (echo command)
    let result = manager.start_browser("test-url");
    assert!(result.is_ok());
    
    // Test process finding (should find nothing for our fake process)
    assert!(!manager.has_running_processes());
    
    // Test killing processes (should succeed even with no processes)
    let result = manager.kill_browser_processes();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_background_manager_integration() {
    // Test all background setting methods
    let result1 = BackgroundManager::set_normal_background("/tmp/test_normal.jpg");
    let result2 = BackgroundManager::set_blocked_background("/tmp/test_blocked.jpg");
    let result3 = BackgroundManager::set_bathroom_break_background("/tmp/test_break.jpg");
    
    // All should complete without error (even if feh fails)
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
}

#[test]
fn test_complete_workflow_simulation() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("workflow_state.json");
    let (blacklist_file, whitelist_file) = create_temp_filter_files();
    
    // Initialize components
    let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();
    let manager = BrowserManager::new("echo".to_string(), "test-process".to_string());
    let mut state = AppState::default();
    
    // Simulate normal operation
    assert!(!state.is_blocked());
    
    // Simulate inappropriate content detection
    let bad_titles = vec!["inappropriate porn content".to_string()];
    if filter.check_titles(&bad_titles) {
        // Block browser
        let _ = manager.kill_browser_processes();
        state.block_browser(10);
        let _ = BackgroundManager::set_blocked_background("/tmp/blocked.jpg");
    }
    
    // Verify state
    assert!(state.is_blocked());
    
    // Save state
    state.save(&state_file).unwrap();
    
    // Simulate restart - load state
    let loaded_state = AppState::load(&state_file).unwrap();
    assert!(loaded_state.is_blocked());
    
    // Simulate bathroom break time
    let mut state = loaded_state;
    state.start_bathroom_break(5, 2);
    assert!(state.in_bathroom_break);
    
    // Save final state
    state.save(&state_file).unwrap();
    
    // Verify final state persists
    let final_state = AppState::load(&state_file).unwrap();
    assert!(final_state.in_bathroom_break);
}

#[test]
fn test_timeout_expiration_logic() {
    let mut state = AppState::default();
    
    // Test blocked timeout expiration
    state.block_browser(0); // Block for 0 minutes (immediate expiration)
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(!state.is_blocked()); // Should be expired
    
    // Test bathroom break expiration
    state.start_bathroom_break(0, 1); // 0 minute break
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(!state.is_bathroom_break_time(1)); // Should be expired
}

#[test]
fn test_edge_case_handling() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test with empty filter files
    let empty_blacklist = temp_dir.path().join("empty_blacklist.txt");
    let empty_whitelist = temp_dir.path().join("empty_whitelist.txt");
    fs::write(&empty_blacklist, "").unwrap();
    fs::write(&empty_whitelist, "").unwrap();
    
    let filter = Filter::new(&empty_blacklist, &empty_whitelist).unwrap();
    assert!(!filter.is_blacklisted("any content"));
    assert!(!filter.check_titles(&vec!["any content".to_string()]));
    
    // Test browser manager with empty process name
    let manager = BrowserManager::new("echo".to_string(), "".to_string());
    assert!(!manager.has_running_processes());
    
    // Test state with corrupted file
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
    
    // Test concurrent state operations
    let handles: Vec<_> = (0..5).map(|i| {
        let state_file = Arc::clone(&state_file);
        thread::spawn(move || {
            let mut state = AppState::default();
            state.block_browser(i + 1);
            let _ = state.save(&*state_file);
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // At least one save should have succeeded
    let final_state = AppState::load(&*state_file);
    assert!(final_state.is_ok());
}

#[test]
fn test_configuration_validation() {
    let config = create_test_config();
    
    // Validate all required fields are present
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
    
    // Test blocking for specific duration
    let start_time = Utc::now();
    state.block_browser(5); // 5 minutes
    
    if let Some(blocked_until) = state.blocked_until {
        let duration = blocked_until - start_time;
        assert!(duration.num_minutes() >= 4 && duration.num_minutes() <= 6); // Allow some variance
    } else {
        panic!("blocked_until should be set");
    }
    
    // Test bathroom break scheduling
    state.start_bathroom_break(3, 2); // 3 min break, next in 2 hours
    
    let expected_next = Utc::now() + Duration::hours(2);
    let time_diff = (state.next_bathroom_break - expected_next).num_seconds().abs();
    assert!(time_diff < 5); // Allow 5 seconds variance
}