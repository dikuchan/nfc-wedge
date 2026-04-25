use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use std::thread;
use std::time::Duration;

pub mod pcsc;

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
    Error(String),
}

/// Spawn NFC worker thread. Returns (handle, command_sender, event_receiver).
pub fn start() -> Result<(thread::JoinHandle<()>, Sender<Command>, Receiver<NfcEvent>)> {
    let (cmd_tx, cmd_rx) = crossbeam_channel::bounded(16);
    let (evt_tx, evt_rx) = crossbeam_channel::bounded(64);

    let handle = thread::spawn(move || run(cmd_rx, evt_tx));

    Ok((handle, cmd_tx, evt_rx))
}

fn run(cmd_rx: Receiver<Command>, evt_tx: Sender<NfcEvent>) {
    let ctx = match pcsc::establish() {
        Ok(c) => c,
        Err(e) => {
            let _ = evt_tx.send(NfcEvent::Error(format!("PC/SC context failed: {e}")));
            return;
        }
    };

    let mut selected_reader: Option<String> = None;
    let mut was_present = false;

    loop {
        // Drain commands without blocking
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Command::SetReader(name) => {
                    tracing::info!("reader selected: {name}");
                    selected_reader = Some(name);
                    was_present = false;
                }
                Command::Pause => {
                    tracing::info!("polling paused");
                    selected_reader = None;
                }
                Command::Resume => {
                    tracing::info!("polling resumed");
                }
                Command::Shutdown => {
                    tracing::info!("NFC thread shutting down");
                    return;
                }
            }
        }

        if let Some(ref reader) = selected_reader {
            match pcsc::poll_card_present(&ctx, reader) {
                Ok(true) => {
                    if !was_present {
                        tracing::info!("card present on {reader}");
                        let _ = evt_tx.send(NfcEvent::CardPresent);
                        was_present = true;
                    }
                }
                Ok(false) => {
                    if was_present {
                        tracing::info!("card removed from {reader}");
                        let _ = evt_tx.send(NfcEvent::CardRemoved);
                        was_present = false;
                    }
                }
                Err(::pcsc::Error::Timeout) => {}
                Err(e) => {
                    tracing::warn!("poll error on {reader}: {e}");
                    let _ = evt_tx.send(NfcEvent::Error(format!("{reader}: {e}")));
                    selected_reader = None;
                    was_present = false;
                }
            }
        } else {
            match pcsc::list_readers(&ctx) {
                Ok(readers) => {
                    if !readers.is_empty() {
                        let _ = evt_tx.send(NfcEvent::Readers(readers.clone()));
                    }
                }
                Err(e) => {
                    tracing::warn!("list readers error: {e}");
                    let _ = evt_tx.send(NfcEvent::Error(format!("list readers: {e}")));
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}
