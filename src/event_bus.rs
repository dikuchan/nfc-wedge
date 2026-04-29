use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;

use crate::nfc::NfcEvent;

/// Central event coordination layer.
/// Manages communication between business logic threads (NFC) and UI thread.
pub struct EventBus {
    nfc_rx: Receiver<NfcEvent>,
    wake_fn: Arc<dyn Fn() + Send + Sync>,
}

impl EventBus {
    /// Creates a new EventBus with a wake callback.
    /// The wake callback will be invoked when events that need immediate UI update arrive.
    pub fn new(wake_fn: impl Fn() + Send + Sync + 'static) -> (Self, NfcEventSender) {
        let (nfc_tx, nfc_rx) = crossbeam_channel::bounded(64);
        let wake = Arc::new(wake_fn);
        
        let bus = Self {
            nfc_rx,
            wake_fn: wake.clone(),
        };
        
        let sender = NfcEventSender {
            tx: nfc_tx,
            wake,
        };
        
        (bus, sender)
    }
    
    /// Poll NFC events. Returns an iterator that drains all pending events.
    pub fn poll_nfc_events(&self) -> impl Iterator<Item = NfcEvent> + '_ {
        std::iter::from_fn(|| self.nfc_rx.try_recv().ok())
    }
}

/// Sender handle for NFC events.
/// Automatically triggers UI wake when sending events that need repaint.
#[derive(Clone)]
pub struct NfcEventSender {
    tx: Sender<NfcEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NfcEventSender {
    /// Send an NFC event to the UI thread.
    /// If the event requires immediate repaint, the wake callback is triggered.
    pub fn send(&self, evt: NfcEvent) {
        let needs_wake = evt.needs_repaint();
        let _ = self.tx.send(evt);
        if needs_wake {
            (self.wake)();
        }
    }
}
