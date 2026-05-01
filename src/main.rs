#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod auto_start;
mod config;
mod event_bus;
mod i18n;
mod log_buffer;
mod nfc;
mod single_shot;
mod tray;
mod wedge;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> anyhow::Result<()> {
    let log_buffer = log_buffer::LogBuffer::new();
    let log_layer = log_buffer::LogBufferLayer::new(log_buffer.clone());
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        ))
        .with(log_layer)
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
            let (_nfc_handle, nfc_cmd) = nfc::start(nfc_event_sender, config.cooldown_ms)
                .expect("Failed to start NFC thread");

            let wake_fn = {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            };
            
            let app = app::App::new(config, i18n, nfc_cmd, event_bus, log_buffer.clone(), wake_fn);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}
