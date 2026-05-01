use crossbeam_channel::Sender;
use eframe::egui;

use crate::config::Config;
use crate::event_bus::EventBus;
use crate::i18n::I18n;
use crate::log_buffer::LogBuffer;
use crate::nfc;
use crate::tray::TrayManager;

pub struct App {
    config: Config,
    i18n: I18n,
    nfc_cmd: Sender<nfc::Command>,
    event_bus: EventBus,
    readers: Vec<String>,
    selected_reader: Option<String>,
    status_text: String,
    status_kind: StatusKind,
    active_tab: Tab,
    polling_enabled: bool,
    tray: Option<TrayManager>,
    should_exit: bool,
    #[cfg(target_os = "windows")]
    auto_start_enabled: bool,
    log_buffer: LogBuffer,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Logs,
    Settings,
    Toggle,
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
        event_bus: EventBus,
        log_buffer: LogBuffer,
        wake_fn: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let status_text = i18n.t("waiting_card");
        let selected_reader = config.default_reader.clone();
        
        // Send default reader to NFC thread if configured
        if let Some(ref reader) = selected_reader {
            let _ = nfc_cmd.send(nfc::Command::SetReader(reader.clone()));
        }
        
        // Create tray icon
        let tray = match TrayManager::new(&i18n.t("show"), &i18n.t("exit"), wake_fn) {
            Ok(tray) => Some(tray),
            Err(e) => {
                tracing::error!("failed to create tray icon: {e}");
                None
            }
        };
        
        Self {
            config,
            i18n,
            nfc_cmd,
            event_bus,
            readers: Vec::new(),
            selected_reader,
            status_text,
            status_kind: StatusKind::Waiting,
            active_tab: Tab::Settings,
            polling_enabled: true,
            tray,
            should_exit: false,
            #[cfg(target_os = "windows")]
            auto_start_enabled: crate::auto_start::is_enabled().unwrap_or(false),
            log_buffer,
        }
    }

    fn poll_nfc(&mut self) {
        for evt in self.event_bus.poll_nfc_events() {
            match evt {
                nfc::NfcEvent::Readers(list) => {
                    self.readers = list;
                }
                nfc::NfcEvent::CardPresent => {
                    self.status_kind = StatusKind::Detected;
                    self.status_text = self.i18n.t("card_detected");
                }
                nfc::NfcEvent::CardRemoved => {
                    self.status_kind = StatusKind::Waiting;
                    self.status_text = self.i18n.t("waiting_card");
                }
                nfc::NfcEvent::Text(text) => {
                    self.status_kind = StatusKind::Detected;
                    self.status_text = format!("{}: {}", self.i18n.t("read_text"), text);
                    
                    // Spawn blocking task to type text into active window
                    let text_clone = text.clone();
                    let delay = self.config.typing_delay_ms;
                    let append_enter = self.config.append_enter;
                    std::thread::spawn(move || {
                        if let Err(e) = crate::wedge::type_text(&text_clone, delay, append_enter) {
                            tracing::error!("keyboard wedge failed: {e}");
                        } else {
                            tracing::info!("typed {} characters", text_clone.len());
                        }
                    });
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
    
    fn poll_tray(&mut self, ctx: &egui::Context) {
        if let Some(ref tray) = self.tray {
            let (show, exit) = tray.poll_events();
            
            if show {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            
            if exit {
                self.should_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll NFC events and tray events
        // Note: Tray events wake us up via request_repaint() in the event handler
        self.poll_nfc();
        self.poll_tray(ctx);

        // Top panel with tabs
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Settings, self.i18n.t("settings"));
                ui.selectable_value(&mut self.active_tab, Tab::Logs, self.i18n.t("logs"));
                ui.selectable_value(&mut self.active_tab, Tab::Toggle, self.i18n.t("enable_disable"));
            });
        });

        // Content panel based on active tab
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                Tab::Settings => self.render_settings_tab(ui, ctx),
                Tab::Logs => self.render_logs_tab(ui),
                Tab::Toggle => self.render_toggle_tab(ui),
            }
        });

        // Handle close button: hide to tray instead of exit
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.should_exit {
                self.send_command(nfc::Command::Shutdown);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }
}

