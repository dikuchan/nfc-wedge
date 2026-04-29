# Architecture

This document describes the high-level architecture of `nfc-wedge`. If you want to familiarize yourself with the codebase, you're in the right place.

## Bird's Eye View

`nfc-wedge` is a background Windows tray application that reads NFC NDEF Text records from ACR1552U readers and types the content into the active window via keyboard simulation. Think of it as a barcode scanner, but for NFC tags.

The application has three main responsibilities:
1. **Polling NFC readers** for card presence and reading NDEF data
2. **Presenting a GUI** for configuration and status monitoring  
3. **Simulating keyboard input** to inject the read text

The architecture is event-driven with three independent threads:
- **NFC thread**: Polls PC/SC readers, reads tags, sends events to UI
- **UI thread** (main): Renders GUI, handles user input, manages tray icon
- **System tray**: Runs on main thread, provides minimize-to-tray functionality

Communication between threads happens via lock-free channels (`crossbeam_channel`). The NFC thread never blocks the UI, and UI updates never block NFC polling.

## Code Map

### `src/main.rs`

Entry point. Sets up tracing (console + in-memory log buffer), loads config, initializes event bus, spawns NFC thread, and launches the eframe GUI.

**Architecture Invariant:** The main function is short and linear. All complexity lives in modules.

### `src/config.rs`

Persistent configuration using JSON serialization. Stores:
- Default reader name
- Cooldown duration (milliseconds)
- Typing delay (milliseconds per character)
- Append Enter flag
- Language preference

**Architecture Invariant:** Config is a value type. It knows nothing about the UI or NFC subsystem. Saving is explicit (`config.save()`), never automatic. This ensures the UI controls when I/O happens.

**File location:** `%APPDATA%\nfc-wedge\config.json` on Windows, `~/.config/nfc-wedge/config.json` on macOS.

### `src/i18n.rs`

Internationalization using JSON dictionaries (`i18n/ru.json`). The `I18n` struct loads a language pack and provides `t(key)` lookups.

**Architecture Invariant:** Translation keys are simple strings, not enums. This keeps i18n decoupled from the rest of the codebase. Missing keys fall back to the key itself (fail gracefully).

### `src/event_bus.rs`

Coordination layer between NFC thread and UI thread. Contains:
- `NfcEvent`: Enum of events (readers discovered, card present/removed, text read, errors)
- `NfcEventSender`: Wrapper that auto-wakes the UI thread on critical events
- `EventBus`: Holds the receiver, provides `poll_nfc_events()` for the UI

**Architecture Invariant:** The event bus knows about the UI wake function (via closure), but the NFC thread does not. The NFC thread only knows about `NfcEventSender`, which is a thin wrapper. This keeps `nfc` module free from eframe dependencies.

**Design choice:** We use channels instead of shared state. Events are cheap to clone (strings, small vecs). This avoids locks and makes testing easier (just drain the channel).

### `src/nfc/`

The NFC subsystem. Independent from the UI and runs on a background thread.

#### `src/nfc/mod.rs`

The main polling loop. Responsibilities:
- Process commands from UI (set reader, pause, resume, shutdown)
- Enumerate PC/SC readers every 2 seconds
- Poll selected reader for card presence
- Trigger card reads when a card is detected
- Send events to UI via `NfcEventSender`

**Architecture Invariant:** The NFC thread is stateless w.r.t. the UI. All state (selected reader, pause/resume) is driven by commands from the UI. If the UI restarts, the NFC thread keeps running with the last known state.

**Architecture Invariant:** Polling is continuous. Even when paused, the thread runs (but skips card polling). This keeps reader enumeration active. The thread only exits on `Shutdown` command.

**Threading model:** The thread sleeps for 100ms between iterations to avoid spinning. Reader enumeration is throttled (every 20 iterations = 2 seconds).

#### `src/nfc/pcsc.rs`

PC/SC API wrappers. Thin layer over the `pcsc` crate:
- `establish_context()`: Opens PC/SC context
- `list_readers()`: Enumerates available readers
- `poll_card_present()`: Checks if a card is present (uses `GetStatusChange` with 100ms timeout)
- `connect_card()`: Establishes card connection
- `disconnect_card()`: Releases card connection
- `transmit_apdu()`: Sends APDU commands to the card

**Architecture Invariant:** All PC/SC errors are logged and converted to `anyhow::Error`. The caller decides whether to retry or propagate. We never panic on PC/SC failures (readers can be unplugged, drivers can crash).

