mod app;
mod config;
mod event_bus;
mod i18n;
mod nfc;

use tracing::Level;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let config = config::Config::load()?;
    let i18n = i18n::I18n::new(&config.language)?;

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_title(i18n.t("waiting_card")),
        ..Default::default()
    };

    eframe::run_native(
        "nfc-wedge",
        options,
        Box::new(|cc| {
            let (event_bus, nfc_event_sender) = event_bus::EventBus::new({
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            });
            let (_nfc_handle, nfc_cmd) = nfc::start(nfc_event_sender)
                .expect("Failed to start NFC thread");

            let app = app::App::new(config, i18n, nfc_cmd, event_bus);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}
