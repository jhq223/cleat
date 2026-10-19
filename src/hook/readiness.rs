//! Observe successful runtime calls without executing IL2CPP during startup.
use super::guard::interceptor;
use crate::{Error, Result};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
};

static READY: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn gum_make_call_listener(
        enter: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        leave: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        data: *mut c_void,
        destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> *mut c_void;
    fn gum_interceptor_attach(
        interceptor: *mut c_void,
        target: *mut c_void,
        listener: *mut c_void,
        options: *const c_void,
    ) -> i32;
    fn gum_interceptor_detach(interceptor: *mut c_void, listener: *mut c_void);
    fn gum_invocation_context_get_return_value(context: *mut c_void) -> *mut c_void;
    #[link_name = "_frida_g_object_unref"]
    fn object_unref(object: *mut c_void);
}

unsafe extern "C" fn returned(context: *mut c_void, _data: *mut c_void) {
    // init returns true on modern Unity; Domain::Get and Thread::Attach return
    // pointers. Observe the game's own calls: even Domain::Get can allocate
    // objects and crash if invoked before Runtime::Init has set up defaults.
    if !unsafe { gum_invocation_context_get_return_value(context) }.is_null() {
        READY.store(true, Ordering::Release);
    }
}

pub(crate) struct RuntimeObserver(*mut c_void);
impl RuntimeObserver {
    /// The library handle must refer to the loaded IL2CPP runtime.
    pub(crate) unsafe fn install(library: *mut c_void) -> Result<Self> {
        // GLib/GObject must be initialized before constructing a listener.
        let interceptor = interceptor();
        let listener =
            unsafe { gum_make_call_listener(None, Some(returned), std::ptr::null_mut(), None) };
        if listener.is_null() {
            return Err(Error::Hook(
                "could not allocate runtime readiness observer".into(),
            ));
        }
        let observer = Self(listener);
        let mut attached = 0;
        for symbol in [
            c"il2cpp_init",
            c"il2cpp_init_utf16",
            c"il2cpp_domain_get",
            c"il2cpp_thread_attach",
        ] {
            let target = unsafe { libc::dlsym(library, symbol.as_ptr()) };
            if target.is_null() {
                continue;
            }
            let status =
                unsafe { gum_interceptor_attach(interceptor, target, listener, std::ptr::null()) };
            if status == 0 {
                attached += 1;
            } else {
                log::warn!(
                    "runtime observer: {}: Gum status {status}",
                    symbol.to_string_lossy()
                );
            }
        }
        if attached == 0 {
            return Err(Error::Hook(
                "could not observe IL2CPP initialization".into(),
            ));
        }
        Ok(observer)
    }

    pub(crate) fn is_ready() -> bool {
        READY.load(Ordering::Acquire)
    }
}
impl Drop for RuntimeObserver {
    fn drop(&mut self) {
        unsafe {
            // Gum retains listeners referenced by in-flight invocations.
            gum_interceptor_detach(interceptor(), self.0);
            object_unref(self.0);
        }
    }
}
