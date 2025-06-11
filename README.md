# Inappropriate Video Handler

A Rust program that monitors window titles against configurable blacklist and whitelist patterns, managing browser access based on content detection and scheduled breaks.

## Features

- **Window Title Monitoring**: Continuously monitors all window titles using X11
- **Regex-based Filtering**: Uses configurable blacklist and whitelist regex patterns
- **Browser Process Management**: Can start and kill browser processes
- **Persistent State**: Maintains timeout state across system reboots
- **Background Image Management**: Changes desktop wallpaper using `feh` based on current state
- **Scheduled Bathroom Breaks**: Enforces periodic breaks at configurable intervals
- **YAML Configuration**: Fully configurable via YAML file

## Requirements

- Linux system with X11
- `feh` for background image management
- `pgrep` for process management (usually pre-installed)

## Installation

1. Clone the repository
2. Build with Cargo:
   ```bash
   cargo build --release
   ```

## Configuration

Edit `config.yaml` to customize behavior:

```yaml
browser:
  executable: "firefox"           # Browser executable path
  url: "https://www.google.com"   # Default URL to open
  process_name: "firefox"         # Process name for killing

monitoring:
  check_frequency_seconds: 60     # How often to check window titles

timeouts:
  blacklist_timeout_minutes: 10   # How long to block after blacklist match
  bathroom_break_minutes: 10      # Duration of bathroom breaks
  bathroom_break_interval_hours: 3 # How often to enforce breaks

backgrounds:
  normal: "/path/to/normal.jpg"        # Normal state background
  blocked: "/path/to/blocked.jpg"      # Blocked state background
  bathroom_break: "/path/to/break.jpg" # Break time background

files:
  blacklist: "blacklist.txt"      # Blacklist patterns file
  whitelist: "whitelist.txt"      # Whitelist patterns file
  state_file: "/tmp/ivh_state.json" # Persistent state file
```

## Pattern Files

### blacklist.txt
Contains regex patterns that trigger browser blocking:
```
.*[Pp]orn.*
.*[Aa]dult.*
.*inappropriate.*
```

### whitelist.txt
Contains regex patterns that override blacklist matches:
```
.*[Ee]ducation.*
.*[Mm]edical.*
.*research.*
```

## Usage

### Start Browser
```bash
./target/release/inappropriate-video-handler --start-browser
```

### Run Monitoring Daemon
```bash
./target/release/inappropriate-video-handler --daemon
```

### Custom Config File
```bash
./target/release/inappropriate-video-handler -c custom-config.yaml --daemon
```

## How It Works

1. **Window Monitoring**: The daemon continuously scans all window titles
2. **Pattern Matching**: Compares titles against blacklist patterns
3. **Whitelist Override**: Checks if blacklisted content is whitelisted
4. **Browser Management**: Kills browser processes when inappropriate content is detected
5. **Timeout Enforcement**: Prevents browser restart until timeout expires
6. **Background Updates**: Changes wallpaper to reflect current state
7. **Bathroom Breaks**: Enforces periodic breaks independent of content detection

## State Persistence

The program maintains state in a JSON file (configurable location) that persists:
- Blacklist timeout end time
- Next scheduled bathroom break time
- Current bathroom break status

This ensures restrictions remain active across system reboots.

## Background States

- **Normal**: Default wallpaper when browser can be used
- **Blocked**: Displayed when browser is blocked due to inappropriate content
- **Bathroom Break**: Shown during scheduled break periods

## Systemd Service (Optional)

Create `/etc/systemd/system/ivh.service`:
```ini
[Unit]
Description=Inappropriate Video Handler
After=graphical-session.target

[Service]
Type=simple
User=yourusername
WorkingDirectory=/path/to/inappropriate-video-handler
ExecStart=/path/to/inappropriate-video-handler/target/release/inappropriate-video-handler --daemon
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

## Security Notes

- The program requires X11 access to monitor window titles
- Process management requires appropriate permissions
- Background image changes require `feh` and display access

## Testing

The project includes extensive unit tests covering all components:

### Running Tests

```bash
# Run all unit tests
cargo test --lib

# Run specific module tests
cargo test config::tests
cargo test state::tests 
cargo test filter::tests
cargo test browser::tests
cargo test background::tests
```

### Test Coverage

- **Config Module**: 10 tests covering YAML loading, validation, and error handling
- **State Module**: 17 tests covering persistence, timeout logic, and state transitions  
- **Filter Module**: 18 tests covering regex patterns, blacklist/whitelist logic, and edge cases
- **Browser Module**: 12 tests covering process management and browser lifecycle
- **Background Module**: 10 tests covering feh integration and error handling

**Total: 67 unit tests with comprehensive coverage**

### Test Features

- **Isolated Testing**: Each module tested independently with mocked dependencies
- **Edge Case Coverage**: Invalid inputs, missing files, empty data, and error conditions
- **Integration Scenarios**: End-to-end workflows testing component interactions
- **Cross-platform Safety**: Tests avoid system dependencies where possible
- **Concurrent Testing**: Thread-safe operations validated with `serial_test`

## License

Licensed under the MIT License. See LICENSE file for details.
