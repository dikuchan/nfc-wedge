use crossbeam_channel::{Receiver, Sender};
use eframe::egui;

use crate::config::Config;
use crate::i18n::I18n;
use crate::nfc;

pub struct App {
    config: Config,
    i18n: I18n,
    nfc_cmd: Sender<nfc::Command>,
    nfc_evt: Receiver<nfc::NfcEvent>,
    readers: Vec<String>,
    selected_reader: Option<String>,
    status_text: String,
    status_kind: StatusKind,
    card_present: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum StatusKind {
    Waiting,
    Detected,
    Error,
}

impl App {
    pub fn new(
        config: Config,
        i18n: I18n,
        nfc_cmd: Sender<nfc::Command>,
        nfc_evt: Receiver<nfc::NfcEvent>,
    ) -> Self {
        let status_text = i18n.t("waiting_card");
        Self {
            config,
            i18n,
            nfc_cmd,
            nfc_evt,
            readers: Vec::new(),
            selected_reader: None,
            status_text,
            status_kind: StatusKind::Waiting,
            card_present: false,
        }
    }

    fn poll_nfc(&mut self) {
        while let Ok(evt) = self.nfc_evt.try_recv() {
            match evt {
                nfc::NfcEvent::Readers(list) => {
                    self.readers = list;
                }
                nfc::NfcEvent::CardPresent => {
                    self.card_present = true;
                    self.status_kind = StatusKind::Detected;
                    self.status_text = self.i18n.t("card_detected");
                }
                nfc::NfcEvent::CardRemoved => {
                    self.card_present = false;
                    self.status_kind = StatusKind::Waiting;
                    self.status_text = self.i18n.t("waiting_card");
                }
                nfc::NfcEvent::Data(bytes) => {
                    self.card_present = true;
                    self.status_kind = StatusKind::Detected;
                    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                    self.status_text = format!("{}: {} bytes = [{}]", self.i18n.t("read_text"), bytes.len(), hex);
                }
                nfc::NfcEvent::Error(msg) => {
                    self.status_kind = StatusKind::Error;
                    self.status_text = format!("{}: {}", self.i18n.t("error"), msg);
                }
            }
        }
    }

    fn send_command(&self, cmd: nfc::Command) {
        if let Err(e) = self.nfc_cmd.send(cmd) {
            tracing::warn!("failed to send NFC command: {e}");
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_nfc();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.i18n.t("settings"));

            ui.horizontal(|ui| {
                ui.label(self.i18n.t("reader"));

                let current = self.selected_reader.as_deref().unwrap_or("");
                egui::ComboBox::from_id_salt("reader_dropdown")
                    .width(240.0)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.selected_reader, None, "—");
                        for r in &self.readers {
                            if ui
                                .selectable_value(
                                    &mut self.selected_reader,
                                    Some(r.clone()),
                                    r.as_str(),
                                )
                                .clicked()
                            {
                                self.send_command(nfc::Command::SetReader(r.clone()));
                            }
                        }
                    });

                if ui.button(self.i18n.t("refresh")).clicked() {
                    // Reader list auto-refreshes from thread; manual refresh just triggers repaint.
                    ctx.request_repaint();
                }
            });

            if let Some(ref reader) = self.selected_reader {
                if ui.button(self.i18n.t("set_default")).clicked() {
                    self.config.default_reader = Some(reader.clone());
                    if let Err(e) = self.config.save() {
                        tracing::error!("failed to save config: {e}");
                    }
                }
            }

            ui.separator();

            let (color, label) = match self.status_kind {
                StatusKind::Waiting => (ui.visuals().weak_text_color(), self.status_text.clone()),
                StatusKind::Detected => (egui::Color32::GREEN, self.status_text.clone()),
                StatusKind::Error => (egui::Color32::RED, self.status_text.clone()),
            };

            ui.colored_label(color, label);
        });

        if ctx.input(|i| i.viewport().close_requested()) {
            self.send_command(nfc::Command::Shutdown);
        }
    }
}
