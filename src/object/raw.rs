//! Handle to a live managed object on the IL2CPP heap.

use std::ffi::c_void;
use std::fmt;

use il2cpp_bridge_rs as bridge;
use il2cpp_bridge_rs::structs::Field;

use crate::{Args, Error, Il2CppClass, Il2CppValueType, MethodInfo, MethodSelector, Result};

/// A handle to a live managed object on the IL2CPP heap.
///
/// Cheap to copy — it's a single pointer under the hood. Two handles
/// are equal when they point to the same C# object.
///
/// This is an unrooted reference. Keep a `GcHandle` across managed allocations
/// when no other managed reference keeps the object alive. Attach worker
/// threads and synchronize access to mutable managed state.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Il2CppObject {
    pub(crate) inner: bridge::structs::Object,
}

impl Il2CppObject {
    /// Retain this object across callbacks, allocations, or stored Rust state.
    pub fn root(&self) -> Result<crate::GcHandle> {
        crate::GcHandle::pinned(*self)
    }
    /// Unbox a managed value, checking its managed type and byte size first.
    pub fn unbox<T: Il2CppValueType>(&self) -> Result<T> {
        let class = self.class()?;
        if T::IS_REFERENCE || !class.is_valuetype() {
            return Err(Error::Bridge("unboxing requires a value type".into()));
        }
        T::validate_type(&bridge::structs::Type {
            address: class.inner.ty,
            name: format!("{}.{}", class.namespace(), class.name()),
            size: 0,
        })?;
        let ptr = unsafe { bridge::api::object_unbox(self.raw_ptr()) };
        if ptr.is_null() {
            return Err(Error::NullObject);
        }
        Ok(unsafe { ptr.cast::<T>().read_unaligned() })
    }
    /// Read a field or property by its C# name (fields take precedence).
    pub fn get<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        if self.is_null() {
            return Err(Error::NullObject);
        }
        if self.class()?.inner.field(name).is_some() {
            self.load(name)
        } else {
            self.property(name)?.get()
        }
    }
    /// Write a field or property. Rust strings become managed System.Strings.
    pub fn set(&self, name: &str, value: impl crate::IntoIl2Cpp) -> Result<()> {
        if self.is_null() {
            return Err(Error::NullObject);
        }
        let _owner = crate::GcHandle::pinned(*self)?;
        let value = value.into_il2cpp()?;
        if self.class()?.inner.field(name).is_some() {
            self.store(name, value)
        } else {
            self.property(name)?.set(value)
        }
    }
    /// Call an instance method. Use `()` as the return type for void methods.
    pub fn call<T: Il2CppValueType>(
        &self,
        selector: impl MethodSelector,
        args: impl Args,
    ) -> Result<T> {
        self.invoke(selector, args)
    }
    pub fn is_null(&self) -> bool {
        self.raw_ptr().is_null()
    }
    pub fn is_instance_of(&self, class: &Il2CppClass) -> Result<bool> {
        Ok(!self.is_null() && class.is_assignable_from(&self.class()?))
    }
    pub fn property(&self, name: &str) -> Result<crate::Property> {
        crate::Property::find(&self.class()?, name, Some(*self))
    }
    pub fn event(&self, name: &str) -> Result<crate::Event> {
        crate::Event::find(&self.class()?, name, Some(*self))
    }
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
        if self.is_null() {
            return Err(Error::NullObject);
        }
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        T::validate_type(&field.type_info)?;
        unsafe { T::load_field(&field) }
    }

    /// Write an instance field by name.
    pub fn store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()> {
        if self.is_null() {
            return Err(Error::NullObject);
        }
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        val.validate_value(&field.type_info)?;
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
        if self.is_null() {
            return Err(Error::NullObject);
        }
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
    pub fn invoke<T: Il2CppValueType>(
        &self,
        name: impl MethodSelector,
        args: impl Args,
    ) -> Result<T> {
        let method = self
            .class()?
            .select_method(name, Some(args.len()))?
            .bind(self)?;
        method.invoke(args)
    }

    /// Call a void method.
    ///
    /// ```ignore
    /// obj.invoke_void("Update", ())?;
    /// obj.invoke_void("SetHP", (999,))?;
    /// ```
    pub fn invoke_void(&self, name: impl MethodSelector, args: impl Args) -> Result<()> {
        let method = self
            .class()?
            .select_method(name, Some(args.len()))?
            .bind(self)?;
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
    pub fn method(&self, name: impl MethodSelector) -> Result<MethodInfo> {
        self.class()?.method(name)?.bind(self)
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
        if self.is_null() {
            return Err(Error::NullObject);
        }
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
        if ptr.is_null() { None } else { Some(ptr) }
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
    const IS_REFERENCE: bool = true;
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
#[derive(Clone)]
pub struct FieldHandle {
    pub(crate) field: Field,
}

impl FieldHandle {
    pub fn name(&self) -> &str {
        &self.field.name
    }
    pub fn type_name(&self) -> &str {
        &self.field.type_info.name
    }
    pub fn is_static(&self) -> bool {
        self.field.is_static
    }
    pub fn bind(&self, object: &Il2CppObject) -> Result<Self> {
        if self.is_static() {
            return Err(Error::Bridge("cannot bind a static field".into()));
        }
        let class = object.class()?;
        let parent = self.field.get_parent();
        if !unsafe { bridge::api::class_is_assignable_from(parent, class.inner.address) } {
            return Err(Error::Bridge(
                "object is incompatible with field's declaring type".into(),
            ));
        }
        let mut field = self.field.clone();
        field.instance = Some(object.raw_ptr());
        Ok(Self { field })
    }
    /// Read the field value.
    pub fn get<T: Il2CppValueType>(&self) -> Result<T> {
        T::validate_type(&self.field.type_info)?;
        unsafe { T::load_field(&self.field) }
    }

    /// Write the field value.
    pub fn set(&self, val: impl crate::IntoIl2Cpp) -> Result<()> {
        let _owner = self
            .field
            .instance
            .map(|ptr| crate::GcHandle::pinned(unsafe { Il2CppObject::from_raw(ptr) }))
            .transpose()?;
        let val = val.into_il2cpp()?;
        val.validate_value(&self.field.type_info)?;
        unsafe { <_ as Il2CppValueType>::store_field(&self.field, val) }
    }
}
