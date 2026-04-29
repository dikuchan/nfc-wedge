use anyhow::Result;

/// Checks if auto-start is enabled.
///
/// # Errors
///
/// Returns error if registry access fails (Windows only).
#[cfg(target_os = "windows")]
pub fn is_enabled() -> Result<bool> {
    use anyhow::Context;
    use auto_launch::AutoLaunch;
    
    let app_name = "nfc-wedge";
    let app_path = std::env::current_exe()
        .context("failed to get current exe path")?;
    
    let auto = AutoLaunch::new(app_name, app_path.to_str().unwrap(), &[] as &[&str]);
    auto.is_enabled()
        .context("failed to check auto-start status")
}

/// Enables auto-start on Windows login.
///
/// # Errors
///
/// Returns error if registry write fails (Windows only).
#[cfg(target_os = "windows")]
pub fn enable() -> Result<()> {
    use anyhow::Context;
    use auto_launch::AutoLaunch;
    
    let app_name = "nfc-wedge";
    let app_path = std::env::current_exe()
        .context("failed to get current exe path")?;
    
    let auto = AutoLaunch::new(app_name, app_path.to_str().unwrap(), &[] as &[&str]);
    auto.enable()
        .context("failed to enable auto-start")
}

/// Disables auto-start.
///
/// # Errors
///
/// Returns error if registry delete fails (Windows only).
#[cfg(target_os = "windows")]
pub fn disable() -> Result<()> {
    use anyhow::Context;
    use auto_launch::AutoLaunch;
    
    let app_name = "nfc-wedge";
    let app_path = std::env::current_exe()
        .context("failed to get current exe path")?;
    
    let auto = AutoLaunch::new(app_name, app_path.to_str().unwrap(), &[] as &[&str]);
    auto.disable()
        .context("failed to disable auto-start")
}

// macOS stubs - do nothing
#[cfg(not(target_os = "windows"))]
pub fn is_enabled() -> Result<bool> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn enable() -> Result<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn disable() -> Result<()> {
    Ok(())
}
