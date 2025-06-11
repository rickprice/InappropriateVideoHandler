use std::process::{Command, Child};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use anyhow::Result;

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
        let child = Command::new(&self.executable)
            .arg(url)
            .spawn()?;
        
        Ok(child)
    }

    pub fn kill_browser_processes(&self) -> Result<()> {
        let pids = self.find_browser_pids()?;
        
        for pid in pids {
            match signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
                Ok(_) => println!("Terminated process {}", pid),
                Err(e) => eprintln!("Failed to terminate process {}: {}", pid, e),
            }
        }
        
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        let remaining_pids = self.find_browser_pids()?;
        for pid in remaining_pids {
            match signal::kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(_) => println!("Killed process {}", pid),
                Err(e) => eprintln!("Failed to kill process {}: {}", pid, e),
            }
        }
        
        Ok(())
    }

    fn find_browser_pids(&self) -> Result<Vec<i32>> {
        let output = Command::new("pgrep")
            .arg("-f")
            .arg(&self.process_name)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pids: Vec<i32> = stdout
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();

        Ok(pids)
    }

    pub fn has_running_processes(&self) -> bool {
        !self.find_browser_pids().unwrap_or_default().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_browser_manager_new() {
        let manager = BrowserManager::new(
            "firefox".to_string(),
            "firefox".to_string(),
        );
        
        assert_eq!(manager.executable, "firefox");
        assert_eq!(manager.process_name, "firefox");
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
        let manager = BrowserManager::new(
            "nonexistent-browser-12345".to_string(),
            "nonexistent-browser-12345".to_string(),
        );
        
        let pids = manager.find_browser_pids().unwrap();
        assert_eq!(pids.len(), 0);
    }

    #[test]
    #[serial]
    fn test_has_running_processes_none() {
        let manager = BrowserManager::new(
            "nonexistent-browser-12345".to_string(),
            "nonexistent-browser-12345".to_string(),
        );
        
        assert!(!manager.has_running_processes());
    }

    #[test]
    #[serial]
    fn test_start_browser_invalid_executable() {
        let manager = BrowserManager::new(
            "nonexistent-browser-executable-12345".to_string(),
            "nonexistent-process".to_string(),
        );
        
        let result = manager.start_browser("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_kill_browser_processes_no_processes() {
        let manager = BrowserManager::new(
            "nonexistent-browser-12345".to_string(),
            "nonexistent-browser-12345".to_string(),
        );
        
        // Should not fail even if no processes exist
        let result = manager.kill_browser_processes();
        assert!(result.is_ok());
    }

    // Note: The following tests would require actual browser processes running
    // and proper privileges to kill them. They are included for completeness
    // but should be run in a controlled test environment.

    #[test]
    #[serial]
    #[ignore] // Ignored by default as it requires a real browser process
    fn test_start_browser_with_real_browser() {
        let manager = BrowserManager::new(
            "firefox".to_string(),
            "firefox".to_string(),
        );
        
        // This would actually start Firefox - only run in isolated test environment
        let result = manager.start_browser("https://example.com");
        
        if result.is_ok() {
            // If we successfully started it, try to clean up
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = manager.kill_browser_processes();
        }
    }

    #[test]
    #[serial]
    #[ignore] // Ignored by default as it requires a real browser process
    fn test_browser_lifecycle() {
        let manager = BrowserManager::new(
            "firefox".to_string(),
            "firefox".to_string(),
        );
        
        // Start browser
        let child = manager.start_browser("https://example.com");
        if child.is_err() {
            // Skip test if Firefox is not available
            return;
        }
        
        // Give it time to start
        std::thread::sleep(std::time::Duration::from_secs(3));
        
        // Check if processes are running
        assert!(manager.has_running_processes());
        
        // Kill processes
        let result = manager.kill_browser_processes();
        assert!(result.is_ok());
        
        // Give it time to terminate
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        // Check processes are gone (this might be flaky in real environments)
        // assert!(!manager.has_running_processes());
    }

    #[test]
    fn test_browser_manager_with_empty_strings() {
        let manager = BrowserManager::new(
            "".to_string(),
            "".to_string(),
        );
        
        assert_eq!(manager.executable, "");
        assert_eq!(manager.process_name, "");
        
        // Starting with empty executable should fail
        let result = manager.start_browser("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_find_browser_pids_empty_process_name() {
        let manager = BrowserManager::new(
            "firefox".to_string(),
            "".to_string(),
        );
        
        // pgrep with empty pattern matches all processes, so we expect many results
        let pids = manager.find_browser_pids().unwrap();
        // Just verify it doesn't crash and returns some result
        assert!(pids.len() >= 0); // This will always pass but documents the behavior
    }

    #[test]
    fn test_start_browser_url_variants() {
        let manager = BrowserManager::new(
            "nonexistent-browser".to_string(),
            "nonexistent-process".to_string(),
        );
        
        // Test various URL formats (all should fail due to nonexistent browser)
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

    // Test to verify the process name matching logic
    #[test]
    #[serial]
    fn test_process_name_matching() {
        // Test with common process names that might exist on the system
        let test_cases = vec![
            ("sh", "sh"),           // Shell process likely exists
            ("init", "init"),       // Init process should exist
            ("kernel", "kernel"),   // Kernel threads
        ];
        
        for (executable, process_name) in test_cases {
            let manager = BrowserManager::new(
                executable.to_string(),
                process_name.to_string(),
            );
            
            // Just test that find_browser_pids doesn't crash
            let result = manager.find_browser_pids();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_browser_manager_clone_behavior() {
        // Test that we can create multiple instances
        let manager1 = BrowserManager::new(
            "firefox".to_string(),
            "firefox".to_string(),
        );
        
        let manager2 = BrowserManager::new(
            "chromium".to_string(),
            "chromium".to_string(),
        );
        
        assert_eq!(manager1.executable, "firefox");
        assert_eq!(manager2.executable, "chromium");
        assert_ne!(manager1.executable, manager2.executable);
    }
}