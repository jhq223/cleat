//! Handle to a live managed object on the IL2CPP heap.

use std::ffi::c_void;
use std::fmt;

use il2cpp_bridge_rs as bridge;
use il2cpp_bridge_rs::structs::Field;

use crate::{Args, Error, Il2CppClass, Il2CppValueType, MethodInfo, Result};

/// A handle to a live managed object on the IL2CPP heap.
///
/// Cheap to copy — it's a single pointer under the hood. Two handles
/// are equal when they point to the same C# object.
///
/// Marked `Send + Sync` because IL2CPP runs single-threaded in the
/// Unity player — objects are never shared across threads in practice.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Il2CppObject {
    pub(crate) inner: bridge::structs::Object,
}

impl Il2CppObject {
    // ── field access ──────────────────────────────────────────────────────

    /// Read an instance field by name.
    ///
    /// `T` can be anything that implements `Il2CppValueType` — primitives,
    /// `Il2CppString`, `Il2CppObject`, `Vector3`, etc.
    ///
    /// ```ignore
    /// let hp: i32 = obj.load("currentHealth")?;
    /// let name: Il2CppString = obj.load("playerName")?;
    /// ```
    pub fn load<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        unsafe { T::load_field(&field) }
    }

    /// Write an instance field by name.
    pub fn store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        unsafe { T::store_field(&field, val) }
    }

    /// Get a cached field handle for repeated access.
    ///
    /// The handle remembers the field offset — subsequent `get()`/`set()`
    /// calls skip the name lookup.
    ///
    /// ```ignore
    /// let hp_field = obj.field("currentHealth")?;
    /// let hp: i32 = hp_field.get()?;
    /// hp_field.set(999)?;
    /// ```
    pub fn field(&self, name: &str) -> Result<FieldHandle> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        Ok(FieldHandle { field })
    }

    // ── method invocation ─────────────────────────────────────────────────

    /// Call a method and get the return value.
    ///
    /// `args` is a tuple — `()` for no parameters, `(42,)` for one, etc.
    ///
    /// ```ignore
    /// let hp: i32 = obj.invoke("get_HP", ())?;
    /// let result: i32 = obj.invoke("Calculate", (10, 20))?;
    /// ```
    pub fn invoke<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T> {
        let method = self.get_method(name)?;
        method.invoke(args)
    }

    /// Call a void method.
    ///
    /// ```ignore
    /// obj.invoke_void("Update", ())?;
    /// obj.invoke_void("SetHP", (999,))?;
    /// ```
    pub fn invoke_void(&self, name: &str, args: impl Args) -> Result<()> {
        let method = self.get_method(name)?;
        method.invoke_void(args)
    }

    /// Get a method handle, useful when you need to specialise a generic
    /// method before calling it.
    ///
    /// ```ignore
    /// let data: Il2CppObject = obj
    ///     .method("GetMasterData")?
    ///     .inflate(&[&some_class])?
    ///     .invoke(())?;
    /// ```
    pub fn method(&self, name: &str) -> Result<MethodInfo> {
        self.get_method(name)
    }

    /// Get the class of this object at runtime.
    ///
    /// Returns the actual type, even if the handle was obtained as
    /// a base class reference.
    ///
    /// ```ignore
    /// let klass = obj.class()?;
    /// if klass.is_subclass_of(&player_class, false) { ... }
    /// ```
    pub fn class(&self) -> Result<Il2CppClass> {
        self.inner
            .get_class()
            .map(|c| Il2CppClass { inner: c })
            .ok_or_else(|| Error::Bridge("could not get class for object".into()))
    }

    // ── low-level field access by offset ─────────────────────────────────

    /// Read a pointer-sized value at a byte offset from the object base.
    ///
    /// Returns `None` when the object or the stored pointer is null.
    ///
    /// # Safety
    ///
    /// `offset` must be within the object's allocated memory.
    pub unsafe fn read_ptr_at(&self, offset: isize) -> Option<*mut c_void> {
        let base = self.raw_ptr();
        if base.is_null() {
            return None;
        }
        let addr = unsafe { (base as *const u8).add(offset as usize) as *const *mut c_void };
        let ptr = unsafe { std::ptr::read(addr) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    /// Write a pointer-sized value at a byte offset.
    ///
    /// # Safety
    ///
    /// `offset` must be within the object's allocated memory.
    pub unsafe fn write_ptr_at(&self, offset: isize, val: *mut c_void) {
        let base = self.raw_ptr();
        if base.is_null() {
            return;
        }
        let addr = unsafe { (base as *mut u8).add(offset as usize) as *mut *mut c_void };
        unsafe { std::ptr::write(addr, val) };
    }

    /// Read a 32-bit signed integer at a byte offset.
    ///
    /// # Safety
    ///
    /// `offset` must be within the object's allocated memory.
    pub unsafe fn read_i32_at(&self, offset: isize) -> i32 {
        let base = self.raw_ptr();
        debug_assert!(!base.is_null());
        let addr = unsafe { (base as *const u8).add(offset as usize) as *const i32 };
        unsafe { std::ptr::read(addr) }
    }

    /// Read a pointer-sized unsigned integer at a byte offset.
    ///
    /// # Safety
    ///
    /// `offset` must be within the object's allocated memory.
    pub unsafe fn read_usize_at(&self, offset: isize) -> usize {
        let base = self.raw_ptr();
        debug_assert!(!base.is_null());
        let addr = unsafe { (base as *const u8).add(offset as usize) as *const usize };
        unsafe { std::ptr::read(addr) }
    }

    // ── internal ─────────────────────────────────────────────────────────

    /// The raw object pointer. Hooks use this to pass `this` into the
    /// trampoline.
    pub fn raw_ptr(&self) -> *mut c_void {
        self.inner.ptr as *mut c_void
    }

    /// Wrap a raw pointer back into a handle.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer to an IL2CPP-managed object.
    pub unsafe fn from_raw(ptr: *mut c_void) -> Self {
        Self {
            inner: unsafe { bridge::structs::Object::from_ptr(ptr) },
        }
    }

    fn get_method(&self, name: &str) -> Result<MethodInfo> {
        let method = self
            .inner
            .method(name)
            .ok_or_else(|| Error::MethodNotFound(name.to_string()))?;
        Ok(MethodInfo { inner: method })
    }
}

// ── standard traits ────────────────────────────────────────────────────────

impl fmt::Debug for Il2CppObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Il2CppObject({:p})", self.inner.as_ptr())
    }
}

