//! Trait for types that can cross the IL2CPP FFI boundary.
//!
//! See [`Il2CppValueType`].

use std::ffi::c_void;

use il2cpp_bridge_rs as bridge;
use il2cpp_bridge_rs::structs::Field;

use crate::{Error, Result};

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
    /// Most types keep the default; `Il2CppList<T>` overrides this because
    /// it is larger than a register.
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
    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self as *const Self as *mut c_void
    }
}

// ── blanket impl for all primitive types ───────────────────────────────────

macro_rules! impl_value_type_direct {
    ($($t:ty),*) => {
        $(
            unsafe impl Il2CppValueType for $t {
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

impl_value_type_direct!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64, bool);
