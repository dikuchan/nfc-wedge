# NFC Wedge — Architecture & Implementation Plan

## Goal
ACR1552U reads NFC text data → types to active foreground window. Background tray app.
Russian GUI. Configurable Enter suffix. Single-shot per tap. Auto-start via installer.

---

## Architecture

### Core Flow

```
[NFC Thread: pcsc polling] → card detected → read bytes → parse NDEF Text → String
                                    ↓
[Main Thread: eframe] ← crossbeam_channel ← String
                                    ↓
[spawn_blocking: enigo] → keystrokes to foreground window
```

### Concurrency Model

- **NFC thread**: Blocking PC/SC. Owns `pcsc::Context`. Polls `get_status_change()`.
  On card present: connect, read, parse, send `String` via channel. Disconnect.
- **Main thread**: `eframe::App::update()` polls channel via `try_recv()`.
  On message: clone string, dispatch to `spawn_blocking` for `enigo` typing.
- **Tray thread**: Optional. Pumps `tray_icon::TrayIconEvent`. Or handled in main thread via `Arc<AtomicBool>` flags checked in `update()`.

### Module Map

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry. Init tracing, config, spawn NFC thread, run `eframe::run_native`. |
| `src/app.rs` | `eframe::App` impl. Russian UI. Channel recv. Wedge trigger. Viewport hide/show. |
| `src/config.rs` | Config struct, load/save JSON to `%APPDATA%\nfc-wedge\config.json`. |
| `src/i18n.rs` | Compile-time JSON map. Russian default. Lookup by key. |
| `src/nfc/mod.rs` | Thread spawn. Public API: `start(cfg) -> (JoinHandle, Sender<Command>)`. |
| `src/nfc/pcsc.rs` | Context lifecycle, list readers, connect, status poll. |
| `src/nfc/tag.rs` | Tag type detection via ATR/ATS/UID heuristics. |
| `src/nfc/apdu.rs` | APDU builders: Type 2 escape `FF B0`, Type 4 SELECT/READ BINARY. |
| `src/nfc/ndef.rs` | Parse NDEF message, extract Text record. Fallback raw UTF-8. |
| `src/wedge.rs` | `enigo` keyboard simulation. `type_text(text, append_enter)`. |
| `src/tray.rs` | `tray-icon`. Context menu: Показать / Выход. HWND hide/show. |
| `src/auto_start.rs` | `auto-launch` crate wrapper. HKCU Run registry toggle. |
| `src/single_shot.rs` | Dedup guard. Hash UID + 2s cooldown. |

### Crate Choices

| Crate | Role |
|-------|------|
| `pcsc` | Native Windows PC/SC. ACR1552U standard compliant. |
| `ndef-rs` | Parse NDEF messages, extract `TextPayload`. |
| `enigo` | Simulate keystrokes via `SendInput`. |
| `tray-icon` | System tray icon + menu. |
| `auto-launch` | HKCU registry auto-start. |
| `crossbeam-channel` | Thread-safe channel NFC → UI. |
| `directories` | Locate `%APPDATA%` for config. |
| `serde` + `serde_json` | Config serialization. |
| `anyhow` | Application error handling. |
| `thiserror` | Library error enums. |
| `tracing` + `tracing-subscriber` | Structured logging. |
| `image` | Generate RGBA placeholder tray icon in memory. |

---

## Expanded Plan (10 Phases)

### Phase 1 — Skeleton & Config

**Goal:** Runnable empty `eframe` window with config load/save and Russian i18n.

**Tasks:**
1. Populate `Cargo.toml` with core deps (`eframe`, `egui`, `serde`, `serde_json`, `directories`, `anyhow`, `thiserror`, `tracing`, `tracing-subscriber`).
2. `src/config.rs`:
   - Define `Config { default_reader: Option<String>, append_enter: bool, language: String }`.
   - `load()` → read `%APPDATA%\nfc-wedge\config.json` or default.
   - `save()` → atomic write to same path.
3. `src/i18n.rs`:
   - `include_str!("../i18n/ru.json")`.
   - Parse to `HashMap<String, String>`.
   - `t(key) -> &str` with key fallback.
4. `src/main.rs`:
   - Init `tracing_subscriber::fmt()`.
   - Load config.
   - `eframe::run_native` with stub `App`.
5. Verify: compiles, window opens, config roundtrip works.