impl PartialEq for Il2CppObject {
    fn eq(&self, other: &Self) -> bool {
        self.inner.ptr == other.inner.ptr
    }
}

impl Eq for Il2CppObject {}

unsafe impl Send for Il2CppObject {}
unsafe impl Sync for Il2CppObject {}

impl Default for Il2CppObject {
    fn default() -> Self {
        Self {
            inner: unsafe { bridge::structs::Object::from_ptr(std::ptr::null_mut()) },
        }
    }
}

// ── Il2CppValueType impl ──────────────────────────────────────────────────

unsafe impl Il2CppValueType for Il2CppObject {
    unsafe fn load_field(field: &Field) -> Result<Self> {
        let ptr = unsafe { field.get_value::<*mut c_void>().map_err(Error::Bridge)? };
        let obj = unsafe { bridge::structs::Object::from_ptr(ptr) };
        Ok(Il2CppObject { inner: obj })
    }

    unsafe fn store_field(field: &Field, val: Self) -> Result<()> {
        unsafe {
            field
                .set_value::<*mut c_void>(val.inner.as_ptr())
                .map_err(Error::Bridge)
        }
    }

    /// Pass the GC object pointer directly instead of a stack address.
    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self.inner.as_ptr()
    }
}

// ── FieldHandle ────────────────────────────────────────────────────────────

/// A cached field handle — remembers the field offset so subsequent
/// reads and writes skip the name lookup.
///
/// Obtain via [`Il2CppObject::field`].
pub struct FieldHandle {
    field: Field,
}

impl FieldHandle {
    /// Read the field value.
    pub fn get<T: Il2CppValueType>(&self) -> Result<T> {
        unsafe { T::load_field(&self.field) }
    }

    /// Write the field value.
    pub fn set<T: Il2CppValueType>(&self, val: T) -> Result<()> {
        unsafe { T::store_field(&self.field, val) }
    }
}
