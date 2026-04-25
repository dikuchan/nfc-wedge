mod app;
mod config;
mod i18n;
mod nfc;

use tracing::Level;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let config = config::Config::load()?;
    let i18n = i18n::I18n::new(&config.language)?;

    let (nfc_handle, nfc_cmd, nfc_evt) = nfc::start()?;

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_title(i18n.t("waiting_card")),
        ..Default::default()
    };

    let app = app::App::new(config, i18n, nfc_cmd, nfc_evt);

    eframe::run_native("nfc-wedge", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    drop(nfc_handle);
    Ok(())
}
