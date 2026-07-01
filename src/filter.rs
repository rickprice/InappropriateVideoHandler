use anyhow::Result;
use log::{debug, info, trace, warn};
use regex::Regex;
use std::fs;
use std::path::Path;

pub struct Filter {
    blacklist: Vec<Regex>,
    whitelist: Vec<Regex>,
}

impl Filter {
    pub fn new<P: AsRef<Path>>(blacklist_path: P, whitelist_path: P) -> Result<Self> {
        let blacklist = Self::load_patterns(blacklist_path, "blacklist")?;
        let whitelist = Self::load_patterns(whitelist_path, "whitelist")?;

        info!("Filter: {} blacklist pattern(s), {} whitelist pattern(s)",
            blacklist.len(), whitelist.len());

        Ok(Filter { blacklist, whitelist })
    }

    fn load_patterns<P: AsRef<Path>>(path: P, label: &str) -> Result<Vec<Regex>> {
        if !path.as_ref().exists() {
            info!("{} file '{}' not found, using empty pattern list",
                label, path.as_ref().display());
            return Ok(Vec::new());
        }

        info!("Loading {} patterns from '{}'", label, path.as_ref().display());

        let content = fs::read_to_string(path)?;
        let mut patterns = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                match Regex::new(line) {
                    Ok(regex) => {
                        trace!("Loaded {} pattern: '{}'", label, line);
                        patterns.push(regex);
                    }
                    Err(e) => warn!("Invalid regex pattern '{}': {}", line, e),
                }
            }
        }

        Ok(patterns)
    }

    pub fn blacklist_len(&self) -> usize {
        self.blacklist.len()
    }

    pub fn whitelist_len(&self) -> usize {
        self.whitelist.len()
    }

    #[allow(dead_code)]
    pub fn is_blacklisted(&self, title: &str) -> bool {
        debug!("Checking title: '{}'", title);
        for pattern in &self.blacklist {
            let matched = pattern.is_match(title);
            trace!("  Blacklist pattern '{}': {}",
                pattern.as_str(), if matched { "MATCH" } else { "no match" });
            if matched {
                let whitelisted = self.is_whitelisted(title);
                debug!("  Blacklist match for '{}', whitelisted={}", title, whitelisted);
                if !whitelisted {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_whitelisted(&self, title: &str) -> bool {
        for pattern in &self.whitelist {
            let matched = pattern.is_match(title);
            trace!("  Whitelist pattern '{}': {}",
                pattern.as_str(), if matched { "MATCH" } else { "no match" });
            if matched {
                return true;
            }
        }
        false
    }

    /// Returns the first (title, pattern_string) pair that is blacklisted, or None.
    pub fn find_blacklisted_title(&self, titles: &[String]) -> Option<(String, String)> {
        info!("find_blacklisted_title: checking {} title(s)", titles.len());
        for title in titles {
            debug!("  Checking: '{}'", title);
            for pattern in &self.blacklist {
                let matched = pattern.is_match(title);
                trace!("  '{}' vs pattern '{}': {}",
                    title, pattern.as_str(), if matched { "MATCH" } else { "no match" });
                if matched && !self.is_whitelisted(title) {
                    info!("Blacklist hit: title='{}' pattern='{}'",
                        title, pattern.as_str());
                    return Some((title.clone(), pattern.as_str().to_string()));
                }
            }
        }
        debug!("No blacklisted titles found");
        None
    }

    #[allow(dead_code)]
    pub fn check_titles(&self, titles: &[String]) -> bool {
        self.find_blacklisted_title(titles).is_some()
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

    fn make_filter(blacklist: &str, whitelist: &str) -> Filter {
        let bl = create_temp_file_with_content(blacklist);
        let wl = create_temp_file_with_content(whitelist);
        Filter::new(bl.path(), wl.path()).unwrap()
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

        assert_eq!(filter.blacklist.len(), 2);
    }

    #[test]
    fn test_is_blacklisted_simple_match() {
        let filter = make_filter(".*porn.*\n.*adult.*", "");

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(filter.is_blacklisted("adult content"));
        assert!(!filter.is_blacklisted("educational video"));
    }

    #[test]
    fn test_is_blacklisted_case_sensitive() {
        let filter = make_filter(".*porn.*", "");

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(!filter.is_blacklisted("free PORN videos"));
    }

    #[test]
    fn test_is_blacklisted_case_insensitive_pattern() {
        let filter = make_filter("(?i).*porn.*", "");

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(filter.is_blacklisted("free PORN videos"));
        assert!(filter.is_blacklisted("free Porn videos"));
    }

    #[test]
    fn test_is_whitelisted() {
        let filter = make_filter("", ".*education.*\n.*medical.*");

        assert!(filter.is_whitelisted("sex education video"));
        assert!(filter.is_whitelisted("medical porn documentary"));
        assert!(!filter.is_whitelisted("random video"));
    }

    #[test]
    fn test_blacklist_with_whitelist_override() {
        let filter = make_filter(".*porn.*\n.*adult.*", ".*education.*\n.*medical.*");

        assert!(filter.is_blacklisted("free porn videos"));
        assert!(!filter.is_blacklisted("sex education documentary"));
        assert!(!filter.is_blacklisted("medical adult content"));
        assert!(!filter.is_blacklisted("cooking tutorial"));
    }

    #[test]
    fn test_check_titles_empty_list() {
        let filter = make_filter(".*porn.*", "");
        assert!(!filter.check_titles(&[]));
    }

    #[test]
    fn test_check_titles_no_matches() {
        let filter = make_filter(".*porn.*\n.*adult.*", "");

        let titles = vec![
            "cooking tutorial".to_string(),
            "news update".to_string(),
            "educational video".to_string(),
        ];

        assert!(!filter.check_titles(&titles));
    }

    #[test]
    fn test_check_titles_with_matches() {
        let filter = make_filter(".*porn.*\n.*adult.*", "");

        let titles = vec![
            "cooking tutorial".to_string(),
            "free porn videos".to_string(),
            "educational video".to_string(),
        ];

        assert!(filter.check_titles(&titles));
    }

    #[test]
    fn test_check_titles_with_whitelist_override() {
        let filter = make_filter(".*porn.*", ".*education.*");

        let titles = vec![
            "cooking tutorial".to_string(),
            "sex education documentary".to_string(),
            "educational video".to_string(),
        ];

        assert!(!filter.check_titles(&titles));
    }

    #[test]
    fn test_complex_regex_patterns() {
        let filter = make_filter(
            r"^.*\b(porn|xxx|adult)\b.*$",
            r".*\b(education|medical|documentary)\b.*",
        );

        assert!(filter.is_blacklisted("watch xxx videos"));
        assert!(filter.is_blacklisted("adult content here"));
        assert!(!filter.is_blacklisted("appropriate content"));
        assert!(!filter.is_blacklisted("xxx educational documentary"));
    }

    #[test]
    fn test_load_patterns_edge_cases() {
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
        let filter = make_filter(".*video.*\n.*content.*", "");

        assert!(filter.is_blacklisted("video content"));
    }

    #[test]
    fn test_empty_title() {
        let filter = make_filter(".*porn.*", "");

        assert!(!filter.is_blacklisted(""));
        assert!(!filter.is_whitelisted(""));
    }

    #[test]
    fn test_blacklist_len_whitelist_len() {
        let filter = make_filter(".*porn.*\n.*adult.*", ".*education.*");
        assert_eq!(filter.blacklist_len(), 2);
        assert_eq!(filter.whitelist_len(), 1);
    }

    #[test]
    fn test_blacklist_len_whitelist_len_empty() {
        let filter = make_filter("", "");
        assert_eq!(filter.blacklist_len(), 0);
        assert_eq!(filter.whitelist_len(), 0);
    }

    #[test]
    fn test_find_blacklisted_title_returns_match_info() {
        let filter = make_filter(".*porn.*\n.*adult.*", "");

        let titles = vec![
            "cooking tutorial".to_string(),
            "free porn videos".to_string(),
            "educational video".to_string(),
        ];

        let result = filter.find_blacklisted_title(&titles);
        assert!(result.is_some());
        let (title, pattern) = result.unwrap();
        assert_eq!(title, "free porn videos");
        assert_eq!(pattern, ".*porn.*");
    }

    #[test]
    fn test_find_blacklisted_title_no_match() {
        let filter = make_filter(".*porn.*", "");

        let titles = vec![
            "cooking tutorial".to_string(),
            "news update".to_string(),
        ];

        assert!(filter.find_blacklisted_title(&titles).is_none());
    }

    #[test]
    fn test_find_blacklisted_title_empty_titles() {
        let filter = make_filter(".*porn.*", "");
        assert!(filter.find_blacklisted_title(&[]).is_none());
    }

    #[test]
    fn test_find_blacklisted_title_first_match_returned() {
        let filter = make_filter(".*porn.*", "");

        let titles = vec![
            "free porn videos".to_string(),
            "more porn content".to_string(),
        ];

        let (title, _) = filter.find_blacklisted_title(&titles).unwrap();
        assert_eq!(title, "free porn videos");
    }

    #[test]
    fn test_find_blacklisted_title_whitelist_override() {
        let filter = make_filter(".*porn.*", ".*education.*");

        let titles = vec!["sex education documentary".to_string()];

        assert!(filter.find_blacklisted_title(&titles).is_none());
    }

    #[test]
    fn test_find_blacklisted_title_pattern_string_correct() {
        let filter = make_filter(r"(?i).*\bxxx\b.*", "");

        let titles = vec!["Watch XXX Videos".to_string()];
        let (title, pattern) = filter.find_blacklisted_title(&titles).unwrap();
        assert_eq!(title, "Watch XXX Videos");
        assert_eq!(pattern, r"(?i).*\bxxx\b.*");
    }

    #[test]
    fn test_check_titles_consistent_with_find_blacklisted() {
        let filter = make_filter(".*porn.*\n.*adult.*", ".*education.*");

        let cases: &[(&[&str], bool)] = &[
            (&["cooking tutorial", "news"], false),
            (&["free porn videos"], true),
            (&["sex education documentary"], false),
            (&[], false),
        ];

        for (raw, expected) in cases {
            let titles: Vec<String> = raw.iter().map(|s| s.to_string()).collect();
            assert_eq!(filter.check_titles(&titles), *expected, "titles={:?}", raw);
            assert_eq!(filter.find_blacklisted_title(&titles).is_some(), *expected, "titles={:?}", raw);
        }
    }
}
