use anyhow::Result;
use log::{debug, error, info};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::process::{Child, Command};

pub struct BrowserManager {
    executable: String,
    process_name: String,
}

impl BrowserManager {
    pub fn new(executable: String, process_name: String) -> Self {
        BrowserManager {
            executable,
            process_name,
        }
    }

    pub fn start_browser(&self, url: &str) -> Result<Child> {
        info!("Starting browser: '{}' '{}'", self.executable, url);
        let child = Command::new(&self.executable).arg(url).spawn()?;
        info!("Browser spawned with pid {}", child.id());
        Ok(child)
    }

    pub fn kill_browser_processes(&self) -> Result<()> {
        let pids = self.find_browser_pids()?;

        info!("kill_browser_processes: found {} pid(s) for '{}'",
            pids.len(), self.process_name);
        debug!("PIDs to SIGTERM: {:?}", pids);

        for pid in pids {
            match signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
                Ok(_) => {
                    println!("Terminated process {}", pid);
                    debug!("SIGTERM sent to pid {}", pid);
                }
                Err(e) => error!("Failed to terminate process {}: {}", pid, e),
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(2));

        let remaining_pids = self.find_browser_pids()?;
        if !remaining_pids.is_empty() {
            info!("{} pid(s) still running after SIGTERM, sending SIGKILL", remaining_pids.len());
            debug!("PIDs to SIGKILL: {:?}", remaining_pids);
        }

        for pid in remaining_pids {
            match signal::kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(_) => {
                    println!("Killed process {}", pid);
                    debug!("SIGKILL sent to pid {}", pid);
                }
                Err(e) => error!("Failed to kill process {}: {}", pid, e),
            }
        }

        Ok(())
    }

    pub fn get_pids(&self) -> Vec<i32> {
        self.find_browser_pids().unwrap_or_default()
    }

    fn find_browser_pids(&self) -> Result<Vec<i32>> {
        if self.process_name.is_empty() {
            debug!("find_browser_pids: process_name is empty, returning no pids");
            return Ok(Vec::new());
        }

        debug!("find_browser_pids: pgrep -f '{}'", self.process_name);

        let output = Command::new("pgrep")
            .arg("-f")
            .arg(&self.process_name)
            .output()?;

        if !output.status.success() {
            debug!("pgrep returned no results (exit {})", output.status);
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pids: Vec<i32> = stdout
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();

        debug!("pgrep found pids: {:?}", pids);

        Ok(pids)
    }

    #[allow(dead_code)]
    pub fn has_running_processes(&self) -> bool {
        !self.find_browser_pids().unwrap_or_default().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn make_manager(executable: &str, process_name: &str) -> BrowserManager {
        BrowserManager::new(executable.to_string(), process_name.to_string())
    }

    #[test]
    fn test_browser_manager_new() {
        let manager = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
        );

        assert_eq!(manager.executable, "google-chrome-stable");
        assert_eq!(manager.process_name, "chrome");
    }

    #[test]
    fn test_browser_manager_new_with_different_values() {
        let manager = BrowserManager::new(
            "/usr/bin/chromium".to_string(),
            "chromium-browser".to_string(),
        );

        assert_eq!(manager.executable, "/usr/bin/chromium");
        assert_eq!(manager.process_name, "chromium-browser");
    }

    #[test]
    #[serial]
    fn test_find_browser_pids_nonexistent_process() {
        let manager = make_manager("nonexistent-browser-12345", "nonexistent-browser-12345");

        let pids = manager.find_browser_pids().unwrap();
        assert_eq!(pids.len(), 0);
    }

    #[test]
    #[serial]
    fn test_has_running_processes_none() {
        let manager = make_manager("nonexistent-browser-12345", "nonexistent-browser-12345");
        assert!(!manager.has_running_processes());
    }

    #[test]
    #[serial]
    fn test_start_browser_invalid_executable() {
        let manager = make_manager("nonexistent-browser-executable-12345", "nonexistent-process");

        let result = manager.start_browser("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_kill_browser_processes_no_processes() {
        let manager = make_manager("nonexistent-browser-12345", "nonexistent-browser-12345");

        let result = manager.kill_browser_processes();
        assert!(result.is_ok());
    }

    #[test]
    fn test_browser_manager_with_empty_strings() {
        let manager = BrowserManager::new("".to_string(), "".to_string());

        assert_eq!(manager.executable, "");
        assert_eq!(manager.process_name, "");

        let result = manager.start_browser("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_find_browser_pids_empty_process_name() {
        let manager = BrowserManager::new("google-chrome-stable".to_string(), "".to_string());

        let pids = manager.find_browser_pids().unwrap();
        assert_eq!(pids.len(), 0);
    }

    #[test]
    fn test_start_browser_url_variants() {
        let manager = make_manager("nonexistent-browser", "nonexistent-process");

        let urls = vec![
            "https://example.com",
            "http://example.com",
            "file:///tmp/test.html",
            "about:blank",
            "",
        ];

        for url in urls {
            let result = manager.start_browser(url);
            assert!(result.is_err());
        }
    }

    #[test]
    #[serial]
    fn test_process_name_matching() {
        let test_cases = vec![
            ("sh", "sh"),
            ("init", "init"),
            ("kernel", "kernel"),
        ];

        for (executable, process_name) in test_cases {
            let manager = BrowserManager::new(executable.to_string(), process_name.to_string());
            let result = manager.find_browser_pids();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_browser_manager_clone_behavior() {
        let manager1 = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
        );
        let manager2 = BrowserManager::new("chromium".to_string(), "chromium".to_string());

        assert_eq!(manager1.executable, "google-chrome-stable");
        assert_eq!(manager2.executable, "chromium");
        assert_ne!(manager1.executable, manager2.executable);
    }

    #[test]
    #[serial]
    #[ignore]
    fn test_start_browser_with_real_browser() {
        let manager = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
        );

        let result = manager.start_browser("https://example.com");
        if result.is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = manager.kill_browser_processes();
        }
    }

    #[test]
    #[serial]
    #[ignore]
    fn test_browser_lifecycle() {
        let manager = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
        );

        let child = manager.start_browser("https://example.com");
        if child.is_err() {
            return;
        }

        std::thread::sleep(std::time::Duration::from_secs(3));
        assert!(manager.has_running_processes());

        let result = manager.kill_browser_processes();
        assert!(result.is_ok());

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
