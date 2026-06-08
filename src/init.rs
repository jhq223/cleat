#[cfg(target_os = "android")]
use std::ffi::CString;
#[cfg(target_os = "android")]
use std::thread;
#[cfg(target_os = "android")]
use std::time::Duration;

/// Max retries waiting for the target image to be loaded.
#[cfg(target_os = "android")]
const MAX_RETRIES: u32 = 30;
#[cfg(target_os = "android")]
const RETRY_DELAY: Duration = Duration::from_secs(2);

/// Bootstrap the cleat runtime.
///
/// Sets up android_logger, then spawns a background thread that waits
/// for `target_image` (e.g. libil2cpp.so) to load before calling
/// il2cpp_bridge_rs::init. Returns immediately so JNI_OnLoad does not
/// block the main thread (which would prevent Unity from loading its
/// native libraries).
#[cfg(target_os = "android")]
pub fn init(target_image: &str, on_ready: impl FnOnce() + Send + 'static) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );

    log::info!("cleat init: waiting for {target_image}...");

    let c_name = CString::new(target_image).unwrap();
    let image = target_image.to_owned();

    thread::spawn(move || {
        for i in 0..=MAX_RETRIES {
            let handle = unsafe { libc::dlopen(c_name.as_ptr(), libc::RTLD_NOLOAD) };
            if !handle.is_null() {
                unsafe { libc::dlclose(handle) };
                log::info!("cleat init: {image} found (attempt {i})");
                il2cpp_bridge_rs::init(&image, on_ready);
                return;
            }
            if i < MAX_RETRIES {
                thread::sleep(RETRY_DELAY);
            }
        }
        log::error!("cleat init: {image} not loaded after {MAX_RETRIES} retries");
    });
}

/// Desktop stub — init is a no-op outside Android.
#[cfg(not(target_os = "android"))]
pub fn init(_target_image: &str, _on_ready: impl FnOnce() + Send + 'static) {
    log::info!("cleat init: skipped (not on Android)");
}
