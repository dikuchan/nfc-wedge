use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::event_bus::NfcEventSender;
use crate::single_shot::CooldownGuard;

pub mod apdu;
pub mod ndef;
pub mod pcsc;
pub mod tag;

/// Commands sent from UI to NFC thread.
pub enum Command {
    SetReader(String),
    Pause,
    Resume,
    Shutdown,
}

/// Events sent from NFC thread to UI.
pub enum NfcEvent {
    Readers(Vec<String>),
    CardPresent,
    CardRemoved,
    Text(String),
    Error(String),
}

impl NfcEvent {
    /// Returns true if this event should trigger an immediate UI repaint.
    pub fn needs_repaint(&self) -> bool {
        matches!(
            self,
            Self::CardPresent | Self::CardRemoved | Self::Text(_) | Self::Error(_)
        )
    }
}

/// Spawn NFC worker thread. Returns (handle, command_sender).
pub fn start(event_sender: NfcEventSender, cooldown_ms: u64) -> Result<(thread::JoinHandle<()>, Sender<Command>)> {
    let (cmd_tx, cmd_rx) = crossbeam_channel::bounded(16);

    let handle = thread::spawn(move || run(cmd_rx, event_sender, cooldown_ms));

    Ok((handle, cmd_tx))
}

fn try_connect_and_read(
    p_ctx: &::pcsc::Context,
    reader: &str,
    evt_tx: &NfcEventSender,
    guard: &mut CooldownGuard,
) -> anyhow::Result<()> {
    for attempt in 1..=5 {
        match pcsc::connect_card(p_ctx, reader) {
            Ok(card) => {
                tracing::info!("connected to card on {reader}, attempt {attempt}");
                
                // Get UID for deduplication
                let uid = match tag::get_uid(&card) {
                    Ok(uid) => uid,
                    Err(e) => {
                        tracing::warn!("failed to get UID: {e}, skipping cooldown check");
                        vec![]
                    }
                };
                
                // Check cooldown guard
                if !uid.is_empty() && !guard.should_process(&uid) {
                    tracing::debug!("card read blocked by cooldown");
                    let _ = pcsc::disconnect_card(card);
                    return Ok(());
                }
                
                match tag::read_tag(&card) {
                    Ok(data) => {
                        tracing::info!("read {} bytes from card", data.len());
                        let text = ndef::extract_text(&data)
                            .unwrap_or_else(|| ndef::fallback_text(&data));
                        evt_tx.send(NfcEvent::Text(text));
                    }
                    Err(e) => {
                        tracing::warn!("read failed on attempt {attempt}: {e}");
                        if attempt == 5 {
                            evt_tx.send(NfcEvent::Error(e.to_string()));
                            return Err(e);
                        }
                    }
                }
                let _ = pcsc::disconnect_card(card);
                return Ok(());
            }
            Err(::pcsc::Error::NoSmartcard) => {
                if attempt < 5 {
                    thread::sleep(Duration::from_millis(200));
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("connect error: {e}"));
            }
        }
    }
    Err(anyhow::anyhow!("card not ready after retries"))
}

fn run(cmd_rx: Receiver<Command>, evt_tx: NfcEventSender, cooldown_ms: u64) {
    let p_ctx = match pcsc::establish() {
        Ok(c) => c,
        Err(e) => {
            evt_tx.send(NfcEvent::Error(format!("PC/SC context failed: {e}")));
            return;
        }
    };

    let mut guard = CooldownGuard::new(Duration::from_millis(cooldown_ms));
    let mut selected_reader: Option<String> = None;
    let mut was_present = false;
    let mut card_read = false;
    let mut reader_list_counter = 0u32;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Command::SetReader(name) => {
                    selected_reader = Some(name);
                    was_present = false;
                    card_read = false;
                }
                Command::Pause => selected_reader = None,
                Command::Resume => {}
                Command::Shutdown => return,
            }
        }

        if let Some(ref reader) = selected_reader {
            match pcsc::poll_card_present(&p_ctx, reader) {
                Ok(true) => {
                    if !was_present {
                        evt_tx.send(NfcEvent::CardPresent);
                        was_present = true;
                        thread::sleep(Duration::from_millis(600));
                    }
                    if !card_read {
                        if let Err(e) = try_connect_and_read(&p_ctx, reader, &evt_tx, &mut guard) {
                            evt_tx.send(NfcEvent::Error(e.to_string()));
                        }
                        card_read = true;
                    }
                }
                Ok(false) => {
                    if was_present {
                        thread::sleep(Duration::from_millis(100));
                        if let Ok(false) = pcsc::poll_card_present(&p_ctx, reader) {
                            tracing::info!("card removed from {reader}");
                            evt_tx.send(NfcEvent::CardRemoved);
                            was_present = false;
                            card_read = false;
                        }
                    }
                }
                Err(::pcsc::Error::Timeout) => {}
                Err(e) => {
                    evt_tx.send(NfcEvent::Error(format!("{reader}: {e}")));
                    selected_reader = None;
                }
            }
        }
        
        // Enumerate readers periodically (every 2 seconds)
        reader_list_counter += 1;
        if reader_list_counter >= 20 {
            reader_list_counter = 0;
            if let Ok(readers) = pcsc::list_readers(&p_ctx) {
                if !readers.is_empty() {
                    evt_tx.send(NfcEvent::Readers(readers));
                }
            }
        }
        
        thread::sleep(Duration::from_millis(100));
    }
}