**Expected Result:**
- `cargo test` passes (`config_roundtrip`, `translate_known_key`, `translate_fallback`).
- `cargo run` opens a window titled `Ожидание карты...`.
- No English text visible in UI. All labels Russian.

**Config schema v1:**
```json
{
  "default_reader": "ACS ACR1552U 1",
  "append_enter": true,
  "language": "ru"
}
```

---

### Phase 2 — PC/SC Reader Enumeration

**Goal:** Detect connected ACR1552U, list in GUI dropdown, detect card insert/remove.

**Tasks:**
1. Add `pcsc` to `Cargo.toml`.
2. `src/nfc/pcsc.rs`:
   - `Context::establish(Scope::User)`.
   - `list_readers()` → `Vec<String>`.
   - `Reader::connect(name)` → `Card`.
   - `get_status_change()` with `ReaderState` to detect `State::PRESENT` / `State::EMPTY`.
3. `src/nfc/mod.rs`:
   - `pub enum Command { SetReader(String), Shutdown }`.
   - Thread loop: receive command → update target reader. If no target, wait for command.
   - If target set: poll reader. On `PRESENT`, send `NfcEvent::CardPresent` via channel. On `EMPTY`, send `NfcEvent::CardRemoved`.
4. `src/app.rs`:
   - Dropdown bound to live reader list (refresh every 2s via `ctx.request_repaint_after`).
   - Status label shows "Ожидание карты..." or "Карта обнаружена".
5. Verify: ACR1552U appears in dropdown. Card insert/removal updates status.

**Expected Result:**
- With ACR1552U plugged in, dropdown lists `ACS ACR1552U` (or similar).
- Tap card on reader → status label turns green: `Карта обнаружена`.
- Remove card → status returns to `Ожидание карты...`.
- No crash if reader unplugged while app running.

---

### Phase 3 — Tag Read (Type 2 & Type 4)

**Goal:** Read raw bytes from any NFC tag (Type 2 or Type 4).

**Tasks:**
1. `src/nfc/tag.rs`:
   - Detect tag type from ATR or historical bytes.
   - Heuristics: short ATR + no protocol → Type 2. Long ATR with T=CL → Type 4.
2. `src/nfc/apdu.rs`:
   - **Type 2 escape**: `build_read_binary_escape(page, len)` → `FF B0 00 [page] [len]`.
   - Read first 16 pages (64 bytes) → scan for NDEF TLV header `0x03`.
   - If NDEF TLV found, read contiguous pages until terminator `0xFE` or payload length reached.
   - **Type 4 APDU**:
     - SELECT NDEF application: `00 A4 04 00 07 D2760000850101 00`
     - SELECT NDEF file: `00 A4 00 0C 02 00 E1 00` (or capability container first)
     - READ BINARY: `00 B0 00 00 [len]`
     - Parse response to read full NDEF message length, then read remainder.
3. `src/nfc/mod.rs`:
   - On `CardPresent`: call tag detection → read bytes → send `NfcEvent::Data(Vec<u8>)`.
4. Verify: with real tag, raw bytes appear in logs. Use `tracing::debug!("raw={:02x?}", bytes)`.

**Expected Result:**
- Console / log viewer shows hex dump after card tap, e.g. `[info] raw=[03, 12, D1, 01, ...]`.
- Tag type correctly identified as Type 2 or Type 4 in logs.
- No panic on unsupported tag type. Error gracefully logged.

**Decision:** Since tag write format unknown, we read enough memory to capture NDEF message or raw payload. We attempt NDEF parse next. If that fails, we fall back to raw UTF-8.

---

### Phase 4 — NDEF Parse & Text Extraction

**Goal:** Convert raw bytes to human-readable text string.

**Tasks:**
1. Add `ndef-rs` to `Cargo.toml`.
2. `src/nfc/ndef.rs`:
   - `fn extract_text(data: &[u8]) -> Option<String>`:
     - Try `NdefMessage::decode(data)`.
     - Iterate records. If `TextPayload`, return `payload.text()`.
   - `fn fallback_text(data: &[u8]) -> String`:
     - Trim trailing nulls (`0x00`).
     - `String::from_utf8_lossy()`.
3. `src/nfc/mod.rs`:
   - After reading bytes: `extract_text(&bytes).unwrap_or_else(|| fallback_text(&bytes))`.
   - Send `NfcEvent::Text(String)` to main thread.
4. `src/app.rs`:
   - Display read text in status label. `Прочитано: <текст>`.
5. Verify: tap tag → text appears in UI. Test with phone HCE and physical tags.

