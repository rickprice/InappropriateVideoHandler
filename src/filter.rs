use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;

pub struct Filter {
    blacklist: Vec<Regex>,
    whitelist: Vec<Regex>,
}

impl Filter {
    pub fn new<P: AsRef<Path>>(blacklist_path: P, whitelist_path: P) -> Result<Self> {
        let blacklist = Self::load_patterns(blacklist_path)?;
        let whitelist = Self::load_patterns(whitelist_path)?;

        Ok(Filter {
            blacklist,
            whitelist,
        })
    }

    fn load_patterns<P: AsRef<Path>>(path: P) -> Result<Vec<Regex>> {
        if !path.as_ref().exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        let mut patterns = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                match Regex::new(line) {
                    Ok(regex) => patterns.push(regex),
                    Err(e) => eprintln!("Invalid regex pattern '{}': {}", line, e),
                }
            }
        }

        Ok(patterns)
    }

    pub fn is_blacklisted(&self, title: &str) -> bool {
        for pattern in &self.blacklist {
            if pattern.is_match(title) && !self.is_whitelisted(title) {
                return true;
            }
        }
        false
    }

    pub fn is_whitelisted(&self, title: &str) -> bool {
        for pattern in &self.whitelist {
            if pattern.is_match(title) {
                return true;
            }
        }
        false
    }

    pub fn check_titles(&self, titles: &[String]) -> bool {
        for title in titles {
            if self.is_blacklisted(title) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file_with_content(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file
    }

    #[test]
    fn test_filter_new_with_valid_files() {
        let blacklist_content = ".*porn.*\n.*adult.*\n.*xxx.*";
        let whitelist_content = ".*education.*\n.*medical.*";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert_eq!(filter.blacklist.len(), 3);
        assert_eq!(filter.whitelist.len(), 2);
    }

    #[test]
    fn test_filter_new_with_nonexistent_files() {
        let filter =
            Filter::new("/nonexistent/blacklist.txt", "/nonexistent/whitelist.txt").unwrap();

        assert_eq!(filter.blacklist.len(), 0);
        assert_eq!(filter.whitelist.len(), 0);
    }

    #[test]
    fn test_filter_with_comments_and_empty_lines() {
        let content = "
# This is a comment
.*porn.*

# Another comment
.*adult.*

# Empty line above and below

.*xxx.*
";
        let blacklist_file = create_temp_file_with_content(content);
        let whitelist_file = create_temp_file_with_content("");

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert_eq!(filter.blacklist.len(), 3);
        assert_eq!(filter.whitelist.len(), 0);
    }

    #[test]
    fn test_filter_with_invalid_regex() {
        let content = ".*porn.*\n[invalid regex\n.*adult.*";
        let blacklist_file = create_temp_file_with_content(content);
        let whitelist_file = create_temp_file_with_content("");

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        // Should skip invalid regex and only have 2 valid ones
        assert_eq!(filter.blacklist.len(), 2);
    }

    #[test]
    fn test_is_blacklisted_simple_match() {
        let blacklist_content = ".*porn.*\n.*adult.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(filter.is_blacklisted("adult content"));
        assert!(!filter.is_blacklisted("educational video"));
    }

    #[test]
    fn test_is_blacklisted_case_sensitive() {
        let blacklist_content = ".*porn.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(!filter.is_blacklisted("free PORN videos")); // Case sensitive
    }

    #[test]
    fn test_is_blacklisted_case_insensitive_pattern() {
        let blacklist_content = "(?i).*porn.*"; // Case insensitive pattern
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(filter.is_blacklisted("free PORN videos"));
        assert!(filter.is_blacklisted("free Porn videos"));
    }

    #[test]
    fn test_is_whitelisted() {
        let blacklist_content = "";
        let whitelist_content = ".*education.*\n.*medical.*";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(filter.is_whitelisted("sex education video"));
        assert!(filter.is_whitelisted("medical porn documentary"));
        assert!(!filter.is_whitelisted("random video"));
    }

    #[test]
    fn test_blacklist_with_whitelist_override() {
        let blacklist_content = ".*porn.*\n.*adult.*";
        let whitelist_content = ".*education.*\n.*medical.*";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        // Should be blacklisted (matches blacklist, not whitelisted)
        assert!(filter.is_blacklisted("free porn videos"));

        // Should NOT be blacklisted (matches blacklist but also whitelisted)
        assert!(!filter.is_blacklisted("sex education documentary"));
        assert!(!filter.is_blacklisted("medical adult content"));

        // Should NOT be blacklisted (doesn't match blacklist)
        assert!(!filter.is_blacklisted("cooking tutorial"));
    }

    #[test]
    fn test_check_titles_empty_list() {
        let blacklist_content = ".*porn.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(!filter.check_titles(&[]));
    }

    #[test]
    fn test_check_titles_no_matches() {
        let blacklist_content = ".*porn.*\n.*adult.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        let titles = vec![
            "cooking tutorial".to_string(),
            "news update".to_string(),
            "educational video".to_string(),
        ];

        assert!(!filter.check_titles(&titles));
    }

    #[test]
    fn test_check_titles_with_matches() {
        let blacklist_content = ".*porn.*\n.*adult.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        let titles = vec![
            "cooking tutorial".to_string(),
            "free porn videos".to_string(), // This should trigger
            "educational video".to_string(),
        ];

        assert!(filter.check_titles(&titles));
    }

    #[test]
    fn test_check_titles_with_whitelist_override() {
        let blacklist_content = ".*porn.*";
        let whitelist_content = ".*education.*";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        let titles = vec![
            "cooking tutorial".to_string(),
            "sex education documentary".to_string(), // Blacklisted but whitelisted
            "educational video".to_string(),
        ];

        assert!(!filter.check_titles(&titles));
    }

    #[test]
    fn test_complex_regex_patterns() {
        let blacklist_content = r"^.*\b(porn|xxx|adult)\b.*$";
        let whitelist_content = r".*\b(education|medical|documentary)\b.*";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(filter.is_blacklisted("watch xxx videos"));
        assert!(filter.is_blacklisted("adult content here"));
        assert!(!filter.is_blacklisted("appropriate content")); // No word boundary match
        assert!(!filter.is_blacklisted("xxx educational documentary")); // Whitelisted
    }

    #[test]
    fn test_load_patterns_edge_cases() {
        // Test with only whitespace and comments
        let content = "
# Comment only
   
   # Another comment
   
";
        let blacklist_file = create_temp_file_with_content(content);
        let whitelist_file = create_temp_file_with_content("");

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert_eq!(filter.blacklist.len(), 0);
    }

    #[test]
    fn test_multiple_patterns_same_title() {
        let blacklist_content = ".*video.*\n.*content.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        // Should match first pattern and return true
        assert!(filter.is_blacklisted("video content"));
    }

    #[test]
    fn test_empty_title() {
        let blacklist_content = ".*porn.*";
        let whitelist_content = "";

        let blacklist_file = create_temp_file_with_content(blacklist_content);
        let whitelist_file = create_temp_file_with_content(whitelist_content);

        let filter = Filter::new(blacklist_file.path(), whitelist_file.path()).unwrap();

        assert!(!filter.is_blacklisted(""));
        assert!(!filter.is_whitelisted(""));
    }
}
