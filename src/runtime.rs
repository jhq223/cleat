//! Thread attachment and managed object lifetime.
use crate::{Error, Il2CppObject, Result, bridge};
use std::{marker::PhantomData, rc::Rc};

/// Attaches the calling thread until dropped. Never move it to another thread.
pub struct ThreadGuard {
    _inner: bridge::api::Thread,
    _thread: PhantomData<Rc<()>>,
}
impl ThreadGuard {
    pub fn attach() -> Result<Self> {
        if !bridge::api::is_loaded() {
            return Err(Error::Bridge("IL2CPP is not initialized".into()));
        }
        bridge::api::Thread::attach(true)
            .map(|inner| Self {
                _inner: inner,
                _thread: PhantomData,
            })
            .ok_or_else(|| Error::Bridge("could not attach the current thread".into()))
    }
}

/// Keeps a managed object alive until dropped. Does not synchronize its contents.
pub struct GcHandle {
    handle: bridge::api::GcHandleId,
}
impl GcHandle {
    pub fn new(object: Il2CppObject) -> Result<Self> {
        Self::create(object, false)
    }
    /// Pin the object when native code must retain its address.
    pub fn pinned(object: Il2CppObject) -> Result<Self> {
        Self::create(object, true)
    }
    fn create(object: Il2CppObject, pinned: bool) -> Result<Self> {
        if object.is_null() {
            return Err(Error::NullObject);
        }
        let handle = unsafe { bridge::api::gchandle_new(object.raw_ptr(), pinned) };
        if handle as usize == 0 {
            return Err(Error::Bridge("could not create GC handle".into()));
        }
        Ok(Self { handle })
    }
    pub fn object(&self) -> Il2CppObject {
        unsafe { Il2CppObject::from_raw(bridge::api::gchandle_get_target(self.handle)) }
    }
}
impl Drop for GcHandle {
    fn drop(&mut self) {
        unsafe { bridge::api::gchandle_free(self.handle) };
    }
}

// The runtime owns this opaque GC handle; it may be queried or freed on an attached thread.
unsafe impl Send for GcHandle {}
unsafe impl Sync for GcHandle {}
