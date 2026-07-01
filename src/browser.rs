use anyhow::Result;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::process::{Child, Command};

pub struct BrowserManager {
    executable: String,
    process_name: String,
    debug_level: u8,
}

impl BrowserManager {
    pub fn new(executable: String, process_name: String, debug_level: u8) -> Self {
        BrowserManager {
            executable,
            process_name,
            debug_level,
        }
    }

    pub fn start_browser(&self, url: &str) -> Result<Child> {
        if self.debug_level >= 1 {
            eprintln!("[DEBUG] Starting browser: '{}' '{}'", self.executable, url);
        }
        let child = Command::new(&self.executable).arg(url).spawn()?;
        if self.debug_level >= 1 {
            eprintln!("[DEBUG] Browser spawned with pid {}", child.id());
        }
        Ok(child)
    }

    pub fn kill_browser_processes(&self) -> Result<()> {
        let pids = self.find_browser_pids()?;

        if self.debug_level >= 1 {
            eprintln!("[DEBUG] kill_browser_processes: found {} pid(s) for '{}'",
                pids.len(), self.process_name);
        }
        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] PIDs to SIGTERM: {:?}", pids);
        }

        for pid in pids {
            match signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
                Ok(_) => {
                    println!("Terminated process {}", pid);
                    if self.debug_level >= 2 {
                        eprintln!("[DEBUG2] SIGTERM sent to pid {}", pid);
                    }
                }
                Err(e) => eprintln!("Failed to terminate process {}: {}", pid, e),
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(2));

        let remaining_pids = self.find_browser_pids()?;
        if self.debug_level >= 1 && !remaining_pids.is_empty() {
            eprintln!("[DEBUG] {} pid(s) still running after SIGTERM, sending SIGKILL", remaining_pids.len());
        }
        if self.debug_level >= 2 && !remaining_pids.is_empty() {
            eprintln!("[DEBUG2] PIDs to SIGKILL: {:?}", remaining_pids);
        }

        for pid in remaining_pids {
            match signal::kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(_) => {
                    println!("Killed process {}", pid);
                    if self.debug_level >= 2 {
                        eprintln!("[DEBUG2] SIGKILL sent to pid {}", pid);
                    }
                }
                Err(e) => eprintln!("Failed to kill process {}: {}", pid, e),
            }
        }

        Ok(())
    }

    pub fn get_pids(&self) -> Vec<i32> {
        self.find_browser_pids().unwrap_or_default()
    }

    fn find_browser_pids(&self) -> Result<Vec<i32>> {
        if self.process_name.is_empty() {
            if self.debug_level >= 2 {
                eprintln!("[DEBUG2] find_browser_pids: process_name is empty, returning no pids");
            }
            return Ok(Vec::new());
        }

        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] find_browser_pids: pgrep -f '{}'", self.process_name);
        }

        let output = Command::new("pgrep")
            .arg("-f")
            .arg(&self.process_name)
            .output()?;

        if !output.status.success() {
            if self.debug_level >= 2 {
                eprintln!("[DEBUG2] pgrep returned no results (exit {})", output.status);
            }
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pids: Vec<i32> = stdout
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();

        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] pgrep found pids: {:?}", pids);
        }

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
        BrowserManager::new(executable.to_string(), process_name.to_string(), 0)
    }

    #[test]
    fn test_browser_manager_new() {
        let manager = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
            0,
        );

        assert_eq!(manager.executable, "google-chrome-stable");
        assert_eq!(manager.process_name, "chrome");
        assert_eq!(manager.debug_level, 0);
    }

    #[test]
    fn test_browser_manager_new_with_debug_level() {
        for level in 0u8..=3 {
            let manager = BrowserManager::new("browser".to_string(), "proc".to_string(), level);
            assert_eq!(manager.debug_level, level);
        }
    }

    #[test]
    fn test_browser_manager_new_with_different_values() {
        let manager = BrowserManager::new(
            "/usr/bin/chromium".to_string(),
            "chromium-browser".to_string(),
            0,
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
    fn test_find_browser_pids_with_debug() {
        for level in 0u8..=3 {
            let manager = BrowserManager::new(
                "nonexistent-browser-12345".to_string(),
                "nonexistent-browser-12345".to_string(),
                level,
            );
            let pids = manager.find_browser_pids().unwrap();
            assert_eq!(pids.len(), 0, "debug_level={}", level);
        }
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

        // Should not fail even if no processes exist
        let result = manager.kill_browser_processes();
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_kill_browser_processes_no_processes_with_debug() {
        for level in 0u8..=3 {
            let manager = BrowserManager::new(
                "nonexistent-browser-12345".to_string(),
                "nonexistent-browser-12345".to_string(),
                level,
            );
            assert!(manager.kill_browser_processes().is_ok(), "debug_level={}", level);
        }
    }

    #[test]
    fn test_browser_manager_with_empty_strings() {
        let manager = BrowserManager::new("".to_string(), "".to_string(), 0);

        assert_eq!(manager.executable, "");
        assert_eq!(manager.process_name, "");

        // Starting with empty executable should fail
        let result = manager.start_browser("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_find_browser_pids_empty_process_name() {
        let manager = BrowserManager::new("google-chrome-stable".to_string(), "".to_string(), 0);

        // Empty process_name returns empty vec early (our guard clause)
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
            assert!(result.is_err()); // Should fail due to nonexistent executable
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
            let manager = BrowserManager::new(executable.to_string(), process_name.to_string(), 0);
            let result = manager.find_browser_pids();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_browser_manager_clone_behavior() {
        let manager1 = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
            0,
        );
        let manager2 = BrowserManager::new("chromium".to_string(), "chromium".to_string(), 0);

        assert_eq!(manager1.executable, "google-chrome-stable");
        assert_eq!(manager2.executable, "chromium");
        assert_ne!(manager1.executable, manager2.executable);
    }

    // Note: The following tests would require actual browser processes running
    // and proper privileges to kill them.

    #[test]
    #[serial]
    #[ignore] // Ignored by default as it requires a real browser process
    fn test_start_browser_with_real_browser() {
        let manager = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
            1,
        );

        let result = manager.start_browser("https://example.com");
        if result.is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = manager.kill_browser_processes();
        }
    }

    #[test]
    #[serial]
    #[ignore] // Ignored by default as it requires a real browser process
    fn test_browser_lifecycle() {
        let manager = BrowserManager::new(
            "google-chrome-stable".to_string(),
            "chrome".to_string(),
            1,
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
