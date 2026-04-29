use anyhow::{Context, Result};
use enigo::{Enigo, Keyboard, Settings};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

/// Global Enigo instance, initialized lazily on first use.
static ENIGO_INSTANCE: OnceLock<Mutex<Enigo>> = OnceLock::new();

/// Gets or initializes the singleton Enigo instance.
fn get_enigo() -> &'static Mutex<Enigo> {
    ENIGO_INSTANCE.get_or_init(|| {
        let enigo = Enigo::new(&Settings::default())
            .expect("failed to initialize enigo");
        Mutex::new(enigo)
    })
}

/// Types the given text into the active window using keyboard simulation.
///
/// # Arguments
/// * `text` - The text to type
/// * `delay_ms` - Optional delay in milliseconds between each character (0 = no delay)
/// * `append_enter` - Whether to press Enter after typing
///
/// # Errors
///
/// Returns error if keyboard simulation fails or mutex is poisoned.
pub fn type_text(text: &str, delay_ms: u64, append_enter: bool) -> Result<()> {
    let mut enigo = get_enigo()
        .lock()
        .map_err(|_| anyhow::anyhow!("enigo mutex poisoned"))?;

    if delay_ms > 0 {
        let delay = Duration::from_millis(delay_ms);
        for ch in text.chars() {
            enigo.text(&ch.to_string())
                .context("failed to type character")?;
            thread::sleep(delay);
        }
    } else {
        enigo.text(text)
            .context("failed to type text")?;
    }

    if append_enter {
        enigo.key(enigo::Key::Return, enigo::Direction::Click)
            .context("failed to press Enter")?;
    }

    Ok(())
}
