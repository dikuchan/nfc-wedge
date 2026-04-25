use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use std::thread;
use std::time::Duration;

pub mod apdu;
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
    Data(Vec<u8>),
    Error(String),
}

/// Spawn NFC worker thread. Returns (handle, command_sender, event_receiver).
pub fn start() -> Result<(thread::JoinHandle<()>, Sender<Command>, Receiver<NfcEvent>)> {
    let (cmd_tx, cmd_rx) = crossbeam_channel::bounded(16);
    let (evt_tx, evt_rx) = crossbeam_channel::bounded(64);

    let handle = thread::spawn(move || run(cmd_rx, evt_tx));

    Ok((handle, cmd_tx, evt_rx))
}

/// Retry connect up to 5 times with 200ms delay.
/// Returns Ok on successful read, Err only after retries exhausted.
fn try_connect_and_read(
    ctx: &::pcsc::Context,
    reader: &str,
    evt_tx: &Sender<NfcEvent>,
) -> anyhow::Result<()> {
    for attempt in 1..=5 {
        match pcsc::connect_card(ctx, reader) {
            Ok(card) => {
                tracing::info!("connected to card on {reader}, attempt {attempt}");
                match tag::read_tag(&card) {
                    Ok(data) => {
                        tracing::info!("read {} bytes from card", data.len());
                        let _ = evt_tx.send(NfcEvent::Data(data));
                    }
                    Err(e) => {
                        tracing::warn!("read failed on attempt {attempt}: {e}");
                        if attempt == 5 {
                            return Err(e);
                        }
                    }
                }
                if let Err(e) = pcsc::disconnect_card(card) {
                    tracing::warn!("disconnect error: {e}");
                }
                return Ok(());
            }
            Err(::pcsc::Error::NoSmartcard) => {
                tracing::info!(
                    "card not ready on attempt {}/5, retrying in 200ms",
                    attempt
                );
                if attempt < 5 {
                    thread::sleep(Duration::from_millis(200));
                }
            }
            Err(e) => {
                tracing::warn!("connect error on attempt {attempt}: {e}");
                return Err(anyhow::anyhow!("{e}"));
            }
        }
    }
    Err(anyhow::anyhow!(
        "card not ready after 5 attempts (approx 1s)"
    ))
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
    let mut card_read = false;

    loop {
        // Drain commands without blocking
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Command::SetReader(name) => {
                    tracing::info!("reader selected: {name}");
                    selected_reader = Some(name);
                    was_present = false;
                    card_read = false;
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
                        // Card needs time to power up and negotiate protocol.
                        thread::sleep(Duration::from_millis(300));
                    }
                    if !card_read {
                        if let Err(e) = try_connect_and_read(&ctx, reader, &evt_tx) {
                            tracing::warn!("failed to read card: {e}");
                            let _ = evt_tx.send(NfcEvent::Error(format!("{e}")));
                        }
                        card_read = true;
                    }
                }
                Ok(false) => {
                    if was_present {
                        tracing::info!("card removed from {reader}");
                        let _ = evt_tx.send(NfcEvent::CardRemoved);
                        was_present = false;
                        card_read = false;
                    }
                }
                Err(::pcsc::Error::Timeout) => {}
                Err(e) => {
                    tracing::warn!("poll error on {reader}: {e}");
                    let _ = evt_tx.send(NfcEvent::Error(format!("{reader}: {e}")));
                    selected_reader = None;
                    was_present = false;
                    card_read = false;
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
