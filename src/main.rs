mod config;
mod i18n;

use eframe::egui;
use config::Config;
use i18n::I18n;
use tracing::Level;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let config = Config::load()?;
    let i18n = I18n::new(&config.language);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_title(i18n.t("waiting_card")),
        ..Default::default()
    };

    eframe::run_native(
        "nfc-wedge",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(config, i18n)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}

struct App {
    #[allow(dead_code)]
    config: Config,
    i18n: I18n,
}

impl App {
    fn new(config: Config, i18n: I18n) -> Self {
        Self { config, i18n }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.i18n.t("waiting_card"));
            ui.label(self.i18n.t("settings"));
            ui.label(self.i18n.t("logs"));
            ui.label(self.i18n.t("enable_disable"));
        });
    }
}
