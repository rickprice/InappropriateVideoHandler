use anyhow::Result;
use std::process::Command;

pub struct BackgroundManager {
    debug_level: u8,
}

impl BackgroundManager {
    pub fn new(debug_level: u8) -> Self {
        BackgroundManager { debug_level }
    }

    pub fn set_background(&self, image_path: &str) -> Result<()> {
        if self.debug_level >= 1 {
            eprintln!("[DEBUG] Setting background: feh --bg-scale '{}'", image_path);
        }

        let output = Command::new("feh")
            .arg("--bg-scale")
            .arg(image_path)
            .output()?;

        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] feh exit status: {}", output.status);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Failed to set background: {}", stderr);
            if self.debug_level >= 2 {
                eprintln!("[DEBUG2] feh stderr: {}", stderr);
            }
        } else if self.debug_level >= 2 {
            eprintln!("[DEBUG2] Background set successfully");
        }

        Ok(())
    }

    pub fn set_normal_background(&self, image_path: &str) -> Result<()> {
        if self.debug_level >= 1 {
            eprintln!("[DEBUG] set_normal_background('{}')", image_path);
        }
        self.set_background(image_path)
    }

    pub fn set_blocked_background(&self, image_path: &str) -> Result<()> {
        if self.debug_level >= 1 {
            eprintln!("[DEBUG] set_blocked_background('{}')", image_path);
        }
        self.set_background(image_path)
    }

    pub fn set_bathroom_break_background(&self, image_path: &str) -> Result<()> {
        if self.debug_level >= 1 {
            eprintln!("[DEBUG] set_bathroom_break_background('{}')", image_path);
        }
        self.set_background(image_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn bg(debug_level: u8) -> BackgroundManager {
        BackgroundManager::new(debug_level)
    }

    #[test]
    fn test_background_manager_new() {
        let manager = BackgroundManager::new(0);
        assert_eq!(manager.debug_level, 0);
    }

    #[test]
    fn test_background_manager_new_all_debug_levels() {
        for level in 0u8..=3 {
            let manager = BackgroundManager::new(level);
            assert_eq!(manager.debug_level, level);
        }
    }

    #[test]
    #[serial]
    fn test_set_background_nonexistent_file() {
        let result = bg(0).set_background("/nonexistent/path/image.jpg");
        // Should complete without error even if feh fails
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_empty_path() {
        let result = bg(0).set_background("");
        // Should complete without error even if feh fails
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_invalid_path() {
        let result = bg(0).set_background("/dev/null");
        // Should complete without error even if feh fails with invalid image
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_normal_background() {
        let result = bg(0).set_normal_background("/test/normal.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_blocked_background() {
        let result = bg(0).set_blocked_background("/test/blocked.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_bathroom_break_background() {
        let result = bg(0).set_bathroom_break_background("/test/break.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_with_spaces() {
        let result = bg(0).set_background("/test path/image with spaces.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_with_special_characters() {
        let paths = vec![
            "/test/image-with-dashes.jpg",
            "/test/image_with_underscores.jpg",
            "/test/image.with.dots.jpg",
            "/test/image@special.jpg",
        ];

        for path in paths {
            let result = bg(0).set_background(path);
            assert!(result.is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_multiple_background_changes() {
        let manager = bg(0);
        let backgrounds = vec!["/test/bg1.jpg", "/test/bg2.jpg", "/test/bg3.jpg"];

        for path in backgrounds {
            assert!(manager.set_background(path).is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_background_methods_consistency() {
        let manager = bg(0);
        let test_path = "/test/consistency.jpg";

        assert!(manager.set_background(test_path).is_ok());
        assert!(manager.set_normal_background(test_path).is_ok());
        assert!(manager.set_blocked_background(test_path).is_ok());
        assert!(manager.set_bathroom_break_background(test_path).is_ok());
    }

    #[test]
    #[serial]
    fn test_background_unicode_paths() {
        let manager = bg(0);
        let unicode_paths = vec!["/test/测试.jpg", "/test/café.jpg", "/test/🖼️.jpg"];

        for path in unicode_paths {
            assert!(manager.set_background(path).is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_background_very_long_path() {
        let long_path = format!("/test/{}.jpg", "a".repeat(1000));
        assert!(bg(0).set_background(&long_path).is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_with_debug_level() {
        // All debug levels should still return Ok (feh errors are swallowed)
        for level in 0u8..=3 {
            let result = bg(level).set_background("/nonexistent/path.jpg");
            assert!(result.is_ok(), "debug_level={}", level);
        }
    }

    #[test]
    #[serial]
    fn test_all_methods_with_debug_level_2() {
        let manager = bg(2);
        assert!(manager.set_background("/test/debug.jpg").is_ok());
        assert!(manager.set_normal_background("/test/normal.jpg").is_ok());
        assert!(manager.set_blocked_background("/test/blocked.jpg").is_ok());
        assert!(manager.set_bathroom_break_background("/test/break.jpg").is_ok());
    }

    // Note: These tests assume feh is installed but don't verify the actual
    // background change since that would require a display environment.
    // The tests verify that the API calls complete without panicking.

    #[test]
    #[serial]
    #[ignore] // Only run if feh is available and display is accessible
    fn test_set_background_with_real_image() {
        use std::process::Command;

        let feh_check = Command::new("which").arg("feh").output();
        if feh_check.is_err() {
            return;
        }

        let test_paths = vec![
            "/usr/share/pixmaps/debian-logo.png",
            "/usr/share/icons/hicolor/48x48/apps/google-chrome.png",
        ];

        for path in test_paths {
            if std::path::Path::new(path).exists() {
                let result = bg(1).set_background(path);
                assert!(result.is_ok());
                break;
            }
        }
    }
}
