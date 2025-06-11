use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppState {
    pub blocked_until: Option<DateTime<Utc>>,
    pub next_bathroom_break: DateTime<Utc>,
    pub in_bathroom_break: bool,
    pub bathroom_break_until: Option<DateTime<Utc>>,
}

impl AppState {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(AppState::default_with_next_break());
        }
        
        let content = fs::read_to_string(path)?;
        let state: AppState = serde_json::from_str(&content)?;
        Ok(state)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn is_blocked(&self) -> bool {
        if let Some(blocked_until) = self.blocked_until {
            Utc::now() < blocked_until
        } else {
            false
        }
    }

    pub fn is_bathroom_break_time(&self, _interval_hours: u64) -> bool {
        if self.in_bathroom_break {
            if let Some(until) = self.bathroom_break_until {
                return Utc::now() < until;
            }
        }
        Utc::now() >= self.next_bathroom_break
    }

    pub fn block_browser(&mut self, timeout_minutes: u64) {
        self.blocked_until = Some(Utc::now() + chrono::Duration::minutes(timeout_minutes as i64));
    }

    pub fn start_bathroom_break(&mut self, duration_minutes: u64, interval_hours: u64) {
        self.in_bathroom_break = true;
        self.bathroom_break_until = Some(Utc::now() + chrono::Duration::minutes(duration_minutes as i64));
        self.next_bathroom_break = Utc::now() + chrono::Duration::hours(interval_hours as i64);
    }

    pub fn end_bathroom_break(&mut self) {
        self.in_bathroom_break = false;
        self.bathroom_break_until = None;
    }

    fn default_with_next_break() -> Self {
        AppState {
            blocked_until: None,
            next_bathroom_break: Utc::now() + chrono::Duration::hours(3),
            in_bathroom_break: false,
            bathroom_break_until: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        
        assert!(state.blocked_until.is_none());
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
    }

    #[test]
    fn test_app_state_load_nonexistent_file() {
        let state = AppState::load("/nonexistent/path/state.json").unwrap();
        
        assert!(state.blocked_until.is_none());
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
        assert!(state.next_bathroom_break > Utc::now());
    }

    #[test]
    fn test_app_state_save_and_load() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();
        
        let mut original_state = AppState::default();
        original_state.blocked_until = Some(Utc::now() + chrono::Duration::minutes(10));
        original_state.in_bathroom_break = true;
        original_state.bathroom_break_until = Some(Utc::now() + chrono::Duration::minutes(5));
        
        original_state.save(&temp_path).unwrap();
        
        let loaded_state = AppState::load(&temp_path).unwrap();
        
        assert_eq!(original_state.blocked_until, loaded_state.blocked_until);
        assert_eq!(original_state.in_bathroom_break, loaded_state.in_bathroom_break);
        assert_eq!(original_state.bathroom_break_until, loaded_state.bathroom_break_until);
        assert_eq!(original_state.next_bathroom_break, loaded_state.next_bathroom_break);
    }

    #[test]
    fn test_app_state_load_invalid_json() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"invalid json content").unwrap();
        
        let result = AppState::load(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_is_blocked_when_not_blocked() {
        let state = AppState::default();
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_is_blocked_when_blocked_future() {
        let mut state = AppState::default();
        state.blocked_until = Some(Utc::now() + chrono::Duration::minutes(10));
        assert!(state.is_blocked());
    }

    #[test]
    fn test_is_blocked_when_blocked_past() {
        let mut state = AppState::default();
        state.blocked_until = Some(Utc::now() - chrono::Duration::minutes(10));
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_block_browser() {
        let mut state = AppState::default();
        assert!(!state.is_blocked());
        
        state.block_browser(15);
        assert!(state.is_blocked());
        
        if let Some(blocked_until) = state.blocked_until {
            let expected_time = Utc::now() + chrono::Duration::minutes(15);
            let time_diff = (blocked_until - expected_time).num_seconds().abs();
            assert!(time_diff < 2); // Allow 2 seconds difference for test execution time
        } else {
            panic!("blocked_until should be set");
        }
    }

    #[test]
    fn test_is_bathroom_break_time_not_in_break() {
        let mut state = AppState::default();
        state.next_bathroom_break = Utc::now() - chrono::Duration::minutes(1); // Past time
        state.in_bathroom_break = false;
        
        assert!(state.is_bathroom_break_time(3));
    }

    #[test]
    fn test_is_bathroom_break_time_future() {
        let mut state = AppState::default();
        state.next_bathroom_break = Utc::now() + chrono::Duration::minutes(10); // Future time
        state.in_bathroom_break = false;
        
        assert!(!state.is_bathroom_break_time(3));
    }

    #[test]
    fn test_is_bathroom_break_time_currently_in_break() {
        let mut state = AppState::default();
        state.in_bathroom_break = true;
        state.bathroom_break_until = Some(Utc::now() + chrono::Duration::minutes(5));
        
        assert!(state.is_bathroom_break_time(3));
    }

    #[test]
    fn test_is_bathroom_break_time_break_expired() {
        let mut state = AppState::default();
        state.in_bathroom_break = true;
        state.bathroom_break_until = Some(Utc::now() - chrono::Duration::minutes(5));
        
        assert!(!state.is_bathroom_break_time(3));
    }

    #[test]
    fn test_start_bathroom_break() {
        let mut state = AppState::default();
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
        
        state.start_bathroom_break(10, 3);
        
        assert!(state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_some());
        
        if let Some(break_until) = state.bathroom_break_until {
            let expected_time = Utc::now() + chrono::Duration::minutes(10);
            let time_diff = (break_until - expected_time).num_seconds().abs();
            assert!(time_diff < 2); // Allow 2 seconds difference
        }
        
        let expected_next_break = Utc::now() + chrono::Duration::hours(3);
        let time_diff = (state.next_bathroom_break - expected_next_break).num_seconds().abs();
        assert!(time_diff < 2); // Allow 2 seconds difference
    }

    #[test]
    fn test_end_bathroom_break() {
        let mut state = AppState::default();
        state.in_bathroom_break = true;
        state.bathroom_break_until = Some(Utc::now() + chrono::Duration::minutes(5));
        
        state.end_bathroom_break();
        
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
    }

    #[test]
    fn test_default_with_next_break() {
        let state = AppState::default_with_next_break();
        
        assert!(state.blocked_until.is_none());
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
        
        let expected_next_break = Utc::now() + chrono::Duration::hours(3);
        let time_diff = (state.next_bathroom_break - expected_next_break).num_seconds().abs();
        assert!(time_diff < 2); // Allow 2 seconds difference
    }

    #[test]
    fn test_multiple_block_operations() {
        let mut state = AppState::default();
        
        // First block
        state.block_browser(5);
        let first_block = state.blocked_until.unwrap();
        
        // Second block (should overwrite)
        state.block_browser(15);
        let second_block = state.blocked_until.unwrap();
        
        assert!(second_block > first_block);
    }

    #[test]
    fn test_bathroom_break_state_transitions() {
        let mut state = AppState::default();
        
        // Start break
        state.start_bathroom_break(10, 3);
        assert!(state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_some());
        
        // End break
        state.end_bathroom_break();
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
        
        // Start another break
        state.start_bathroom_break(5, 2);
        assert!(state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_some());
    }
}