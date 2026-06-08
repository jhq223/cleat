//! Desktop no-op stub — allows `cargo check` on non-Android platforms.

use crate::Result;

/// Desktop no-op hook handle.
pub struct HookGuard;

impl HookGuard {
    /// No-op — logs a warning and returns `Ok`.
    pub fn install(
        _target: *const std::ffi::c_void,
        _replace: *const std::ffi::c_void,
    ) -> Result<Self> {
        log::warn!("HookGuard::install called on non-Android platform — skipping");
        Ok(Self)
    }

    /// Returns a null trampoline pointer.
    pub fn trampoline(&self) -> *const std::ffi::c_void {
        std::ptr::null()
    }

    /// No-op uninstall.
    pub fn uninstall(self) -> Result<()> {
        Ok(())
    }
}

unsafe impl Send for HookGuard {}
unsafe impl Sync for HookGuard {}
