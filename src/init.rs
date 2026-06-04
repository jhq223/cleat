use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The app's private data directory, set once at startup.
static APP_DATA: OnceLock<PathBuf> = OnceLock::new();

/// Bootstrap the cleat runtime.
///
/// Sets up android_logger (output goes to logcat) and hands control to
/// il2cpp_bridge_rs. The `on_ready` closure fires after IL2CPP metadata
/// has been parsed — that's where your mod code runs.
///
/// Usually you won't call this yourself. `#[cleat::main]` wraps it for you.
pub fn init(target_image: &str, on_ready: impl FnOnce() + Send + 'static) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );
    il2cpp_bridge_rs::init(target_image, on_ready);
}

/// Tell cleat where your app's private files live.
///
/// Call this once at the top of your `#[cleat::main]` function. Subsequent
/// calls are ignored.
///
/// ```ignore
/// cleat::set_app_data("/data/data/com.example.game/files");
/// ```
pub fn set_app_data(path: impl AsRef<Path>) {
    let _ = APP_DATA.set(path.as_ref().to_path_buf());
}

/// Get the app data directory that was set with [`set_app_data`].
///
/// Returns `Err(Error::NotInitialized)` if `set_app_data` hasn't been
/// called yet.
///
/// ```ignore
/// let config = cleat::app_data()?.join("ModConfig/config.json");
/// let json = std::fs::read_to_string(config)?;
/// ```
pub fn app_data() -> Result<&'static Path> {
    APP_DATA
        .get()
        .map(|p| p.as_path())
        .ok_or(Error::NotInitialized)
}
