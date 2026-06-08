#[cfg(target_os = "android")]
mod guard;
#[cfg(not(target_os = "android"))]
mod stub;

#[cfg(target_os = "android")]
pub use guard::HookGuard;
#[cfg(not(target_os = "android"))]
pub use stub::HookGuard;