**Expected Result:**
- Status label shows `Прочитано: <текст карты>`.
- If card contains NDEF Text → exact text displayed.
- If card contains raw UTF-8 → text displayed without nulls.
- If unreadable → status shows `Ошибка: <причина>`.

---

### Phase 5 — Single-Shot Guard

**Goal:** Card held for 2 seconds = type once. No spam.

**Tasks:**
1. `src/single_shot.rs`:
   - `CooldownGuard { last_uid: Option<Vec<u8>>, last_time: Instant, cooldown: Duration }`
   - `fn allow(&mut self, uid: &[u8]) -> bool`: compare UID, check `elapsed() > cooldown`.
2. `src/nfc/mod.rs`:
   - Extract UID from card (ATS or `SCardStatus`). Use as dedup key.
   - Before sending text: check `cooldown_guard.allow(&uid)`.
   - If denied: log `tracing::debug!("duplicate tap ignored")`, do not send.
3. Verify: hold card → types once. Remove and re-tap within 2s → blocked. Re-tap after 2s → allowed.

**Expected Result:**
- Hold card on reader 3s → only one `Прочитано: ...` entry in log.
- Quick re-tap same card within 2s → no new log entry. `duplicate tap ignored` in debug log.
- Wait 3s, re-tap same card → new read accepted.

---

### Phase 6 — Keyboard Wedge

**Goal:** Read text appears in foreground application (e.g., Notepad).

**Tasks:**
1. Add `enigo` to `Cargo.toml`.
2. `src/wedge.rs`:
   - `pub fn type_text(text: &str, append_enter: bool) -> anyhow::Result<()>`
   - `let mut enigo = Enigo::new(&Settings::default())?;`
   - `enigo.text(text)?;`
   - If `append_enter`: `enigo.key(Key::Return, Click)?;`
3. `src/app.rs`:
   - In `update()`, on `NfcEvent::Text(text)`: spawn `std::thread::spawn_blocking` calling `wedge::type_text(&text, cfg.append_enter)`.
   - Log result. Show "Текст введён" in status.
4. Verify: open Notepad, tap tag → text appears. Check append Enter.

**Expected Result:**
- Open TextEdit (macOS) or Notepad (Windows). Tap tag → text typed into document.
- With `Добавить Enter` enabled → cursor moves to next line after text.
- With checkbox unchecked → text appears, no newline.
- UI remains responsive during typing. No freeze.

**Note:** `enigo` uses `SendInput` on Windows. Must run in blocking thread to not stall UI. Ensure app has foreground focus or Windows may block input. Usually `SendInput` works regardless.

---

### Phase 7 — GUI (Russian)

**Goal:** Full settings panel. All labels in Russian.

**Tasks:**
1. `src/app.rs`:
   - Left panel:
     - `Считыватель:` dropdown (readers + `Обновить` button).
     - `Установить по умолчанию` button → calls `config.save()`.
     - Checkbox `Добавить Enter` → toggles `config.append_enter`, saves.
     - Checkbox `Запускать при входе в Windows` → toggles auto-start (Phase 9).
   - Right panel / bottom:
     - Status label: large, colored. `Ожидание карты...` (gray), `Карта обнаружена` (green), `Ошибка: ...` (red), `Прочитано: ...` (blue).
   - Close button behavior: override in `update()`:
     ```rust
     if ctx.input(|i| i.viewport().close_requested()) {
         ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
         ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
     }
     ```
2. `src/i18n.rs`:
   - Complete `ru.json` with all UI keys.
3. Verify: all controls functional. Config persists across restarts.

**Expected Result:**
- Select reader, click `Установить по умолчанию`, close app, reopen → dropdown pre-selects same reader.
- Toggle `Добавить Enter`, close app, reopen → checkbox state restored.
- No English labels anywhere. All buttons, checkboxes, status text in Russian.

---

### Phase 8 — System Tray

**Goal:** Close button minimizes to tray. Tray menu to show/exit.

**Tasks:**
1. Add `tray-icon`, `winit` (for raw window handle), `image` to `Cargo.toml`.
2. `src/tray.rs`:
   - Generate 16x16 RGBA icon in memory (colored square, no external file).
   - `TrayIconBuilder::new().with_icon(icon).with_menu(menu).build()?`.
   - Menu items: `Показать`, `Выход`.
   - `TrayIconEvent` receiver. On click: set atomic flag `SHOULD_SHOW`.
