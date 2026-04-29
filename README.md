# NFC Wedge

Background Windows tray application that reads NFC NDEF Text records from ACR1552U readers and types the content into the active window via keyboard simulation.

Think of it as a barcode scanner, but for NFC tags.

## Features

- **NFC Reading**: Supports ACR1552U reader with Type 2 tags (NTAG216)
- **Keyboard Wedge**: Types NFC text into any active window
- **System Tray**: Minimize to tray, runs in background
- **Russian UI**: All interface elements in Russian
- **Configurable**:
  - Cooldown period (0-5000ms) to prevent duplicate reads
  - Typing delay (0-200ms per character)
  - Optional Enter key after text
  - Auto-start on Windows login
- **Logging**: Live log viewer with color-coded levels
- **Pause/Resume**: Temporarily disable NFC polling

## Requirements

- **Windows**: Primary target, MSI installer available
- **macOS**: Runs for development/testing only (no installer)
- **Hardware**: ACR1552U NFC reader
- **Tags**: Type 2 NFC tags (NTAG213/215/216) with NDEF Text records

## Installation (Windows)

### Option 1: MSI Installer (Recommended)

1. Download `nfc-wedge-0.1.0-x86_64.msi` from releases
2. Run the installer (requires administrator privileges)
3. Choose installation directory (default: `C:\Program Files\nfc-wedge`)
4. Optionally add to system PATH
5. Launch from Start Menu: "NFC Wedge"

The installer:
- Creates Start Menu shortcut
- Installs to Program Files
- Supports in-place upgrades
- Uninstall via "Add/Remove Programs"

### Option 2: Portable Binary

1. Download `nfc-wedge.exe` from releases
2. Place in any directory
3. Run directly (no installation required)
4. Config saved to `%APPDATA%\nfc-wedge\config.json`

## Usage

### First Run

1. Launch the application
2. Go to **Настройки (Settings)** tab
3. Select your NFC reader from the dropdown
4. Click **Установить по умолчанию (Set as Default)** to save

### Reading NFC Tags

1. Ensure polling is enabled (green status in **Вкл/Выкл** tab)
2. Click on any text field (browser, notepad, etc.)
3. Place NFC tag on reader
4. Text from tag is typed automatically

### Configuration Options

**Настройки (Settings) Tab:**
- **Читатель (Reader)**: Select NFC reader
- **Задержка повтора (Cooldown)**: Minimum time between reads of same card (ms)
- **Задержка ввода (Typing delay)**: Delay between typed characters (ms)
- **Добавить Enter (Append Enter)**: Press Enter after typing text
- **Автозапуск (Auto-start)**: Launch on Windows startup *(Windows only)*

**Журнал (Logs) Tab:**
- View application logs with timestamps
- Color-coded by level (ERROR/WARN/INFO/DEBUG)
- Circular buffer (last 500 entries)

**Вкл/Выкл (Toggle) Tab:**
- Pause/resume NFC polling
- Green = running, Red = paused

### System Tray

