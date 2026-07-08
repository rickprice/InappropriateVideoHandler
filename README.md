# Inappropriate Video Handler

A Rust daemon that monitors browser window titles against configurable blacklist and whitelist patterns, killing the browser and locking it out when inappropriate content is detected. It also enforces scheduled bathroom breaks.

## Features

- **Browser Title Monitoring**: Watches browser window titles via X11 (active tab per window)
- **Regex-based Filtering**: Configurable blacklist and whitelist using regular expressions
- **Browser Blocking**: Kills the browser and prevents restart for a configurable timeout after a blacklist match
- **Grace Retries**: The first few violations within a rolling window only trigger a short block; exceeding the limit applies a longer hard lock that re-extends on further violations until it fully expires
- **Scheduled Bathroom Breaks**: Enforces periodic breaks at configurable intervals
- **Background Image Management**: Changes the desktop wallpaper to reflect the current state
- **Persistent State**: Block timeouts and break schedules survive system reboots

## Requirements

- Linux with X11
- `feh` for desktop background management
- `pgrep` for process management (typically pre-installed)

## Installation

```bash
cargo build --release
```

## Usage

### Run the monitoring daemon

```bash
./target/release/inappropriate-video-handler
```

### Open the browser (respects block and break state)

```bash
./target/release/inappropriate-video-handler --start-browser
```

Use this command as the browser launcher in your desktop environment instead of calling Chrome directly. It always sets the desktop wallpaper to reflect the current state, and will refuse to open the browser if a block or break is active.

### Custom config file

```bash
./target/release/inappropriate-video-handler -c /path/to/config.yaml
```

---

## Configuration

All configuration lives in `config.yaml` (default location, overridable with `-c`).

```yaml
browser:
  executable: "google-chrome-stable"   # Command used to launch the browser
  url: "https://www.youtube.com"        # URL opened by --start-browser
  process_name: "chrome"               # Process name used to find and kill Chrome

monitoring:
  check_frequency_seconds: 60          # How often the daemon checks window titles

timeouts:
  blacklist_timeout_minutes: 1         # How long the browser is blocked during a grace retry
  bathroom_break_minutes: 4            # Duration of each scheduled break
  bathroom_break_interval_hours: 2     # How often breaks are enforced
  grace_retries: 3                     # Violations allowed before a hard lock is applied
  hard_lock_minutes: 40                # How long the hard lock lasts once grace retries are exhausted
  retry_reset_minutes: 20              # Quiet period (with no violations, while not blocked) needed to reset the count

backgrounds:
  normal: "/path/to/normal.jpg"        # Wallpaper during normal operation
  blocked: "/path/to/blocked.jpg"      # Wallpaper while the browser is blocked
  bathroom_break: "/path/to/break.jpg" # Wallpaper during a scheduled break

files:
  blacklist: "~/.config/inappropriate-video-handler/BlackList.txt"
  whitelist: "~/.config/inappropriate-video-handler/WhiteList.txt"
  state_file: "~/.cache/inappropriate-video-handler/state.json"  # Persists block/break state across reboots
  log_file: "~/.cache/inappropriate-video-handler/ivh.log"       # Optional; uses XDG default if omitted
  titles_file: "~/.cache/inappropriate-video-handler/window-titles.txt"  # Optional; cumulative title log
```

### Configuration reference

| Key | Description | Default |
|-----|-------------|---------|
| `browser.executable` | Path or name of the browser binary | `google-chrome-stable` |
| `browser.url` | URL opened when `--start-browser` is used | `https://www.youtube.com` |
| `browser.process_name` | Process name matched by `pgrep` to kill the browser | `chrome` |

| `monitoring.check_frequency_seconds` | Seconds between each title check | `60` |
| `timeouts.blacklist_timeout_minutes` | Minutes the browser stays blocked during a grace retry | `1` |
| `timeouts.bathroom_break_minutes` | Duration of each break in minutes | `4` |
| `timeouts.bathroom_break_interval_hours` | Hours between scheduled breaks | `2` |
| `timeouts.grace_retries` | Violations allowed within the reset window before a hard lock is applied | `3` |
| `timeouts.hard_lock_minutes` | Minutes the browser stays hard-locked once grace retries are exhausted | `40` |
| `timeouts.retry_reset_minutes` | Minutes of quiet time (not blocked, no new violations) needed to reset the violation count | `20` |
| `backgrounds.normal` | Wallpaper path during normal operation | `$XDG_CONFIG_HOME/inappropriate-video-handler/wallpaper/normal.jpg` |
| `backgrounds.blocked` | Wallpaper path while blocked | `$XDG_CONFIG_HOME/inappropriate-video-handler/wallpaper/blocked.jpg` |
| `backgrounds.bathroom_break` | Wallpaper path during a break | `$XDG_CONFIG_HOME/inappropriate-video-handler/wallpaper/bathroom.jpg` |
| `files.blacklist` | Path to blacklist pattern file | `$XDG_CONFIG_HOME/inappropriate-video-handler/BlackList.txt` |
| `files.whitelist` | Path to whitelist pattern file | `$XDG_CONFIG_HOME/inappropriate-video-handler/WhiteList.txt` |
| `files.state_file` | Path to persistent state JSON file | `$XDG_CACHE_HOME/inappropriate-video-handler/state.json` |
| `files.log_file` | Path to log file | `$XDG_CACHE_HOME/inappropriate-video-handler/ivh.log` |
| `files.titles_file` | Cumulative log of all seen window titles (useful for building pattern lists) | `$XDG_CACHE_HOME/inappropriate-video-handler/window-titles.txt` |

