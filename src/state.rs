use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// The daemon's current phase. Exactly one variant is active at a time, so
/// "blocked" and "on a bathroom break" can no longer disagree across call
/// sites the way independent `Option`/`bool` fields could.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum AppPhase {
    #[default]
    Normal,
    GraceBlocked { until: DateTime<Utc> },
    HardLocked { until: DateTime<Utc> },
    BathroomBreak { until: DateTime<Utc> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub phase: AppPhase,
    pub next_bathroom_break: DateTime<Utc>,
    #[serde(default)]
    pub violation_count: u32,
    #[serde(default)]
    pub violation_window_start: Option<DateTime<Utc>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            phase: AppPhase::Normal,
            next_bathroom_break: Utc::now() + chrono::Duration::hours(2),
            violation_count: 0,
            violation_window_start: None,
        }
    }
}

impl AppState {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(AppState::default());
        }

        let content = fs::read_to_string(path)?;
        let state: AppState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Loads only if the state file's mtime differs from `last_modified`,
    /// to avoid a read+parse on every daemon tick when nothing changed
    /// since the previous one. Returns `None` when the file is unchanged
    /// (or still absent) and the cached state can keep being used.
    pub fn load_if_changed<P: AsRef<Path>>(
        path: P,
        last_modified: Option<std::time::SystemTime>,
    ) -> Result<Option<(Self, std::time::SystemTime)>> {
        let path = path.as_ref();
        let mtime = match fs::metadata(path).and_then(|m| m.modified()) {
            Ok(mtime) => mtime,
            Err(_) => {
                return if last_modified.is_none() {
                    Ok(Some((AppState::default(), std::time::SystemTime::UNIX_EPOCH)))
                } else {
                    Ok(None)
                };
            }
        };

        if Some(mtime) == last_modified {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        let state: AppState = serde_json::from_str(&content)?;
        Ok(Some((state, mtime)))
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn is_blocked(&self) -> bool {
        match self.phase {
            AppPhase::GraceBlocked { until } | AppPhase::HardLocked { until } => Utc::now() < until,
            _ => false,
        }
    }

    pub fn is_in_bathroom_break(&self) -> bool {
        matches!(self.phase, AppPhase::BathroomBreak { until } if Utc::now() < until)
    }

    pub fn is_bathroom_break_time(&self) -> bool {
        if let AppPhase::BathroomBreak { until } = self.phase {
            return Utc::now() < until;
        }
        Utc::now() >= self.next_bathroom_break
    }

    pub fn blocked_until(&self) -> Option<DateTime<Utc>> {
        match self.phase {
            AppPhase::GraceBlocked { until } | AppPhase::HardLocked { until } => Some(until),
            _ => None,
        }
    }

    pub fn bathroom_break_until(&self) -> Option<DateTime<Utc>> {
        match self.phase {
            AppPhase::BathroomBreak { until } => Some(until),
            _ => None,
        }
    }

    pub fn block_browser(&mut self, timeout_minutes: u64) {
        self.phase = AppPhase::GraceBlocked {
            until: Utc::now() + chrono::Duration::minutes(timeout_minutes as i64),
        };
    }

    /// Records a blacklist violation and applies grace-retry logic.
    ///
    /// Violations accumulate as long as they keep happening within
    /// `retry_reset_minutes` of the previous one. Within `grace_retries`, only a
    /// short `grace_timeout_minutes` block is applied. Once the count exceeds
    /// `grace_retries`, a `hard_lock_minutes` block is applied instead. The count
    /// is deliberately *not* reset when the hard lock is applied: a further
    /// violation while that block is still active re-applies (extends) it rather
    /// than falling back to a fresh grace window, so waiting out part of a hard
    /// lock and reoffending doesn't downgrade the block. The count only resets
    /// once enough quiet time has passed while not blocked.
    ///
    /// Returns `true` if this violation triggered (or re-applied) the hard lock.
    pub fn record_violation(
        &mut self,
        grace_retries: u32,
        retry_reset_minutes: u64,
        grace_timeout_minutes: u64,
        hard_lock_minutes: u64,
    ) -> bool {
        let now = Utc::now();

        if let Some(last) = self.violation_window_start {
            let elapsed = now - last;
            if elapsed.num_minutes() >= retry_reset_minutes as i64 && !self.is_blocked() {
                self.violation_count = 0;
            }
        }

        self.violation_window_start = Some(now);
        self.violation_count += 1;

        if self.violation_count > grace_retries {
            self.phase = AppPhase::HardLocked {
                until: now + chrono::Duration::minutes(hard_lock_minutes as i64),
            };
            true
        } else {
            self.block_browser(grace_timeout_minutes);
            false
        }
    }

    pub fn start_bathroom_break(&mut self, duration_minutes: u64, interval_hours: u64) {
        self.phase = AppPhase::BathroomBreak {
            until: Utc::now() + chrono::Duration::minutes(duration_minutes as i64),
        };
        self.next_bathroom_break = Utc::now() + chrono::Duration::hours(interval_hours as i64);
    }

    pub fn end_bathroom_break(&mut self) {
        self.phase = AppPhase::Normal;
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

        assert_eq!(state.phase, AppPhase::Normal);
        assert!(!state.is_blocked());
        assert!(!state.is_in_bathroom_break());
    }

    #[test]
    fn test_app_state_load_nonexistent_file() {
        let state = AppState::load("/nonexistent/path/state.json").unwrap();

        assert_eq!(state.phase, AppPhase::Normal);
        assert!(!state.is_blocked());
        assert!(!state.is_in_bathroom_break());
        assert!(state.next_bathroom_break > Utc::now());
    }

    #[test]
    fn test_app_state_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let original_state = AppState {
            phase: AppPhase::BathroomBreak {
                until: Utc::now() + chrono::Duration::minutes(5),
            },
            ..Default::default()
        };

        original_state.save(&temp_path).unwrap();

        let loaded_state = AppState::load(&temp_path).unwrap();

        assert_eq!(original_state.phase, loaded_state.phase);
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
    fn test_load_if_changed_skips_unchanged_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();
        AppState::default().save(&temp_path).unwrap();

        let (_state, mtime) = AppState::load_if_changed(&temp_path, None)
            .unwrap()
            .expect("first load should return Some");

        let second = AppState::load_if_changed(&temp_path, Some(mtime)).unwrap();
        assert!(second.is_none(), "unchanged file should not be reloaded");
    }

    #[test]
    fn test_load_if_changed_reloads_after_write() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();
        AppState::default().save(&temp_path).unwrap();

        let (_state, mtime) = AppState::load_if_changed(&temp_path, None)
            .unwrap()
            .expect("first load should return Some");

        std::thread::sleep(std::time::Duration::from_millis(10));
        let mut state = AppState::default();
        state.block_browser(5);
        state.save(&temp_path).unwrap();

        let reloaded = AppState::load_if_changed(&temp_path, Some(mtime)).unwrap();
        assert!(reloaded.is_some(), "changed file should be reloaded");
        assert!(reloaded.unwrap().0.is_blocked());
    }

    #[test]
    fn test_is_blocked_when_not_blocked() {
        let state = AppState::default();
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_is_blocked_when_blocked_future() {
        let state = AppState {
            phase: AppPhase::GraceBlocked {
                until: Utc::now() + chrono::Duration::minutes(10),
            },
            ..Default::default()
        };
        assert!(state.is_blocked());
    }

    #[test]
    fn test_is_blocked_when_blocked_past() {
        let state = AppState {
            phase: AppPhase::GraceBlocked {
                until: Utc::now() - chrono::Duration::minutes(10),
            },
            ..Default::default()
        };
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_block_browser() {
        let mut state = AppState::default();
        assert!(!state.is_blocked());

        state.block_browser(15);
        assert!(state.is_blocked());

        if let Some(blocked_until) = state.blocked_until() {
            let expected_time = Utc::now() + chrono::Duration::minutes(15);
            let time_diff = (blocked_until - expected_time).num_seconds().abs();
            assert!(time_diff < 2); // Allow 2 seconds difference for test execution time
        } else {
            panic!("blocked_until should be set");
        }
    }

    #[test]
    fn test_is_bathroom_break_time_not_in_break() {
        let state = AppState {
            next_bathroom_break: Utc::now() - chrono::Duration::minutes(1), // Past time
            phase: AppPhase::Normal,
            ..Default::default()
        };

        assert!(state.is_bathroom_break_time());
    }

    #[test]
    fn test_is_bathroom_break_time_future() {
        let state = AppState {
            next_bathroom_break: Utc::now() + chrono::Duration::minutes(10), // Future time
            phase: AppPhase::Normal,
            ..Default::default()
        };

        assert!(!state.is_bathroom_break_time());
    }

    #[test]
    fn test_is_bathroom_break_time_currently_in_break() {
        let state = AppState {
            phase: AppPhase::BathroomBreak {
                until: Utc::now() + chrono::Duration::minutes(5),
            },
            ..Default::default()
        };

        assert!(state.is_bathroom_break_time());
    }

    #[test]
    fn test_is_bathroom_break_time_break_expired() {
        let state = AppState {
            phase: AppPhase::BathroomBreak {
                until: Utc::now() - chrono::Duration::minutes(5),
            },
            ..Default::default()
        };

        assert!(!state.is_bathroom_break_time());
    }

    #[test]
    fn test_start_bathroom_break() {
        let mut state = AppState::default();
        assert!(!state.is_in_bathroom_break());
        assert!(state.bathroom_break_until().is_none());

        state.start_bathroom_break(10, 3);

        assert!(state.is_in_bathroom_break());
        assert!(state.bathroom_break_until().is_some());

        if let Some(break_until) = state.bathroom_break_until() {
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
        let mut state = AppState {
            phase: AppPhase::BathroomBreak {
                until: Utc::now() + chrono::Duration::minutes(5),
            },
            ..Default::default()
        };

        state.end_bathroom_break();

        assert!(!state.is_in_bathroom_break());
        assert_eq!(state.phase, AppPhase::Normal);
    }

    #[test]
    fn test_default_next_break_is_two_hours_out() {
        let state = AppState::default();

        assert_eq!(state.phase, AppPhase::Normal);

        let expected_next_break = Utc::now() + chrono::Duration::hours(2);
        let time_diff = (state.next_bathroom_break - expected_next_break)
            .num_seconds()
            .abs();
        assert!(time_diff < 2);
    }

    #[test]
    fn test_record_violation_within_grace_limit() {
        let mut state = AppState::default();

        // grace_retries = 2: first two violations should not hard-lock
        assert!(!state.record_violation(2, 20, 10, 40));
        assert!(state.is_blocked()); // short grace block still applied
        assert_eq!(state.violation_count, 1);

        assert!(!state.record_violation(2, 20, 10, 40));
        assert_eq!(state.violation_count, 2);
    }

    #[test]
    fn test_record_violation_exceeds_grace_limit() {
        let mut state = AppState::default();

        assert!(!state.record_violation(2, 20, 10, 40));
        assert!(!state.record_violation(2, 20, 10, 40));
        // Third violation exceeds the grace limit of 2 -> hard lock
        assert!(state.record_violation(2, 20, 10, 40));
        assert!(state.is_blocked());
        assert_eq!(state.violation_count, 3);
        assert!(matches!(state.phase, AppPhase::HardLocked { .. }));

        let blocked_until = state.blocked_until().unwrap();
        let expected = Utc::now() + chrono::Duration::minutes(40);
        assert!((blocked_until - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_record_violation_during_active_hard_lock_reapplies_lock() {
        let mut state = AppState::default();

        state.record_violation(2, 20, 10, 40);
        state.record_violation(2, 20, 10, 40);
        state.record_violation(2, 20, 10, 40); // triggers hard lock
        let first_blocked_until = state.blocked_until().unwrap();

        // A further violation while still hard-locked should re-apply (extend)
        // the lock rather than resetting to a fresh grace window.
        assert!(state.record_violation(2, 20, 10, 40));
        assert!(state.blocked_until().unwrap() >= first_blocked_until);
        assert_eq!(state.violation_count, 4);
    }

    #[test]
    fn test_record_violation_resets_after_quiet_window() {
        let mut state = AppState::default();

        state.record_violation(2, 20, 10, 40);
        state.record_violation(2, 20, 10, 40);
        assert_eq!(state.violation_count, 2);

        // Simulate the grace block having expired and enough quiet time passing
        state.phase = AppPhase::GraceBlocked {
            until: Utc::now() - chrono::Duration::minutes(1),
        };
        state.violation_window_start = Some(Utc::now() - chrono::Duration::minutes(25));

        // Should be treated as a fresh grace window, not a hard lock
        assert!(!state.record_violation(2, 20, 10, 40));
        assert_eq!(state.violation_count, 1);
    }

    #[test]
    fn test_record_violation_does_not_reset_while_still_blocked() {
        let mut state = AppState::default();

        state.record_violation(2, 20, 10, 40);
        state.record_violation(2, 20, 10, 40);
        state.record_violation(2, 20, 10, 40); // hard lock, count = 3

        // Simulate enough elapsed time for the reset window, but the hard lock
        // is still active -> count must stay pinned, not reset.
        state.violation_window_start = Some(Utc::now() - chrono::Duration::minutes(25));
        assert!(state.is_blocked());

        assert!(state.record_violation(2, 20, 10, 40));
        assert_eq!(state.violation_count, 4);
    }

    #[test]
    fn test_multiple_block_operations() {
        let mut state = AppState::default();

        // First block
        state.block_browser(5);
        let first_block = state.blocked_until().unwrap();

        // Second block (should overwrite)
        state.block_browser(15);
        let second_block = state.blocked_until().unwrap();

        assert!(second_block > first_block);
    }

    #[test]
    fn test_bathroom_break_state_transitions() {
        let mut state = AppState::default();

        // Start break
        state.start_bathroom_break(10, 3);
        assert!(state.is_in_bathroom_break());
        assert!(state.bathroom_break_until().is_some());

        // End break
        state.end_bathroom_break();
        assert!(!state.is_in_bathroom_break());
        assert!(state.bathroom_break_until().is_none());

        // Start another break
        state.start_bathroom_break(5, 2);
        assert!(state.is_in_bathroom_break());
        assert!(state.bathroom_break_until().is_some());
    }
}
