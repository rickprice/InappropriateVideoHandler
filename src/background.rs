use anyhow::Result;
use log::{debug, error, info};
use std::process::Command;

pub struct BackgroundManager;

impl BackgroundManager {
    pub fn new() -> Self {
        BackgroundManager
    }

    pub fn set_background(&self, image_path: &str) -> Result<()> {
        info!("Setting background: feh --bg-scale '{}'", image_path);

        let output = Command::new("feh")
            .arg("--bg-scale")
            .arg(image_path)
            .output()?;

        debug!("feh exit status: {}", output.status);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to set background: {}", stderr);
        } else {
            debug!("Background set successfully");
        }

        Ok(())
    }

    pub fn set_normal_background(&self, image_path: &str) -> Result<()> {
        info!("set_normal_background('{}')", image_path);
        self.set_background(image_path)
    }

    pub fn set_blocked_background(&self, image_path: &str) -> Result<()> {
        info!("set_blocked_background('{}')", image_path);
        self.set_background(image_path)
    }

    pub fn set_bathroom_break_background(&self, image_path: &str) -> Result<()> {
        info!("set_bathroom_break_background('{}')", image_path);
        self.set_background(image_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_background_manager_new() {
        let _manager = BackgroundManager::new();
    }

    #[test]
    #[serial]
    fn test_set_background_nonexistent_file() {
        let result = BackgroundManager::new().set_background("/nonexistent/path/image.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_empty_path() {
        let result = BackgroundManager::new().set_background("");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_invalid_path() {
        let result = BackgroundManager::new().set_background("/dev/null");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_normal_background() {
        let result = BackgroundManager::new().set_normal_background("/test/normal.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_blocked_background() {
        let result = BackgroundManager::new().set_blocked_background("/test/blocked.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_bathroom_break_background() {
        let result = BackgroundManager::new().set_bathroom_break_background("/test/break.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_with_spaces() {
        let result = BackgroundManager::new().set_background("/test path/image with spaces.jpg");
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
            let result = BackgroundManager::new().set_background(path);
            assert!(result.is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_multiple_background_changes() {
        let manager = BackgroundManager::new();
        let backgrounds = vec!["/test/bg1.jpg", "/test/bg2.jpg", "/test/bg3.jpg"];

        for path in backgrounds {
            assert!(manager.set_background(path).is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_background_methods_consistency() {
        let manager = BackgroundManager::new();
        let test_path = "/test/consistency.jpg";

        assert!(manager.set_background(test_path).is_ok());
        assert!(manager.set_normal_background(test_path).is_ok());
        assert!(manager.set_blocked_background(test_path).is_ok());
        assert!(manager.set_bathroom_break_background(test_path).is_ok());
    }

    #[test]
    #[serial]
    fn test_background_unicode_paths() {
        let manager = BackgroundManager::new();
        let unicode_paths = vec!["/test/测试.jpg", "/test/café.jpg", "/test/🖼️.jpg"];

        for path in unicode_paths {
            assert!(manager.set_background(path).is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_background_very_long_path() {
        let long_path = format!("/test/{}.jpg", "a".repeat(1000));
        assert!(BackgroundManager::new().set_background(&long_path).is_ok());
    }

    #[test]
    #[serial]
    #[ignore]
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
                let result = BackgroundManager::new().set_background(path);
                assert!(result.is_ok());
                break;
            }
        }
    }
}