---

## Pattern Files

Both files contain one regex pattern per line. Lines starting with `#` and blank lines are ignored. Patterns are case-sensitive by default; prefix with `(?i)` for case-insensitive matching.

### blacklist.txt

Window titles matching any of these patterns will trigger a block:

```
# Block common adult content keywords
(?i).*\bporn\b.*
(?i).*\badult content\b.*
(?i).*\bxxx\b.*
```

### whitelist.txt

Titles matching these patterns are **never** blocked, even if they also match the blacklist. Use this to protect legitimate content that might otherwise be caught:

```
# Allow educational and medical content
(?i).*education.*
(?i).*medical.*
(?i).*research.*
```

---


## Logging

Control log verbosity with `--log-level`:

```bash
./target/release/inappropriate-video-handler --log-level debug
```

| Level | What you see |
|-------|-------------|
| `error` | Errors only |
| `warn` | Errors and warnings (default) |
| `info` | + startup messages, match hits, title check counts |
| `debug` | + every browser window title and Chrome tab title being checked |
| `trace` | + non-browser windows that were seen and rejected, every regex comparison |

Log output goes to **stderr**. Use `--log-level debug` to verify which window titles are being checked if a match is not firing as expected.

---

## State Machine

The daemon tracks one of four states at any given time.

### States

| State | Condition | Browser | Wallpaper |
|-------|-----------|---------|-----------|
| **Normal** | No active block or break | Allowed to run | `backgrounds.normal` |
| **Grace Blocked** | Blacklist match; `violation_count` ≤ `grace_retries` | Killed; blocked for `blacklist_timeout_minutes` | `backgrounds.blocked` |
| **Hard Locked** | `violation_count` exceeds `grace_retries` | Killed; blocked for `hard_lock_minutes` | `backgrounds.blocked` |
| **Bathroom Break** | Break interval elapsed (only triggered when not already blocked) | Killed; blocked for `bathroom_break_minutes` | `backgrounds.bathroom_break` |

### Transitions

| From | Event | To |
|------|-------|----|
| Normal | Blacklist match (count ≤ `grace_retries`) | Grace Blocked |
| Normal | Blacklist match (count already > `grace_retries`) | Hard Locked |
| Normal | Break interval (`bathroom_break_interval_hours`) elapsed | Bathroom Break |
| Grace Blocked | Another blacklist match that pushes count > `grace_retries` | Hard Locked |
| Grace Blocked | Block timer expires AND `retry_reset_minutes` of quiet time | Normal (count resets) |
| Hard Locked | Another blacklist match while still locked | Hard Locked (timer re-extended) |
| Hard Locked | Block timer expires AND `retry_reset_minutes` of quiet time | Normal (count resets) |
| Bathroom Break | Break duration (`bathroom_break_minutes`) elapsed | Normal |

### Violation counter

The violation count drives the grace/hard-lock distinction. It only resets after `retry_reset_minutes` of unblocked, violation-free time, so waiting out part of a hard lock and re-offending does not downgrade the block back to a short grace window.

| Count | Block applied |
|-------|--------------|
| 1 … `grace_retries` | Short block (`blacklist_timeout_minutes`) |
| > `grace_retries` | Hard lock (`hard_lock_minutes`), re-extended on each additional violation |
| After `retry_reset_minutes` quiet (unblocked) | Counter resets to 0 |

---

## How It Works

1. The daemon starts, loads config, filter patterns, and persisted state, then sets the desktop wallpaper to reflect the current state (Normal, Blocked, or Bathroom Break).
2. Every `check_frequency_seconds` it finds all browser process IDs with `pgrep`.
3. It queries the X11 window tree for windows belonging to those PIDs and collects their titles.
4. Each title is checked against the blacklist. If it matches and is not overridden by the whitelist, the browser is killed and a violation is recorded: within `grace_retries`, a short `blacklist_timeout_minutes` block is applied; beyond that, a `hard_lock_minutes` hard lock is applied (and re-extended on further violations while it's still active) until enough quiet time (`retry_reset_minutes`) passes while unblocked.
5. Separately, if the scheduled break interval has elapsed, the browser is killed and a break is started (provided no block is already active).
6. The desktop wallpaper is updated to reflect the current state.

---

## State Persistence

State is stored as JSON at `files.state_file`. It records:

- When the current block expires
- The current violation count and when the last violation happened (for grace-retry tracking)
- When the next break is due
- Whether a break is currently active and when it ends

This means a block or active break will still be in effect if the machine reboots or the daemon restarts.

---

## Systemd Service

Create `~/.config/systemd/user/ivh.service`:

```ini
[Unit]
Description=Inappropriate Video Handler
After=graphical-session.target

[Service]
Type=simple
ExecStart=/path/to/target/release/inappropriate-video-handler -c /path/to/config.yaml
Restart=always
Environment=DISPLAY=:0

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user enable ivh.service
systemctl --user start ivh.service
```

If using Chrome tab monitoring, make sure Chrome is started with `--remote-debugging-port=9222` before or shortly after the daemon starts.

---

## Testing

```bash
# Unit tests only (no X11 required)
cargo test --lib

# All tests including integration tests
cargo test
```

## License

Licensed under the MIT License. See LICENSE file for details.
