use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppState {
    pub blocked_until: Option<DateTime<Utc>>,
    pub next_bathroom_break: DateTime<Utc>,
    pub in_bathroom_break: bool,
    pub bathroom_break_until: Option<DateTime<Utc>>,
    pub grace_retry_count: u64,
    pub last_violation_at: Option<DateTime<Utc>>,
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

    /// Records a blacklist violation, applying grace-retry logic.
    ///
    /// If the previous violation happened longer ago than `grace_retry_window_minutes`
    /// (and no hard block is currently active), the grace counter is treated as reset
    /// (the user "behaved" long enough). Otherwise the counter accumulates. Once the
    /// count exceeds `max_grace_retries`, a hard block of `hard_timeout_minutes` is
    /// applied. The counter stays pinned above the limit — and any further violation
    /// keeps re-applying the hard block — until the block fully runs out; only
    /// `clear_expired_block` resets it after that point.
    ///
    /// Returns `true` if this violation triggered (or re-applied) a hard block.
    pub fn record_violation(
        &mut self,
        max_grace_retries: u64,
        grace_retry_window_minutes: u64,
        hard_timeout_minutes: u64,
    ) -> bool {
        let now = Utc::now();

        if !self.is_blocked() {
            if let Some(last) = self.last_violation_at {
                if now - last > chrono::Duration::minutes(grace_retry_window_minutes as i64) {
                    self.grace_retry_count = 0;
                }
            }
        }

        self.last_violation_at = Some(now);
        self.grace_retry_count += 1;

        if self.grace_retry_count > max_grace_retries {
            self.block_browser(hard_timeout_minutes);
            true
        } else {
            false
        }
    }

    /// Resets the grace-retry counter once a previously-applied hard block has fully
    /// run out. Must be called before `record_violation` will grant fresh grace retries
    /// after a hard block.
    pub fn clear_expired_block(&mut self) {
        if self.grace_retry_count > 0 && !self.is_blocked() {
            if let Some(blocked_until) = self.blocked_until {
                if Utc::now() >= blocked_until {
                    self.grace_retry_count = 0;
                }
            }
        }
    }

    pub fn start_bathroom_break(&mut self, duration_minutes: u64, interval_hours: u64) {
        self.in_bathroom_break = true;
        self.bathroom_break_until =
            Some(Utc::now() + chrono::Duration::minutes(duration_minutes as i64));
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
            grace_retry_count: 0,
            last_violation_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();

        assert!(state.blocked_until.is_none());
        assert!(!state.in_bathroom_break);
        assert!(state.bathroom_break_until.is_none());
        assert_eq!(state.grace_retry_count, 0);
        assert!(state.last_violation_at.is_none());
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
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let mut original_state = AppState::default();
        original_state.blocked_until = Some(Utc::now() + chrono::Duration::minutes(10));
        original_state.in_bathroom_break = true;
        original_state.bathroom_break_until = Some(Utc::now() + chrono::Duration::minutes(5));

        original_state.save(&temp_path).unwrap();

        let loaded_state = AppState::load(&temp_path).unwrap();

        assert_eq!(original_state.blocked_until, loaded_state.blocked_until);
        assert_eq!(
            original_state.in_bathroom_break,
            loaded_state.in_bathroom_break
        );
        assert_eq!(
            original_state.bathroom_break_until,
            loaded_state.bathroom_break_until
        );
        assert_eq!(
            original_state.next_bathroom_break,
            loaded_state.next_bathroom_break
        );
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
        let time_diff = (state.next_bathroom_break - expected_next_break)
            .num_seconds()
            .abs();
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
        let time_diff = (state.next_bathroom_break - expected_next_break)
            .num_seconds()
            .abs();
        assert!(time_diff < 2); // Allow 2 seconds difference
    }

    #[test]
    fn test_record_violation_within_grace_limit() {
        let mut state = AppState::default();

        // max_grace_retries = 2: first two violations should not hard-block
        assert!(!state.record_violation(2, 10, 15));
        assert!(!state.is_blocked());
        assert_eq!(state.grace_retry_count, 1);

        assert!(!state.record_violation(2, 10, 15));
        assert!(!state.is_blocked());
        assert_eq!(state.grace_retry_count, 2);
    }

    #[test]
    fn test_record_violation_exceeds_grace_limit() {
        let mut state = AppState::default();

        assert!(!state.record_violation(2, 10, 15));
        assert!(!state.record_violation(2, 10, 15));
        // Third violation exceeds the grace limit of 2 -> hard block
        assert!(state.record_violation(2, 10, 15));
        assert!(state.is_blocked());
        // Counter stays pinned above the limit until the block fully expires
        assert_eq!(state.grace_retry_count, 3);
    }

    #[test]
    fn test_record_violation_during_active_block_reapplies_block() {
        let mut state = AppState::default();

        state.record_violation(2, 10, 15);
        state.record_violation(2, 10, 15);
        state.record_violation(2, 10, 15); // triggers hard block
        let first_blocked_until = state.blocked_until.unwrap();

        // A further violation while still blocked should re-apply (extend) the block
        // rather than granting a fresh grace window.
        assert!(state.record_violation(2, 10, 15));
        assert!(state.blocked_until.unwrap() >= first_blocked_until);
        assert_eq!(state.grace_retry_count, 4);
    }

    #[test]
    fn test_clear_expired_block_resets_grace_count() {
        let mut state = AppState::default();

        state.record_violation(2, 10, 15);
        state.record_violation(2, 10, 15);
        state.record_violation(2, 10, 15); // triggers hard block
        assert_eq!(state.grace_retry_count, 3);

        // Block hasn't expired yet -> counter should remain pinned
        state.clear_expired_block();
        assert_eq!(state.grace_retry_count, 3);

        // Simulate the hard block having fully run out
        state.blocked_until = Some(Utc::now() - chrono::Duration::minutes(1));
        state.clear_expired_block();
        assert_eq!(state.grace_retry_count, 0);
    }

    #[test]
    fn test_record_violation_resets_after_quiet_window() {
        let mut state = AppState::default();

        state.record_violation(2, 10, 15);
        state.record_violation(2, 10, 15);
        assert_eq!(state.grace_retry_count, 2);

        // Simulate a violation that happened outside the grace retry window
        state.last_violation_at = Some(Utc::now() - chrono::Duration::minutes(15));

        // Should be treated as a fresh grace window, not a hard block
        assert!(!state.record_violation(2, 10, 15));
        assert!(!state.is_blocked());
        assert_eq!(state.grace_retry_count, 1);
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