3. `src/app.rs`:
   - `update()` checks `SHOULD_SHOW.swap(false, SeqCst)`. If true: `ViewportCommand::Visible(true)`.
   - `SHOULD_EXIT` flag for tray Exit menu. If true: allow close.
4. Verify: click X → window hides, tray icon remains. Double-click tray → window shows. Exit menu → process terminates.

**Expected Result:**
- Click window close (X) → window disappears, but `nfc-wedge` still visible in system tray / menu bar.
- Click tray icon → context menu with `Показать` and `Выход`.
- Click `Показать` → window reappears at last position.
- Click `Выход` → process exits fully. Tray icon disappears.

---

### Phase 9 — Auto-Start

**Goal:** App starts when Windows user logs in.

**Tasks:**
1. Add `auto-launch` to `Cargo.toml`.
2. `src/auto_start.rs`:
   - `fn is_enabled() -> bool`
   - `fn enable() -> anyhow::Result<()>`: uses `AutoLaunch::new(app_name, current_exe_path, WindowsEnableMode::CurrentUser, &[])`.
   - `fn disable() -> anyhow::Result<()>`
3. `src/app.rs`:
   - Wire checkbox to `auto_start::enable()` / `disable()`.
   - On startup, read state and set checkbox.
4. Verify: enable checkbox, restart Windows (or check registry key). App launches.

**Expected Result:**
- Check `Запускать при входе в Windows` → config saves, registry (Win) or LaunchAgent (macOS) created.
- Uncheck → entry removed.
- On Windows: `regedit` shows `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\nfc-wedge` pointing to exe path.
- On macOS: `~/Library/LaunchAgents/nfc-wedge.plist` exists when enabled.

**Decision:** MSI install path (Phase 10) gives stable `exe` path. If user moves portable exe, registry path breaks. We'll use MSI.

---

### Phase 10 — Installer (MSI)

**Goal:** One-click install for shop terminals.

**Tasks:**
1. Install `cargo-wix`: `cargo install cargo-wix`.
2. Create `wix/main.wxs`:
   - Product ID, UpgradeCode (fixed GUID for upgrade support).
   - Directory: `ProgramFiles64Folder\nfc-wedge`.
   - Component: `nfc-wedge.exe`, config folder `%APPDATA%\nfc-wedge` (created on first run).
   - Start Menu shortcut.
   - Optional: auto-launch checkbox in MSI UI (or rely on in-app toggle).
3. Build: `cargo wix --nocapture`.
4. Output: `target/wix/nfc-wedge-0.1.0.msi`.
5. Verify: install on clean Windows VM. App appears in Start Menu. Tray works. Uninstall removes files.

**Expected Result:**
- Double-click `.msi` → wizard completes without errors.
- App appears in Start Menu as `nfc-wedge`.
- App launches from Start Menu, tray icon appears, all Phases 1–9 features functional.
- Uninstall from `Add or Remove Programs` → exe removed, Start Menu shortcut removed. Config in `%APPDATA%` optionally preserved.

---

## Key Decisions & Tradeoffs

1. **NDEF vs Raw Fallback**
   - Primary: NDEF Text record (covers 99% of commercial tags and phones).
   - Fallback: raw bytes as UTF-8. If your tags use proprietary text layout, we'll add offset config in v2.

2. **Tray App vs Windows Service**
   - Tray app: simpler, no IPC, works in logged-in user session. Chosen.
   - Service: would require session 0 keyboard injection (forbidden by UIPI). Not viable for wedge.

3. **Keystroke Timing**
   - `enigo::text()` types as fast as OS accepts. If target app drops chars, add `delay_ms` to config v2.

4. **Single-Shot Cooldown**
   - 2 seconds default. Configurable in v2.

5. **Russian Only**
   - i18n map ready for `en` extension if needed later.

---

## UI Architecture — Multi-Page Layout with Log Viewer

### Tabbed Interface

Replace single-panel layout with tabbed navigation. Top tab bar switches pages:

| Tab | Label | Content |
|-----|-------|---------|
| 1 | `Журнал` | Live log stream. Scrollable. |
| 2 | `Настройки` | Reader, default, append Enter, auto-start. |
| 3 | `Вкл/Выкл` | Global enable/disable toggle for NFC polling. |

Implement via `egui::TopBottomPanel::top` holding tab buttons + active tab index in `App` state. `CentralPanel` renders active page content.

### Log Viewer

**Goal:** Show all tracing logs inside app window when expanded from tray.

