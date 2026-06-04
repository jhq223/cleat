use crate::{Args, Error, Il2CppValueType, MethodInfo, Result};
use il2cpp_bridge_rs as bridge;
use std::ffi::c_void;
use std::fmt;

/// A handle to a live managed object on the IL2CPP heap.
///
/// Cheap to copy (it's a single pointer under the hood). Two handles are
/// equal when they point to the same C# object.
///
/// Not `Send` / `Sync` — managed object pointers are tied to the creating
/// thread. If you need cross-thread access, wrap it in a newtype and
/// `unsafe impl Send` yourself.
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
    /// let weapon: Il2CppObject = obj.load("equippedWeapon")?;
    /// ```
    pub fn load<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        unsafe { T::load_field(&field) }
    }

    /// Write an instance field by name.
    ///
    /// Takes `&self` despite mutating the managed heap — Rust's borrow
    /// checker can't see the mutation because it happens through a raw
    /// pointer.
    pub fn store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        unsafe { T::store_field(&field, val) }
    }

    /// Call a parameterless method and get the return value.
    ///
    /// ```ignore
    /// let name: Il2CppString = obj.invoke("get_Name")?;
    /// let hp: i32 = obj.invoke("get_HP")?;
    /// ```
    pub fn invoke<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        self.invoke_with(name, ())
    }

    /// Call a parameterless void method.
    pub fn invoke_void(&self, name: &str) -> Result<()> {
        self.invoke_with_void(name, ())
    }

    // ── parameterised calls ───────────────────────────────────────────────

    /// Call a method with arguments and get the return value.
    ///
    /// ```ignore
    /// let result: i32 = obj.invoke_with("Calculate", (10, 20))?;
    /// ```
    pub fn invoke_with<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T> {
        let method = self.method(name)?;
        method.invoke(args)
    }

    /// Call a method with arguments, void return.
    pub fn invoke_with_void(&self, name: &str, args: impl Args) -> Result<()> {
        let method = self.method(name)?;
        method.invoke_void(args)
    }

    /// Call a void method with raw pointers — for when Args trait
    /// would pass the wrong pointer (e.g. Il2CppObject gets stack
    /// address of wrapper instead of GC object pointer).
    pub fn invoke_raw_void(&self, name: &str, args: &[*mut c_void]) -> Result<()> {
        let method = self.method(name)?;
        unsafe { method.inner.call::<()>(args).map_err(Error::Bridge) }
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
        let method = self
            .inner
            .method(name)
            .ok_or_else(|| Error::MethodNotFound(name.to_string()))?;
        Ok(MethodInfo { inner: method })
    }

    // ── internal (used by the hook macro) ─────────────────────────────────

    /// The raw object pointer. Hooks use this to pass `this` into the
    /// trampoline.
    #[doc(hidden)]
    pub fn raw_ptr(&self) -> *mut c_void {
        self.inner.ptr as *mut c_void
    }

    /// Wrap a raw pointer back into a handle. The hook macro uses this to
    /// turn the C-ABI `this` back into a borrowable `Il2CppObject`.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer to an IL2CPP-managed object.
    #[doc(hidden)]
    pub unsafe fn from_raw(ptr: *mut c_void) -> Self {
        Self {
            inner: unsafe { bridge::structs::Object::from_ptr(ptr) },
        }
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

// IL2CPP runs single-threaded in the Unity player — marking Send + Sync
// is safe in practice since objects are never shared across threads.
unsafe impl Send for Il2CppObject {}
unsafe impl Sync for Il2CppObject {}

// Null default — the hook wrapper uses this when the inner function
// returns `Err`. The zeroed object will never be dereferenced.
impl Default for Il2CppObject {
    fn default() -> Self {
        Self {
            inner: unsafe { bridge::structs::Object::from_ptr(std::ptr::null_mut()) },
        }
    }
}

unsafe impl Il2CppValueType for Il2CppObject {
    unsafe fn load_field(field: &il2cpp_bridge_rs::structs::Field) -> Result<Self> {
        let ptr = unsafe { field.get_value::<*mut c_void>().map_err(Error::Bridge)? };
        let obj = unsafe { bridge::structs::Object::from_ptr(ptr) };
        Ok(Il2CppObject { inner: obj })
    }

    unsafe fn store_field(field: &il2cpp_bridge_rs::structs::Field, val: Self) -> Result<()> {
        unsafe {
            field
                .set_value::<*mut c_void>(val.inner.as_ptr())
                .map_err(Error::Bridge)
        }
    }
}
