use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem, MenuEvent},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use std::{thread, time::Duration};

/// Tray icon manager with show/exit menu.
pub struct TrayManager {
    _tray_icon: TrayIcon,
    show_requested: Arc<AtomicBool>,
    exit_requested: Arc<AtomicBool>,
}

impl TrayManager {
    /// Creates a new tray icon with menu and sets up event handler.
    ///
    /// # Errors
    ///
    /// Returns error if tray icon creation fails.
    pub fn new(show_label: &str, exit_label: &str, wake_fn: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let icon = Self::generate_icon()?;
        
        let menu = Menu::new();
        let show_item = MenuItem::new(show_label, true, None);
        let exit_item = MenuItem::new(exit_label, true, None);
        
        menu.append(&show_item)
            .context("failed to add show menu item")?;
        menu.append(&exit_item)
            .context("failed to add exit menu item")?;
        
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("nfc-wedge — NFC считыватель")
            .with_icon(icon)
            .build()
            .context("failed to build tray icon")?;
        
        let show_requested = Arc::new(AtomicBool::new(false));
        let exit_requested = Arc::new(AtomicBool::new(false));
        
        // Set up event handler for menu events
        let show_id = show_item.id().clone();
        let exit_id = exit_item.id().clone();
        let show_flag = show_requested.clone();
        let exit_flag = exit_requested.clone();
        let wake = Arc::new(wake_fn);
        
        // Clone wake Arc for Windows polling thread before moving into MenuEvent handler
        #[cfg(target_os = "windows")]
        let wake_for_thread = Arc::clone(&wake);
        
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id() == &show_id {
                show_flag.store(true, Ordering::SeqCst);
                wake();
            } else if event.id() == &exit_id {
                exit_flag.store(true, Ordering::SeqCst);
                wake();
            }
        }));
        
        // On Windows, spawn a background thread to continuously wake the UI
        // This ensures the event loop keeps running even when the window is hidden
        #[cfg(target_os = "windows")]
        {
            let show_flag_clone = Arc::clone(&show_requested);
            let exit_flag_clone = Arc::clone(&exit_requested);
            
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(100));
                    // If there are pending tray events, wake the UI
                    if show_flag_clone.load(Ordering::SeqCst) || exit_flag_clone.load(Ordering::SeqCst) {
                        wake_for_thread();
                    }
                }
            });
        }
        
        Ok(Self {
            _tray_icon: tray_icon,
            show_requested,
            exit_requested,
        })
    }
    
    /// Polls tray menu events and returns (show_clicked, exit_clicked).
    pub fn poll_events(&self) -> (bool, bool) {
        let show = self.show_requested.swap(false, Ordering::SeqCst);
        let exit = self.exit_requested.swap(false, Ordering::SeqCst);
        
        (show, exit)
    }
    
    /// Generates a simple colored icon (16x16 RGBA).
    fn generate_icon() -> Result<Icon> {
        let size = 16;
        let mut rgba = vec![0u8; size * size * 4];
        
        // Draw a simple green circle
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - size as f32 / 2.0;
                let dy = y as f32 - size as f32 / 2.0;
                let dist = (dx * dx + dy * dy).sqrt();
                
                let idx = (y * size + x) * 4;
                if dist < size as f32 / 2.5 {
                    rgba[idx] = 0;       // R
                    rgba[idx + 1] = 200; // G
                    rgba[idx + 2] = 0;   // B
                    rgba[idx + 3] = 255; // A
                } else {
                    rgba[idx + 3] = 0;   // Transparent
                }
            }
        }
        
        Icon::from_rgba(rgba, size as u32, size as u32)
            .context("failed to create icon from RGBA")
    }
}