**Implementation:**
1. Custom `tracing_subscriber::Layer` (`src/log_layer.rs`):
   - Buffers last 500 log lines in `Arc<Mutex<VecDeque<String>>>`.
   - Format: `[timestamp] [level] message`.
2. `src/app.rs` — `Журнал` tab:
   - `ScrollArea::vertical().stick_to_bottom(true)`.
   - Iterate buffer, display as selectable labels.
   - `Очистить` button to drain buffer.
3. Logs must include: PC/SC errors, card detections, wedge results, config saves.
4. `tracing` setup in `main.rs`: `tracing_subscriber::registry().with(fmt::layer()).with(app_log_layer)`. This way logs go to both stderr (for debug) and in-app buffer.

**Note:** Log buffer must be bounded to prevent unbounded memory growth during days of uptime.

### Enable / Disable Toggle

**Goal:** User can pause NFC polling without quitting app.

**Implementation:**
1. `src/app.rs` — `Вкл/Выкл` tab:
   - Large toggle button: `Включить считывание` / `Остановить считывание`.
   - Status: `Статус: Работает` (green) / `Остановлено` (red).
2. State stored in `App { polling_enabled: bool }`.
3. When disabled: send `Command::Pause` to NFC thread. Thread releases PC/SC context or stops polling. Keeps connection alive but ignores cards.
4. When enabled: send `Command::Resume`. Thread reconnects and polls.
5. Persist `polling_enabled` in config? No — default to `true` on startup. User may want it off after reboot, so add `enabled_on_startup: bool` to config schema v2 if needed. For now, always start enabled.

### Window State

- **Minimized to tray:** Window hidden. Logs still accumulate in background buffer.
- **Expanded (shown):** Window visible with last tab active. User sees historical logs since app start.
- **Close button (X):** Hide to tray. Do not prompt.

---

## Cross-Platform Validation (macOS)

**Goal:** Build and validate on macBook before Windows shipping.

### Platform Differences

| Feature | Windows | macOS (Validation only) |
|---------|---------|-------------------------|
| PC/SC | Native `winscard.dll` | `pcsc` crate uses `PCSC.framework`. ACS driver required for ACR1552U. |
| Tray hide/show | `HWND` + `ShowWindow` | `tray-icon` native `NSStatusItem`. No `HWND` needed. |
| Auto-start | HKCU Registry | `auto-launch` crate uses LaunchAgent. Same API call, different backend. |
| Installer | MSI (`cargo-wix`) | None. `cargo run --release` only. |
| Keystroke | `SendInput` | `enigo` uses `CGEventPost`. Same API. |

### Code Changes for Cross-Platform

1. **Conditional compilation:** Gate Windows-specific `HWND` logic.
   ```rust
   #[cfg(target_os = "windows")]
   mod tray_win;
   #[cfg(target_os = "macos")]
   mod tray_mac;
   ```
2. **Reader name format:** macOS reader names may include ` [xxx]` suffix. Reader enumeration and default matching must be fuzzy or exact-but-tolerant.
3. **Auto-start module:** Use `auto-launch` abstracted API. No direct `winreg` calls. The crate handles Windows registry and macOS `~/Library/LaunchAgents` internally.
4. **No MSI on macOS:** Skip Phase 10 on macOS. Validation focuses on Phases 1–9 functionality.

### Validation Workflow (macOS)

1. Install ACS ACR1552U macOS driver.
2. `cargo run --release`.
3. Verify: ACR1552U appears in dropdown.
4. Verify: tap NFC tag → text appears in TextEdit (or any focused macOS app).
5. Verify: tray icon, hide/show, enable/disable, log viewer.
6. Final Windows binary built on Windows CI or machine with `cargo wix`.

**Expected Result (macOS Validation):**
- `cargo run --release` on macBook compiles without errors.
- ACR1552U detected after installing ACS macOS driver.
- Full tap-to-type cycle works in TextEdit.
- Tray icon, logs, enable/disable all functional.
- `cargo wix` skipped on macOS (Windows-only Phase 10).

---

## Success Criteria

- [ ] ACR1552U appears in dropdown.
- [ ] Tap any NFC text tag → text appears in Notepad (or any focused field).
- [ ] Hold card → types exactly once.
- [ ] Close button → tray icon. Exit via tray menu.
- [ ] Auto-start checkbox works after MSI install.
- [ ] All UI in Russian. No raw English visible.
- [ ] `cargo run --release` works on macBook for validation.