#### `src/nfc/tag.rs`

Type 2 tag (NTAG216) specific logic:
- `get_uid()`: Reads card UID using `FF CA 00 00 00` APDU
- `read_tag()`: Reads all pages using `FF B0` escape APDU, parses TLV structure

**Architecture Invariant:** We only support Type 2 tags. Type 4 (ISO-DEP) is explicitly not supported. If you put a Type 4 tag on the reader, the read will fail with "invalid TLV" error.

**TLV parsing:** We scan for `03` (NDEF Message TLV) and extract the payload. The `FE` terminator is optional. We stop at the first NDEF message.

#### `src/nfc/ndef.rs`

NDEF Text record parsing. We use `ndef-rs` crate for basic structure, but manually parse the Text record payload because the library returns the entire payload (including language code prefix).

**Architecture Invariant:** If NDEF parsing fails, we fall back to raw hex dump (first 32 bytes, cleaned of nulls/control chars). This ensures the user sees *something* even if the tag is malformed.

### `src/single_shot.rs`

Cooldown guard. Prevents duplicate reads of the same card.

The `CooldownGuard` tracks `(Vec<u8>, Instant)` pairs. When you call `should_process(uid)`, it checks if the UID was seen recently (within cooldown window). If yes, returns `false`. If no, records the timestamp and returns `true`.

**Architecture Invariant:** The guard uses wall-clock time (`Instant`), not monotonic counters. This means cooldowns are real-time: if you set 2000ms cooldown and tap the card, you must wait 2 real seconds before the next read succeeds.

**Design choice:** We store UIDs as `Vec<u8>` instead of parsing them into a struct. This keeps the guard generic (works with any UID length).

### `src/wedge.rs`

Keyboard wedge using the `enigo` crate.

**Architecture Invariant:** We use a global singleton `Enigo` instance (via `OnceLock<Mutex<Enigo>>`). This avoids reinitializing the platform keyboard API on every read, which is expensive on Windows (COM initialization).

The `type_text(text, delay_ms)` function:
1. Locks the global `Enigo`
2. Iterates over characters
3. Types each character
4. Sleeps for `delay_ms` between characters

**Design choice:** Delay is per-character, not per-keystroke. A 10ms delay means 10ms between 'a' and 'b', not 10ms for 'a' down+up.

**Platform behavior:** On Windows, `Enigo` sends `SendInput` virtual key events. On macOS, it uses `CGEventPost`. Both simulate real keyboard input (not clipboard paste).

### `src/app.rs`

The UI application (`eframe::App` implementation).

**State:**
- Config (mutable, auto-saves on change)
- Event bus (receives NFC events)
- Selected reader (tracks UI dropdown selection)
- Status text + kind (waiting/detected/error)
- Active tab (Settings/Журнал/Вкл-Выкл)
- Polling enabled flag (pause/resume state)
- Tray manager (system tray integration)
- Log buffer (for journal tab)

**Architecture Invariant:** The UI is a pure view. It never performs I/O directly (except `config.save()`). All NFC interactions are via sending commands to the NFC thread and polling events from the event bus.

**Architecture Invariant:** The UI never blocks. Long-running operations (NFC reads) happen on the background thread. The UI just renders the last known state.

**Tabs:**
- **Настройки (Settings):** Reader dropdown, cooldown slider, typing delay slider, append Enter checkbox, auto-start checkbox (Windows only)
- **Журнал (Logs):** Scrollable log viewer with color-coded levels
- **Вкл/Выкл (Toggle):** Pause/resume polling button

**Tray integration:** Clicking the close button (X) hides the window to tray instead of exiting. The tray menu has "Показать" (show) and "Выход" (exit) items.

### `src/tray.rs`

System tray management using `tray-icon` crate.

**Architecture Invariant:** The tray runs on the main thread (required by macOS). We use `MenuEvent::set_event_handler` with atomic flags (`Arc<AtomicBool>`) for event communication. The UI polls these flags on every frame.

**Why atomic flags instead of channels?** The `tray-icon` crate's event handler runs on arbitrary threads. Using channels would require `Send` bounds on the handler closure. Atomic flags are simpler and don't require locks.

**Icon:** We generate a 16x16 green circle PNG in-memory using the `image` crate. No external icon file needed.

### `src/auto_start.rs`

Windows auto-start registry integration using `auto-launch` crate.

**Architecture Invariant:** macOS stubs are no-ops. We explicitly do not touch macOS LaunchAgents to avoid interfering with the user's plist configuration.

