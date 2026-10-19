//! Frida Gum inline replacement backend.
use crate::{Error, Result};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

// Gum lives as long as the injected library. Serialize patch operations.
static PATCH_LOCK: Mutex<()> = Mutex::new(());
static INTERCEPTOR: OnceLock<usize> = OnceLock::new();

unsafe extern "C" {
    fn gum_init_embedded();
    fn gum_interceptor_obtain() -> *mut c_void;
    fn gum_interceptor_replace(
        interceptor: *mut c_void,
        target: *mut c_void,
        replacement: *mut c_void,
        original: *mut *mut c_void,
        options: *const c_void,
    ) -> i32;
    fn gum_interceptor_revert(interceptor: *mut c_void, target: *mut c_void);
}

pub(super) fn interceptor() -> *mut c_void {
    *INTERCEPTOR.get_or_init(|| unsafe {
        gum_init_embedded();
        gum_interceptor_obtain() as usize
    }) as *mut c_void
}
#[cfg(feature = "class-injection")]
pub(crate) fn initialize() {
    let _ = interceptor();
}

#[must_use = "dropping the guard removes the hook"]
pub struct HookGuard {
    target: *const c_void,
    original: *const c_void,
}

impl HookGuard {
    /// Replace a native function until this guard is dropped.
    ///
    /// # Safety
    /// Both pointers must remain valid executable functions with identical ABI,
    /// including IL2CPP's trailing MethodInfo parameter. Keep this guard alive
    /// while using its trampoline or executing the replacement.
    pub unsafe fn install(target: *const c_void, replace: *const c_void) -> Result<Self> {
        if target.is_null() || replace.is_null() {
            return Err(Error::Hook(
                "target and replacement must be non-null".into(),
            ));
        }
        let _lock = PATCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut original = std::ptr::null_mut();
        let status = unsafe {
            gum_interceptor_replace(
                interceptor(),
                target.cast_mut(),
                replace.cast_mut(),
                &mut original,
                std::ptr::null(),
            )
        };
        if status != 0 {
            let reason = match status {
                -1 => "unsupported instruction in function prologue",
                -2 => "unsupported function signature/prologue",
                -3 => "function is already replaced",
                -4 => "code signing policy rejected replacement",
                -5 => "incompatible interception type",
                _ => "unknown Gum error",
            };
            return Err(Error::Hook(format!("Frida Gum: {reason} ({status})")));
        }
        if original.is_null() {
            unsafe { gum_interceptor_revert(interceptor(), target.cast_mut()) };
            return Err(Error::Hook("Frida Gum returned a null trampoline".into()));
        }
        Ok(Self { target, original })
    }
    /// Address of the original function, valid while this guard lives.
    pub fn trampoline(&self) -> *const c_void {
        self.original
    }
    /// Restore the function.
    pub fn uninstall(self) -> Result<()> {
        drop(self);
        Ok(())
    }
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        let _lock = PATCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { gum_interceptor_revert(interceptor(), self.target.cast_mut()) };
    }
}
// Addresses are immutable; Gum patch operations are serialized above.
unsafe impl Send for HookGuard {}
unsafe impl Sync for HookGuard {}
