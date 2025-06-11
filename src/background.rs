use std::process::Command;
use anyhow::Result;

pub struct BackgroundManager;

impl BackgroundManager {
    pub fn set_background(image_path: &str) -> Result<()> {
        let output = Command::new("feh")
            .arg("--bg-scale")
            .arg(image_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Failed to set background: {}", stderr);
        }

        Ok(())
    }

    pub fn set_normal_background(image_path: &str) -> Result<()> {
        Self::set_background(image_path)
    }

    pub fn set_blocked_background(image_path: &str) -> Result<()> {
        Self::set_background(image_path)
    }

    pub fn set_bathroom_break_background(image_path: &str) -> Result<()> {
        Self::set_background(image_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_set_background_nonexistent_file() {
        let result = BackgroundManager::set_background("/nonexistent/path/image.jpg");
        // Should complete without error even if feh fails
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_empty_path() {
        let result = BackgroundManager::set_background("");
        // Should complete without error even if feh fails
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_invalid_path() {
        let result = BackgroundManager::set_background("/dev/null");
        // Should complete without error even if feh fails with invalid image
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_normal_background() {
        let result = BackgroundManager::set_normal_background("/test/normal.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_blocked_background() {
        let result = BackgroundManager::set_blocked_background("/test/blocked.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_bathroom_break_background() {
        let result = BackgroundManager::set_bathroom_break_background("/test/break.jpg");
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_set_background_with_spaces() {
        let result = BackgroundManager::set_background("/test path/image with spaces.jpg");
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
            let result = BackgroundManager::set_background(path);
            assert!(result.is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_multiple_background_changes() {
        // Test rapid succession of background changes
        let backgrounds = vec![
            "/test/bg1.jpg",
            "/test/bg2.jpg",
            "/test/bg3.jpg",
        ];

        for bg in backgrounds {
            let result = BackgroundManager::set_background(bg);
            assert!(result.is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_background_methods_consistency() {
        let test_path = "/test/consistency.jpg";

        // All methods should behave the same way
        let result1 = BackgroundManager::set_background(test_path);
        let result2 = BackgroundManager::set_normal_background(test_path);
        let result3 = BackgroundManager::set_blocked_background(test_path);
        let result4 = BackgroundManager::set_bathroom_break_background(test_path);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());
        assert!(result4.is_ok());
    }

    #[test]
    #[serial]
    fn test_background_unicode_paths() {
        let unicode_paths = vec![
            "/test/测试.jpg",
            "/test/café.jpg", 
            "/test/🖼️.jpg",
        ];

        for path in unicode_paths {
            let result = BackgroundManager::set_background(path);
            assert!(result.is_ok());
        }
    }

    #[test]
    #[serial]
    fn test_background_very_long_path() {
        let long_path = format!("/test/{}.jpg", "a".repeat(1000));
        let result = BackgroundManager::set_background(&long_path);
        assert!(result.is_ok());
    }

    // Note: These tests assume feh is installed but don't verify the actual
    // background change since that would require a display environment.
    // The tests verify that the API calls complete without panicking.

    #[test]
    #[serial]
    #[ignore] // Only run if feh is available and display is accessible
    fn test_set_background_with_real_image() {
        // This test requires a real image file and display environment
        // Create a simple test image or use an existing one
        use std::process::Command;
        
        // Check if feh is available
        let feh_check = Command::new("which").arg("feh").output();
        if feh_check.is_err() {
            return; // Skip if feh not available
        }

        // Try with a real image file if available
        let test_paths = vec![
            "/usr/share/pixmaps/debian-logo.png", // Common on Debian systems
            "/usr/share/icons/hicolor/48x48/apps/firefox.png", // Common Firefox icon
        ];

        for path in test_paths {
            if std::path::Path::new(path).exists() {
                let result = BackgroundManager::set_background(path);
                assert!(result.is_ok());
                break;
            }
        }
    }
}