# Build Instructions

## Windows MSI Installer

### Prerequisites

1. Install WiX Toolset 3.11 or later:
   - Download from https://wixtoolset.org/releases/
   - Add WiX bin directory to PATH

2. Install cargo-wix:
   ```bash
   cargo install cargo-wix
   ```

### Building the MSI

1. Build release binary:
   ```bash
   cargo build --release
   ```

2. Create MSI installer:
   ```bash
   cargo wix
   ```

The MSI will be generated in `target/wix/nfc-wedge-0.1.0-x86_64.msi`.

### Installer Features

- Installs to `C:\Program Files\nfc-wedge\bin\`
- Creates Start Menu shortcut: "NFC Wedge"
- Optional: Adds install directory to system PATH
- Auto-start can be configured in app settings (Windows registry)

## Development Build

```bash
cargo build
cargo run
```

## Running Tests

```bash
cargo test
```
