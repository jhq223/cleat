//! Trait for types that can cross the IL2CPP FFI boundary.
//!
//! See [`Il2CppValueType`].

use std::ffi::c_void;

use il2cpp_bridge_rs as bridge;
use il2cpp_bridge_rs::structs::Field;

use crate::{Error, Result};

/// Values accepted by `Object::set` and `Class::set`.
pub trait IntoIl2Cpp {
    type Value: Il2CppValueType;
    fn into_il2cpp(self) -> Result<Self::Value>;
}
impl<T: Il2CppValueType> IntoIl2Cpp for T {
    type Value = T;
    fn into_il2cpp(self) -> Result<T> {
        Ok(self)
    }
}
impl IntoIl2Cpp for &str {
    type Value = crate::Il2CppString;
    fn into_il2cpp(self) -> Result<Self::Value> {
        crate::Il2CppString::try_new(self)
    }
}
impl IntoIl2Cpp for String {
    type Value = crate::Il2CppString;
    fn into_il2cpp(self) -> Result<Self::Value> {
        crate::Il2CppString::try_new(&self)
    }
}
impl IntoIl2Cpp for &String {
    type Value = crate::Il2CppString;
    fn into_il2cpp(self) -> Result<Self::Value> {
        crate::Il2CppString::try_new(self)
    }
}

/// Marks a type that can be shuttled across the IL2CPP FFI boundary.
///
/// # Safety
///
/// The implementor must be ABI-compatible with the corresponding C# type:
/// same size, same alignment, same field layout.
///
/// # Provided impls
///
/// | Category     | Types                                                       |
/// |-------------|-------------------------------------------------------------|
/// | Primitives   | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool` |
/// | Math         | `Vector2`, `Vector3`, `Vector4`, `Quaternion` (glam-based) |
/// | Managed      | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>`            |
/// | Custom       | `#[repr(C)]` structs via `#[cleat::value_type]`            |
pub unsafe trait Il2CppValueType: Copy + 'static {
    /// Managed references are pointer-sized; value types are passed by address.
    const IS_REFERENCE: bool = false;
    const MANAGED_NAME: Option<&'static str> = None;

    fn validate_value(&self, ty: &bridge::structs::Type) -> Result<()> {
        Self::validate_type(ty)?;
        if Self::IS_REFERENCE {
            let ptr = unsafe { self.to_arg_ptr() };
            if !ptr.is_null() {
                let expected = unsafe { bridge::api::class_from_type(ty.address) };
                let actual = unsafe { bridge::api::object_get_class(ptr) };
                if actual.is_null()
                    || !unsafe { bridge::api::class_is_assignable_from(expected, actual) }
                {
                    return Err(Error::Bridge(format!(
                        "object is not assignable to {}",
                        ty.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Check the managed shape before touching native memory.
    fn validate_type(ty: &bridge::structs::Type) -> Result<()> {
        if let Some(name) = Self::MANAGED_NAME
            && ty.name.trim_end_matches('&') != name
        {
            return Err(Error::Bridge(format!("expected {name}, found {}", ty.name)));
        }
        if ty.address.is_null() {
            return Err(Error::Bridge("missing managed type metadata".into()));
        }
        let class = unsafe { bridge::api::class_from_type(ty.address) };
        if class.is_null() {
            return Err(Error::Bridge(format!(
                "unsupported managed type {}",
                ty.name
            )));
        }
        let value_type = unsafe { bridge::api::class_is_valuetype(class) };
        if Self::IS_REFERENCE == value_type {
            return Err(Error::Bridge(format!(
                "reference/value type mismatch for {}",
                ty.name
            )));
        }
        let size = if value_type {
            (unsafe { bridge::api::class_value_size(class, std::ptr::null_mut()) }) as usize
        } else {
            std::mem::size_of::<*mut c_void>()
        };
        if size != std::mem::size_of::<Self>() {
            return Err(Error::Bridge(format!(
                "{} has size {size}, requested {}",
                ty.name,
                std::mem::size_of::<Self>()
            )));
        }
        Ok(())
    }
    /// Read a value out of an IL2CPP field.
    ///
    /// # Safety
    ///
    /// `field` must be a live `FieldInfo` pointer whose type matches `Self`.
    unsafe fn load_field(field: &Field) -> Result<Self>;

    /// Write a value into an IL2CPP field.
    ///
    /// # Safety
    ///
    /// `field` must be a live `FieldInfo` pointer whose type matches `Self`.
    unsafe fn store_field(field: &Field, val: Self) -> Result<()>;

    /// Build `Self` from the return value of an IL2CPP method call.
    ///
    /// # Safety
    /// The method, instance, argument pointers and return type must all match Self.
    unsafe fn invoke_result(
        method: &bridge::structs::Method,
        args: &[*mut c_void],
    ) -> Result<Self> {
        unsafe { method.call::<Self>(args).map_err(Error::Bridge) }
    }

    /// Convert `self` into a `*mut c_void` suitable for passing as a
    /// method argument.
    ///
    /// The default takes a pointer to `self` on the stack. Reference
    /// types (like `Il2CppObject`) override this to pass the GC-managed
    /// pointer directly.
    /// # Safety
    /// Use the returned pointer only while self and its managed referents remain live.
    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self as *const Self as *mut c_void
    }
}

// ── blanket impl for all primitive types ───────────────────────────────────

macro_rules! impl_value_type_direct {
    ($($t:ty => $name:literal),*) => {
        $(
            unsafe impl Il2CppValueType for $t {
                const MANAGED_NAME: Option<&'static str> = Some($name);
                unsafe fn load_field(field: &Field) -> Result<Self> {
                    unsafe { field.get_value::<$t>().map_err(Error::Bridge) }
                }

                unsafe fn store_field(field: &Field, val: Self) -> Result<()> {
                    unsafe { field.set_value::<$t>(val).map_err(Error::Bridge) }
                }
            }
        )*
    };
}

impl_value_type_direct!(i8 => "System.SByte", u8 => "System.Byte", i16 => "System.Int16", u16 => "System.UInt16",
    i32 => "System.Int32", u32 => "System.UInt32", i64 => "System.Int64", u64 => "System.UInt64",
    isize => "System.IntPtr", usize => "System.UIntPtr", f32 => "System.Single", f64 => "System.Double",
    bool => "System.Boolean", () => "System.Void");

unsafe impl<T: 'static> Il2CppValueType for *mut T {
    const IS_REFERENCE: bool = true;
    unsafe fn load_field(field: &Field) -> Result<Self> {
        unsafe { field.get_value::<Self>().map_err(Error::Bridge) }
    }
    unsafe fn store_field(field: &Field, value: Self) -> Result<()> {
        unsafe { field.set_value(value).map_err(Error::Bridge) }
    }
    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self.cast()
    }
}
