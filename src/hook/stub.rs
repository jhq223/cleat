//! Unsupported platforms fail explicitly; no fake trampoline is returned.
use crate::{Error, Result};
use std::ffi::c_void;
pub struct HookGuard;
impl HookGuard {
    /// # Safety
    /// See the Android backend's `HookGuard::install` contract.
    pub unsafe fn install(_target: *const c_void, _replace: *const c_void) -> Result<Self> {
        Err(Error::UnsupportedPlatform("inline hooks require Android"))
    }
    pub fn trampoline(&self) -> *const c_void {
        std::ptr::null()
    }
    pub fn uninstall(self) -> Result<()> {
        Ok(())
    }
}
