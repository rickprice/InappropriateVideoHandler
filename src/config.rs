use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub browser: BrowserConfig,
    pub monitoring: MonitoringConfig,
    pub timeouts: TimeoutConfig,
    pub backgrounds: BackgroundConfig,
    pub files: FileConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub executable: String,
    pub url: String,
    pub process_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub check_frequency_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub blacklist_timeout_minutes: u64,
    pub bathroom_break_minutes: u64,
    pub bathroom_break_interval_hours: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackgroundConfig {
    pub normal: String,
    pub blocked: String,
    pub bathroom_break: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileConfig {
    pub blacklist: String,
    pub whitelist: String,
    pub state_file: String,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Config {
            browser: BrowserConfig {
                executable: "firefox".to_string(),
                url: "https://www.google.com".to_string(),
                process_name: "firefox".to_string(),
            },
            monitoring: MonitoringConfig {
                check_frequency_seconds: 60,
            },
            timeouts: TimeoutConfig {
                blacklist_timeout_minutes: 10,
                bathroom_break_minutes: 10,
                bathroom_break_interval_hours: 3,
            },
            backgrounds: BackgroundConfig {
                normal: "/home/user/backgrounds/normal.jpg".to_string(),
                blocked: "/home/user/backgrounds/blocked.jpg".to_string(),
                bathroom_break: "/home/user/backgrounds/bathroom.jpg".to_string(),
            },
            files: FileConfig {
                blacklist: "blacklist.txt".to_string(),
                whitelist: "whitelist.txt".to_string(),
                state_file: "/tmp/ivh_state.json".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        
        assert_eq!(config.browser.executable, "firefox");
        assert_eq!(config.browser.url, "https://www.google.com");
        assert_eq!(config.browser.process_name, "firefox");
        assert_eq!(config.monitoring.check_frequency_seconds, 60);
        assert_eq!(config.timeouts.blacklist_timeout_minutes, 10);
        assert_eq!(config.timeouts.bathroom_break_minutes, 10);
        assert_eq!(config.timeouts.bathroom_break_interval_hours, 3);
        assert_eq!(config.backgrounds.normal, "/home/user/backgrounds/normal.jpg");
        assert_eq!(config.backgrounds.blocked, "/home/user/backgrounds/blocked.jpg");
        assert_eq!(config.backgrounds.bathroom_break, "/home/user/backgrounds/bathroom.jpg");
        assert_eq!(config.files.blacklist, "blacklist.txt");
        assert_eq!(config.files.whitelist, "whitelist.txt");
        assert_eq!(config.files.state_file, "/tmp/ivh_state.json");
    }

    #[test]
    fn test_config_load_valid_yaml() {
        let yaml_content = r#"
browser:
  executable: "chromium"
  url: "https://example.com"
  process_name: "chromium"

monitoring:
  check_frequency_seconds: 30

timeouts:
  blacklist_timeout_minutes: 15
  bathroom_break_minutes: 5
  bathroom_break_interval_hours: 2

backgrounds:
  normal: "/test/normal.png"
  blocked: "/test/blocked.png"
  bathroom_break: "/test/break.png"

files:
  blacklist: "test_blacklist.txt"
  whitelist: "test_whitelist.txt"
  state_file: "/test/state.json"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(yaml_content.as_bytes()).unwrap();
        
        let config = Config::load(temp_file.path()).unwrap();
        
        assert_eq!(config.browser.executable, "chromium");
        assert_eq!(config.browser.url, "https://example.com");
        assert_eq!(config.browser.process_name, "chromium");
        assert_eq!(config.monitoring.check_frequency_seconds, 30);
        assert_eq!(config.timeouts.blacklist_timeout_minutes, 15);
        assert_eq!(config.timeouts.bathroom_break_minutes, 5);
        assert_eq!(config.timeouts.bathroom_break_interval_hours, 2);
        assert_eq!(config.backgrounds.normal, "/test/normal.png");
        assert_eq!(config.backgrounds.blocked, "/test/blocked.png");
        assert_eq!(config.backgrounds.bathroom_break, "/test/break.png");
        assert_eq!(config.files.blacklist, "test_blacklist.txt");
        assert_eq!(config.files.whitelist, "test_whitelist.txt");
        assert_eq!(config.files.state_file, "/test/state.json");
    }

    #[test]
    fn test_config_load_invalid_yaml() {
        let invalid_yaml = "invalid: yaml: content: [";
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(invalid_yaml.as_bytes()).unwrap();
        
        let result = Config::load(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_load_nonexistent_file() {
        let result = Config::load("/nonexistent/path/config.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_load_incomplete_yaml() {
        let incomplete_yaml = r#"
browser:
  executable: "firefox"
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(incomplete_yaml.as_bytes()).unwrap();
        
        let result = Config::load(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_browser_config_fields() {
        let config = BrowserConfig {
            executable: "test_browser".to_string(),
            url: "https://test.com".to_string(),
            process_name: "test_process".to_string(),
        };

        assert_eq!(config.executable, "test_browser");
        assert_eq!(config.url, "https://test.com");
        assert_eq!(config.process_name, "test_process");
    }

    #[test]
    fn test_monitoring_config_fields() {
        let config = MonitoringConfig {
            check_frequency_seconds: 120,
        };

        assert_eq!(config.check_frequency_seconds, 120);
    }

    #[test]
    fn test_timeout_config_fields() {
        let config = TimeoutConfig {
            blacklist_timeout_minutes: 20,
            bathroom_break_minutes: 15,
            bathroom_break_interval_hours: 4,
        };

        assert_eq!(config.blacklist_timeout_minutes, 20);
        assert_eq!(config.bathroom_break_minutes, 15);
        assert_eq!(config.bathroom_break_interval_hours, 4);
    }

    #[test]
    fn test_background_config_fields() {
        let config = BackgroundConfig {
            normal: "/path/normal.jpg".to_string(),
            blocked: "/path/blocked.jpg".to_string(),
            bathroom_break: "/path/break.jpg".to_string(),
        };

        assert_eq!(config.normal, "/path/normal.jpg");
        assert_eq!(config.blocked, "/path/blocked.jpg");
        assert_eq!(config.bathroom_break, "/path/break.jpg");
    }

    #[test]
    fn test_file_config_fields() {
        let config = FileConfig {
            blacklist: "test_blacklist.txt".to_string(),
            whitelist: "test_whitelist.txt".to_string(),
            state_file: "/test/state.json".to_string(),
        };

        assert_eq!(config.blacklist, "test_blacklist.txt");
        assert_eq!(config.whitelist, "test_whitelist.txt");
        assert_eq!(config.state_file, "/test/state.json");
    }
}