- Click **X** to minimize to tray (doesn't exit)
- Right-click tray icon:
  - **Показать (Show)**: Restore window
  - **Выход (Exit)**: Quit application

## Supported Tag Format

NFC tags must contain NDEF Text records:

```
NDEF Message:
  Record 0:
    Type: Text
    Language: en (or any)
    Text: "Hello, World!"
```

**What happens:**
1. Tag detected → app reads NDEF message
2. Extracts text from first Text record
3. Types text into active window
4. Cooldown prevents re-read for configured duration

**Fallback:** If NDEF parsing fails, the app types a hex dump of the first 32 bytes.

## Configuration File

Settings stored in JSON:

**Windows**: `%APPDATA%\nfc-wedge\config.json`  
**macOS**: `~/.config/nfc-wedge/config.json`

```json
{
  "language": "ru",
  "cooldown_ms": 2000,
  "typing_delay_ms": 10,
  "append_enter": false,
  "default_reader": "ACS ACR1552U 00 00"
}
```

The file is auto-created on first run. Manual edits are preserved (not overwritten by the app).

## Building from Source

### Prerequisites

- Rust 1.70+ (2024 edition)
- Cargo
- On Windows: MSVC toolchain
- On macOS: Xcode command line tools

### Development Build

```bash
git clone https://github.com/dikuchan/nfc-wedge.git
cd nfc-wedge
cargo build
cargo run
```

### Release Build

```bash
cargo build --release
# Binary: target/release/nfc-wedge.exe (Windows)
#         target/release/nfc-wedge (macOS)
```

### Running Tests

```bash
cargo test
```

All tests are unit tests (no hardware required).

## Building the MSI Installer (Windows)

### Prerequisites

1. Install WiX Toolset 3.11+: https://wixtoolset.org/releases/
2. Add WiX `bin` directory to PATH
3. Install cargo-wix:
   ```bash
   cargo install cargo-wix
   ```

### Build Steps

```bash
# 1. Build release binary
cargo build --release

# 2. Generate MSI
cargo wix

# Output: target/wix/nfc-wedge-0.1.0-x86_64.msi
```

### Installer Features

- **Location**: `C:\Program Files\nfc-wedge\bin\nfc-wedge.exe`
- **License**: MIT (shown in installer dialog)
- **Start Menu**: Creates "NFC Wedge" folder with shortcut
- **PATH**: Optionally adds install directory to system PATH
- **Auto-start**: Not enabled by installer (user configures in app)
- **Upgrade support**: In-place upgrades preserve settings
- **Uninstall**: Removes files/shortcuts, preserves config in `%APPDATA%`

### Installation Flow

1. Welcome screen
2. License agreement (MIT)
3. Feature selection:
   - Application (required)
   - PATH environment variable (optional)
4. Installation directory (default: `C:\Program Files\nfc-wedge`)
5. Install
6. Finish

### Technical Details

- **Upgrade GUID**: `DA1EAFE8-E654-4E82-AEB7-39B8A2017A42` (fixed for upgrade support)
- **Install scope**: Per-machine (requires admin)
- **Compression**: Cabinet file embedded in MSI
- **Architecture**: Automatically detects x64 vs x86

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

**High-level overview:**

- **3-thread architecture**: NFC polling thread, UI thread, system tray on main thread
- **Event-driven**: Lock-free channels (`crossbeam_channel`) for thread communication
- **Modular**: Config, i18n, NFC, wedge, UI are independent modules
- **Error handling**: `anyhow::Result` everywhere, no panics in production
- **Testing**: Unit tests for business logic, no hardware dependencies

**Key invariants:**

- NFC thread never blocks UI
- UI never performs I/O (except config save)
- Config is a value type (explicit save, no auto-save)
- Syntax tree (NDEF parsing) is independent from semantics (keyboard typing)

## Troubleshooting

### Reader not detected

- Check reader is plugged in (USB)
- Windows: Ensure PC/SC Smart Card service is running
  - `services.msc` → "Smart Card" → Start
- Try another USB port
- Check `Журнал` tab for error messages

### Card not reading

- Ensure card is NTAG216 (Type 2 tag)
- Check card has NDEF Text record
- Try reading with NFC Tools app (Android/iOS) to verify tag format
- Increase cooldown if card reads repeatedly
- Check `Журнал` tab for NDEF parse errors

### Text not typing

- Ensure an input field is focused (click in a text box)
- Check typing delay isn't too high (try 0ms)
- Disable antivirus/security software temporarily (may block keyboard input)
- Check `Журнал` for errors

### Auto-start not working (Windows)

- Check registry key exists:
  - `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\nfc-wedge`
- Ensure path points to correct executable
- Try toggling auto-start off/on in settings
- Run as administrator if registry access denied

### macOS Issues

- macOS build is for development only (no auto-start, no installer)
- System tray appears in menu bar (top right)
- Keyboard wedge uses `CGEventPost` (may require accessibility permissions)
- Grant "Input Monitoring" permission: System Preferences → Security & Privacy → Privacy → Input Monitoring

## License

MIT License. See [wix/License.rtf](wix/License.rtf) for full text.

## Credits

- **NFC**: Uses PC/SC API via `pcsc` crate
- **UI**: Built with `eframe` and `egui`
- **Keyboard**: `enigo` crate for cross-platform input simulation
- **NDEF**: `ndef-rs` for basic parsing (with custom Text record handling)
- **Tray**: `tray-icon` for system tray integration
- **Installer**: WiX Toolset 3.x

## Contributing

See [AGENTS.md](AGENTS.md) for coding guidelines.

**Key rules:**
- Minimal dependencies, prefer `std`
- No `.unwrap()` outside tests, use `?` propagation
- Error handling: `anyhow` in business logic, `thiserror` for libraries
- Use `tracing` for logging, not `println!`
- Async I/O with `tokio`, but this project is currently sync-only
- Public API must have rustdoc with `# Errors` section

## Support

- **Issues**: https://github.com/dikuchan/nfc-wedge/issues
- **Documentation**: See `ARCHITECTURE.md` for design details
- **Logs**: Check `Журнал` tab for runtime diagnostics