**Windows behavior:** Writes to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\nfc-wedge` with the current exe path.

### `src/log_buffer.rs`

Custom `tracing::Layer` that captures log events into a circular buffer (500 entries).

**Architecture Invariant:** The log buffer is a separate concern from logging infrastructure. We use `tracing-subscriber` with multiple layers: one for console output, one for the buffer. This keeps logging decoupled from the UI.

**Thread safety:** The buffer is `Arc<Mutex<VecDeque>>`. The tracing layer is `Clone`, so it can be shared across threads.

**Visitor pattern:** We implement `tracing::field::Visit` to extract message and field data from log events.

## Cross-Cutting Concerns

### Error Handling

**Architecture Invariant:** The NFC subsystem uses `anyhow::Result` everywhere. Errors are logged and converted to `NfcEvent::Error` events. The UI displays errors as red status text.

**No panics in production:** We use `?` propagation and `tracing::error!` instead of `unwrap()`/`expect()`. The only exception is initialization (if we can't load config, we fail fast).

### Testing

Tests are minimal and focused on business logic:
- `config::tests::config_roundtrip`: Config serialization round-trip
- `i18n::tests`: Translation fallback behavior
- `nfc::ndef::tests`: NDEF parsing edge cases
- `nfc::pcsc::tests`: PC/SC context creation (smoke test)
- `single_shot::tests`: Cooldown guard logic (4 tests covering different UID/time scenarios)

**Architecture Invariant:** Tests do not use real NFC readers. All NFC tests are unit tests on pure functions (parsing, cooldown logic).

**Architecture Invariant:** Tests do not depend on external files. All test data is inline strings or fixtures.

### Cancellation

We do not support cancellation. NFC reads are fast (< 100ms) and non-blocking. If the user changes the selected reader mid-read, the old read completes and the result is discarded.

### Performance

The application is designed to be lightweight:
- NFC thread sleeps 100ms between iterations (10 Hz polling rate)
- Reader enumeration is throttled to every 2 seconds
- UI repaints only on events (or every 100ms when hidden, for tray polling)
- Log buffer is capped at 500 entries (~ 50KB of text)

**Memory usage:** Steady state is ~10MB RSS (mostly eframe/egui). No allocations in the hot path (NFC polling loop reuses buffers).

### Observability

We use structured logging (`tracing` crate) with these levels:
- `ERROR`: PC/SC failures, NDEF parse errors, config save failures
- `WARN`: Recoverable issues (e.g., failed to get UID, skipping cooldown check)
- `INFO`: Normal operations (card detected, text read, reader selected)
- `DEBUG`: Verbose tracing (not used in release builds)

All logs go to both console (stdout) and the in-memory buffer (visible in Журнал tab).

### Platform Differences

**Windows:**
- System tray uses `NSStatusItem` via `tray-icon`
- Auto-start uses registry
- Keyboard wedge uses `SendInput` API
- Config stored in `%APPDATA%\nfc-wedge\`

**macOS:**
- System tray uses `NSStatusItem` (menu bar)
- Auto-start is a no-op (user must configure manually)
- Keyboard wedge uses `CGEventPost`
- Config stored in `~/.config/nfc-wedge/`

**Architecture Invariant:** The codebase is cross-platform by default. Windows-specific code is behind `#[cfg(target_os = "windows")]`. The app compiles and runs on macOS (useful for development), but the installer and auto-start are Windows-only.

## Dependency Boundaries

**API Boundaries:**
- `config`, `i18n`, `single_shot`, `wedge`: Pure libraries, no dependencies on UI or NFC
- `nfc`: Depends on `pcsc` crate, independent from UI (only knows about channels)
- `app`: Depends on everything, orchestrates the application

**External dependencies:**
- `eframe`/`egui`: UI framework (main thread only)
- `pcsc`: PC/SC smart card API (background thread only)
- `enigo`: Keyboard simulation (main thread only, via global lock)
- `crossbeam_channel`: Lock-free MPSC channels
- `tracing`/`tracing-subscriber`: Logging infrastructure
- `serde`/`serde_json`: Config serialization
- `tray-icon`: System tray integration
- `auto-launch`: Windows registry auto-start

## Future Directions

Potential improvements (not implemented):
- Support for Type 4 tags (ISO-DEP)
- Multiple card read (batch mode)
- Configurable keyboard shortcuts
- Export logs to file
- Reader auto-selection (prefer last working reader)
- Network mode (read on one machine, type on another)
