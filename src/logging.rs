use simplelog::*;
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};

/// Logging flags for granular control
static LOG_SELECTION: AtomicBool = AtomicBool::new(false);
static LOG_MOVEMENT: AtomicBool = AtomicBool::new(false);
static LOG_KEYS: AtomicBool = AtomicBool::new(false);

/// Initialize logging with the specified file path
pub fn init(path: &str) -> std::io::Result<()> {
    let file = File::create(path)?;

    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        file,
    ).map_err(std::io::Error::other)?;

    Ok(())
}

/// Configure which categories to log
pub fn configure(selection: bool, movement: bool, keys: bool) {
    LOG_SELECTION.store(selection, Ordering::Relaxed);
    LOG_MOVEMENT.store(movement, Ordering::Relaxed);
    LOG_KEYS.store(keys, Ordering::Relaxed);
}

/// Check if selection logging is enabled
pub fn log_selection() -> bool {
    LOG_SELECTION.load(Ordering::Relaxed)
}

/// Check if movement logging is enabled
pub fn log_movement() -> bool {
    LOG_MOVEMENT.load(Ordering::Relaxed)
}

/// Check if key logging is enabled
pub fn log_keys() -> bool {
    LOG_KEYS.load(Ordering::Relaxed)
}
