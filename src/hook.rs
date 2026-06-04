//! ShadowHook inline hook integration.
//!
//! We don't link against libshadowhook.so at compile time — symbols are
//! resolved at runtime through `dlopen` / `dlsym`. Users need to extract
//! libshadowhook.so from the Maven AAR and place it in the APK under
//! `lib/arm64-v8a/`.

use crate::{Error, Result};
use std::ffi::{CStr, c_char, c_void};
use std::sync::{Once, OnceLock};

// ── Function pointer types for each ShadowHook API ──

type ShInitFn       = unsafe extern "C" fn(mode: u32, debuggable: bool) -> i32;
type ShHookFn       = unsafe extern "C" fn(target: *const c_void,
                                           new_addr: *const c_void,
                                           orig_addr: *mut *const c_void) -> *mut c_void;
type ShUnhookFn     = unsafe extern "C" fn(stub: *mut c_void) -> i32;
type ShGetErrnoFn   = unsafe extern "C" fn() -> i32;
type ShToErrmsgFn   = unsafe extern "C" fn(err_num: i32) -> *const c_char;

/// Resolved ShadowHook API entry points. Initialised lazily on first use.
struct ShadowHookAPI {
    init:           ShInitFn,
    hook_func_addr: ShHookFn,
    unhook:         ShUnhookFn,
    get_errno:      ShGetErrnoFn,
    to_errmsg:      ShToErrmsgFn,
}

/// OnceLock doesn't have an infallible `try` variant on stable, so we wrap
/// the result in an `Option`. `None` means the library wasn't found.
static SH: OnceLock<Option<ShadowHookAPI>> = OnceLock::new();

/// Open libshadowhook.so and resolve the five API functions we need.
/// Returns `None` instead of panicking — callers decide how to handle the
/// missing-library case.
fn resolve_shadowhook() -> Option<&'static ShadowHookAPI> {
    SH.get_or_init(|| {
        let handle = unsafe {
            libc::dlopen(
                b"libshadowhook.so\0".as_ptr() as *const _,
                libc::RTLD_NOW | libc::RTLD_GLOBAL,
            )
        };
        if handle.is_null() {
            log::error!(
                "libshadowhook.so not found in APK's lib/arm64-v8a/!\n\
                 Make sure to extract it from shadowhook.aar and inject into the APK."
            );
            return None;
        }

        macro_rules! dlsym {
            ($name:expr) => {
                libc::dlsym(handle, concat!($name, "\0").as_ptr() as *const _)
            };
        }

        Some(unsafe {
            ShadowHookAPI {
                init:           std::mem::transmute(dlsym!("shadowhook_init")),
                hook_func_addr: std::mem::transmute(dlsym!("shadowhook_hook_func_addr")),
                unhook:         std::mem::transmute(dlsym!("shadowhook_unhook")),
                get_errno:      std::mem::transmute(dlsym!("shadowhook_get_errno")),
                to_errmsg:      std::mem::transmute(dlsym!("shadowhook_to_errmsg")),
            }
        })
    })
    .as_ref()
}

/// One-shot ShadowHook initialisation guard.
static SHADOWHOOK_INIT: Once = Once::new();

fn ensure_init() {
    SHADOWHOOK_INIT.call_once(|| {
        let Some(api) = resolve_shadowhook() else {
            log::error!("shadowhook_init failed: library not found");
            return;
        };
        // SHADOWHOOK_MODE_UNIQUE = 1 — each address can only be hooked once.
        let ret = unsafe { (api.init)(1, false) };
        if ret != 0 {
            log::error!("shadowhook_init failed: {}", ret);
        }
    });
}

// ── HookGuard ────────────────────────────────────────────────────────────

/// Opaque hook handle. Created and managed by the `#[cleat::hook]` macro —
/// users never construct or destroy these directly.
///
/// Carries the ShadowHook stub (needed for unhook) and the trampoline pointer
/// that lets `original()` call the real function after patching.
#[doc(hidden)]
pub struct HookGuard {
    stub:     *mut c_void,
    original: *const c_void,
}

#[doc(hidden)]
impl HookGuard {
    /// Patch the target function so calls are redirected to `replace`.
    /// Returns a guard that will restore the original code when dropped
    /// (or when `uninstall` is called explicitly).
    pub fn install(target: *const c_void, replace: *const c_void) -> Result<Self> {
        ensure_init();
        let Some(api) = resolve_shadowhook() else {
            return Err(Error::Hook("shadowhook library not loaded".into()));
        };

        let mut original: *const c_void = std::ptr::null();
        let stub = unsafe { (api.hook_func_addr)(target, replace, &mut original) };
        if stub.is_null() {
            let err_num = unsafe { (api.get_errno)() };
            let err_msg = unsafe { CStr::from_ptr((api.to_errmsg)(err_num)) }.to_string_lossy();
            return Err(Error::Hook(format!(
                "shadowhook_hook_func_addr failed [errno={err_num}]: {err_msg}"
            )));
        }

        Ok(Self { stub, original })
    }

    /// The trampoline address. The generated `original()` function transmutes
    /// this into a proper function pointer and calls through it.
    pub fn trampoline(&self) -> *const c_void {
        self.original
    }

    /// Tear down the hook, putting the original instructions back.
    /// Consumes the guard so Drop won't double-unhook.
    pub fn uninstall(self) -> Result<()> {
        let Some(api) = resolve_shadowhook() else {
            // Library already unloaded — stub is stale anyway.
            std::mem::forget(self);
            return Ok(());
        };
        let stub = self.stub;
        std::mem::forget(self);          // don't let Drop try again
        let ret = unsafe { (api.unhook)(stub) };
        if ret != 0 {
            return Err(Error::Hook(format!("shadowhook_unhook failed: {ret}")));
        }
        Ok(())
    }
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        try_unhook(&mut self.stub);
    }
}

// ── try_unhook ───────────────────────────────────────────────────────────

/// Best-effort unhook that never panics (safe to use from Drop).
///
/// Uses `RTLD_NOLOAD` to check whether libshadowhook.so is still in memory.
/// If the library has already been unloaded we can't do anything — we just
/// discard the stub and move on.
fn try_unhook(stub: &mut *mut c_void) {
    if stub.is_null() {
        return;
    }

    // RTLD_NOLOAD — don't trigger a load, just check what's already there.
    let handle = unsafe {
        libc::dlopen(
            b"libshadowhook.so\0".as_ptr() as *const _,
            libc::RTLD_NOW | libc::RTLD_NOLOAD,
        )
    };
    if handle.is_null() {
        *stub = std::ptr::null_mut();
        return;
    }

    let unhook: ShUnhookFn = unsafe {
        let sym = libc::dlsym(handle, b"shadowhook_unhook\0".as_ptr() as *const _);
        if sym.is_null() {
            *stub = std::ptr::null_mut();
            return;
        }
        std::mem::transmute(sym)
    };

    unsafe { (unhook)(*stub) };
    *stub = std::ptr::null_mut();
}
