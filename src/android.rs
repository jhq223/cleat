//! Access to Android APIs from Rust, using the VM supplied to JNI_OnLoad.
use crate::{Error, Result};
pub use jni;

/// Register Android's VM. Called by `#[cleat::main]` before starting the worker.
///
/// # Safety
/// `vm` must be the process's live JavaVM pointer supplied to JNI_OnLoad.
#[doc(hidden)]
pub unsafe fn initialize(vm: *mut std::ffi::c_void) {
    if !vm.is_null() {
        let _ = unsafe { jni::JavaVM::from_raw(vm.cast()) };
    }
}

/// Run a JNI operation with scoped local references and exception handling.
/// Java references returned from this callback must be global references.
pub fn with_java<T>(callback: impl FnOnce(&mut jni::Env) -> jni::errors::Result<T>) -> Result<T> {
    jni::JavaVM::singleton()
        .and_then(|vm| vm.attach_current_thread(callback))
        .map_err(|error| Error::Bridge(format!("Android: {error}")))
}