impl App {
    fn render_settings_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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
                ctx.request_repaint();
            }
        });

        if let Some(ref reader) = self.selected_reader
            && ui.button(self.i18n.t("set_default")).clicked() {
            self.config.default_reader = Some(reader.clone());
            if let Err(e) = self.config.save() {
                tracing::error!("failed to save config: {e}");
            }
        }

        ui.separator();

        // Cooldown slider
        ui.horizontal(|ui| {
            ui.label(self.i18n.t("cooldown_ms"));
            if ui.add(egui::Slider::new(&mut self.config.cooldown_ms, 0..=5000)).changed()
                && let Err(e) = self.config.save()
            {
                tracing::error!("failed to save config: {e}");
            }
        });

        // Typing delay slider
        ui.horizontal(|ui| {
            ui.label(self.i18n.t("typing_delay_ms"));
            if ui.add(egui::Slider::new(&mut self.config.typing_delay_ms, 0..=200)).changed()
                && let Err(e) = self.config.save()
            {
                tracing::error!("failed to save config: {e}");
            }
        });

        // Append Enter checkbox
        if ui.checkbox(&mut self.config.append_enter, self.i18n.t("append_enter")).changed()
            && let Err(e) = self.config.save()
        {
            tracing::error!("failed to save config: {e}");
        }

        ui.separator();

        // Auto-start checkbox (Windows only)
        #[cfg(target_os = "windows")]
        {
            if ui.checkbox(&mut self.auto_start_enabled, self.i18n.t("auto_start")).changed() {
                let result = if self.auto_start_enabled {
                    crate::auto_start::enable()
                } else {
                    crate::auto_start::disable()
                };
                
                if let Err(e) = result {
                    tracing::error!("failed to update auto-start: {e}");
                    self.auto_start_enabled = !self.auto_start_enabled;
                }
            }
        }
        
        ui.separator();

        // Status display
        let color = match self.status_kind {
            StatusKind::Waiting => ui.visuals().weak_text_color(),
            StatusKind::Detected => egui::Color32::GREEN,
            StatusKind::Error => egui::Color32::RED,
        };

        ui.colored_label(color, &self.status_text);
    }

    fn render_logs_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.i18n.t("logs"));
        
        ui.separator();
        
        let logs = self.log_buffer.get_all();
        
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                
                for entry in &logs {
                    let level_color = match entry.level.as_str() {
                        "ERROR" => egui::Color32::RED,
                        "WARN" => egui::Color32::YELLOW,
                        "INFO" => egui::Color32::LIGHT_BLUE,
                        "DEBUG" => egui::Color32::LIGHT_GRAY,
                        _ => egui::Color32::WHITE,
                    };
                    
                    ui.horizontal(|ui| {
                        ui.label(&entry.timestamp);
                        ui.colored_label(level_color, format!("[{}]", entry.level));
                        ui.label(&entry.message);
                    });
                }
            });
    }

    fn render_toggle_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.i18n.t("enable_disable"));

        ui.add_space(20.0);

        let button_text = if self.polling_enabled {
            self.i18n.t("disable_polling")
        } else {
            self.i18n.t("enable_polling")
        };

        if ui.button(button_text).clicked() {
            self.polling_enabled = !self.polling_enabled;
            if self.polling_enabled {
                // Resume: restore selected reader
                if let Some(ref reader) = self.selected_reader {
                    self.send_command(nfc::Command::SetReader(reader.clone()));
                } else {
                    self.send_command(nfc::Command::Resume);
                }
            } else {
                // Pause: stop polling
                self.send_command(nfc::Command::Pause);
            }
        }

        ui.add_space(10.0);

        let status_text = if self.polling_enabled {
            self.i18n.t("status_running")
        } else {
            self.i18n.t("status_stopped")
        };

        let color = if self.polling_enabled {
            egui::Color32::GREEN
        } else {
            egui::Color32::RED
        };

        ui.colored_label(color, status_text);
    }
}
