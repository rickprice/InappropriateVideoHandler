# Inappropriate Video Handler

A Rust daemon that monitors browser window titles against configurable blacklist and whitelist patterns, killing the browser and locking it out when inappropriate content is detected. It also enforces scheduled bathroom breaks.

## Features

- **Browser Title Monitoring**: Watches browser window titles via X11 (active tab per window)
- **Regex-based Filtering**: Configurable blacklist and whitelist using regular expressions
- **Browser Blocking**: Kills the browser and prevents restart for a configurable timeout after a blacklist match
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
  blacklist_timeout_minutes: 10        # How long the browser is blocked after a match
  bathroom_break_minutes: 10           # Duration of each scheduled break
  bathroom_break_interval_hours: 3     # How often breaks are enforced

backgrounds:
  normal: "/path/to/normal.jpg"        # Wallpaper during normal operation
  blocked: "/path/to/blocked.jpg"      # Wallpaper while the browser is blocked
  bathroom_break: "/path/to/break.jpg" # Wallpaper during a scheduled break

files:
  blacklist: "~/.config/inappropriate-video-handler/BlackList.txt"
  whitelist: "~/.config/inappropriate-video-handler/WhiteList.txt"
  state_file: "/tmp/ivh_state.json"    # Persists block/break state across reboots
```

### Configuration reference

| Key | Description | Default |
|-----|-------------|---------|
| `browser.executable` | Path or name of the browser binary | `google-chrome-stable` |
| `browser.url` | URL opened when `--start-browser` is used | `https://www.youtube.com` |
| `browser.process_name` | Process name matched by `pgrep` to kill the browser | `chrome` |

| `monitoring.check_frequency_seconds` | Seconds between each title check | `60` |
| `timeouts.blacklist_timeout_minutes` | Minutes the browser stays blocked after a match | `10` |
| `timeouts.bathroom_break_minutes` | Duration of each break in minutes | `10` |
| `timeouts.bathroom_break_interval_hours` | Hours between scheduled breaks | `3` |
| `backgrounds.normal` | Wallpaper path during normal operation | — |
| `backgrounds.blocked` | Wallpaper path while blocked | — |
| `backgrounds.bathroom_break` | Wallpaper path during a break | — |
| `files.blacklist` | Path to blacklist pattern file | — |
| `files.whitelist` | Path to whitelist pattern file | — |
| `files.state_file` | Path to persistent state JSON file | `/tmp/ivh_state.json` |

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

## How It Works

1. The daemon starts, loads config, filter patterns, and persisted state, then sets the desktop wallpaper to reflect the current state (normal, blocked, or bathroom break).
2. Every `check_frequency_seconds` it finds all Chrome process IDs with `pgrep`.
3. It queries the X11 window tree for windows belonging to those PIDs and collects their titles.
4. If `remote_debugging_port` is set, it also fetches all tab titles from Chrome's debug API.
5. Each title is checked against the blacklist. If it matches and is not overridden by the whitelist, the browser is killed and a block timeout is written to the state file.
6. Separately, if the scheduled break interval has elapsed, the browser is killed and a break is started regardless of what was open.
7. The desktop wallpaper is updated to reflect the current state.

---

## State Persistence

State is stored as JSON at `files.state_file`. It records:

- When the current block expires
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
