//! Synchronization used by generated hooks. No lock is held during user code.
use crate::{Error, HookGuard, MethodPtr, Result};
use std::{
    cell::RefCell,
    ffi::c_void,
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, Mutex},
};

struct Installed {
    guard: HookGuard,
    method: MethodPtr,
}
thread_local! {
    static INVOCATIONS: RefCell<Vec<(usize, Arc<Installed>, *mut c_void)>> = const { RefCell::new(Vec::new()) };
}

/// Implementation detail of `#[cleat::hook]`.
#[doc(hidden)]
pub struct HookSlot(Mutex<Option<Arc<Installed>>>);
impl Default for HookSlot {
    fn default() -> Self {
        Self::new()
    }
}
impl HookSlot {
    pub const fn new() -> Self {
        Self(Mutex::new(None))
    }
    /// # Safety
    /// `replacement` must match the native ABI of `method` exactly.
    pub unsafe fn install(&self, method: MethodPtr, replacement: *const c_void) -> Result<()> {
        let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return Ok(());
        }
        // A callback arriving during installation waits until publication.
        let guard = unsafe { HookGuard::install(method.function, replacement)? };
        *slot = Some(Arc::new(Installed { guard, method }));
        Ok(())
    }
    /// Uninstall after callers have stopped invoking the target method. The
    /// native-to-Rust entry window is not covered by the invocation snapshot.
    pub fn uninstall(&self) -> Result<()> {
        let retired = self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
        // Existing invocations retain the guard until their callback returns.
        drop(retired);
        Ok(())
    }
    pub fn is_installed(&self) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }
    pub fn enter(&self, method_info: *mut c_void) -> Result<HookInvocation> {
        let installed = self.snapshot()?;
        let key = self as *const Self as usize;
        INVOCATIONS.with(|stack| stack.borrow_mut().push((key, installed, method_info)));
        Ok(HookInvocation {
            _thread: PhantomData,
        })
    }
    pub fn with_original<T>(&self, call: impl FnOnce(MethodPtr) -> T) -> Result<T> {
        let key = self as *const Self as usize;
        let active = INVOCATIONS.with(|stack| {
            stack
                .borrow()
                .iter()
                .rev()
                .find(|(slot, _, _)| *slot == key)
                .map(|(_, hook, info)| (Arc::clone(hook), *info))
        });
        let (installed, method_info) = match active {
            Some(active) => active,
            None => {
                let hook = self.snapshot()?;
                let info = hook.method.method_info;
                (hook, info)
            }
        };
        Ok(call(MethodPtr {
            function: installed.guard.trampoline().cast_mut(),
            method_info,
        }))
    }
    fn snapshot(&self) -> Result<Arc<Installed>> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| Error::Hook("hook is not installed".into()))
    }
}
#[doc(hidden)]
pub struct HookInvocation {
    _thread: PhantomData<Rc<()>>,
}
impl Drop for HookInvocation {
    fn drop(&mut self) {
        let retired = INVOCATIONS.with(|stack| stack.borrow_mut().pop());
        drop(retired);
    }
}
