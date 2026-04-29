# Windows Installer (MSI)

## Overview

The MSI installer is configured using WiX Toolset 3.x and can be built with `cargo wix`.

## Features

### Installation
- **Location**: `C:\Program Files\nfc-wedge\bin\nfc-wedge.exe`
- **License**: MIT (included in installer dialog)
- **Upgrade support**: In-place upgrades supported (preserves settings)

### Start Menu Integration
- Creates folder: "NFC Wedge" in Start Menu
- Shortcut name: "NFC Wedge"
- Description: "NFC NDEF text reader with keyboard wedge"
- Uninstall removes shortcuts automatically

### Optional Features
- **PATH environment variable**: Optionally adds install directory to system PATH (configurable during installation)

### Auto-Start
- Not configured by installer (to avoid being intrusive)
- User can enable in app settings after installation
- Uses Windows registry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

## Building on Windows

### Prerequisites
1. Install WiX Toolset 3.11+: https://wixtoolset.org/releases/
2. Add WiX bin directory to PATH
3. Install cargo-wix: `cargo install cargo-wix`

### Build Steps
```bash
# 1. Build release binary
cargo build --release

# 2. Generate MSI
cargo wix

# Output: target/wix/nfc-wedge-0.1.0-x86_64.msi
```

## Installation Flow

1. Welcome screen
2. License agreement (MIT)
3. Feature selection:
   - Application (required)
   - PATH environment variable (optional)
4. Installation directory selection (default: `C:\Program Files\nfc-wedge`)
5. Install
6. Finish

## Uninstallation

- Via "Add/Remove Programs" control panel
- Removes all files, shortcuts, and registry entries
- Preserves user config in `%APPDATA%\nfc-wedge\config.json`

## Technical Details

- **Upgrade GUID**: `DA1EAFE8-E654-4E82-AEB7-39B8A2017A42` (fixed for upgrade support)
- **Install scope**: Per-machine (requires admin)
- **Compression**: Cabinet file embedded in MSI
- **Architecture**: Automatically detects x64 vs x86 based on target
