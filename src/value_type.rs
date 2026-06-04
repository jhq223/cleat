use crate::{Error, Result};
use il2cpp_bridge_rs::structs::Field;
use std::ffi::c_void;

/// Marks a type that can be shuttled across the IL2CPP FFI boundary.
///
/// # Safety
///
/// The implementor must be ABI-compatible with the corresponding C# type:
/// same size, same alignment, same field layout. The bridge's
/// `get_value::<Self>` / `set_value::<Self>` must be legal to call with
/// this type.
///
/// # Provided impls
///
/// | Category     | Types                                                       |
/// |-------------|-------------------------------------------------------------|
/// | Primitives   | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool` |
/// | Math         | `Vector2`, `Vector3`, `Vector4`, `Quaternion` (glam-based) |
/// | Managed      | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>`            |
/// | Your structs | Anything `#[repr(C)]` + `#[cleat::value_type]`              |
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
    /// # Safety
    ///
    /// `method` must be a resolved `MethodInfo`. `args` must be a valid
    /// pointer array. The return value must be ABI-compatible with `Self`.
    ///
    /// Most types can keep the default implementation; `Il2CppList<T>`
    /// overrides this because it's a value type that's larger than a
    /// register.
    unsafe fn invoke_result(
        method: &il2cpp_bridge_rs::structs::Method,
        args: &[*mut std::ffi::c_void],
    ) -> Result<Self> {
        unsafe { method.call::<Self>(args).map_err(Error::Bridge) }
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

                unsafe fn store_field(field: &il2cpp_bridge_rs::structs::Field, val: Self) -> Result<()> {
                    unsafe { field.set_value::<$t>(val).map_err(Error::Bridge) }
                }
            }
        )*
    };
}

impl_value_type_direct!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64, bool);

// ── Args trait ─────────────────────────────────────────────────────────────

/// Converts a tuple of arguments into an array of FFI raw pointers.
///
/// # Safety
///
/// `to_arg_ptrs` returns pointers into `self`. They're only valid until the
/// next mutation of the tuple (which in practice means "for the duration of
/// the call"). Don't stash the returned `Vec` — pass it directly to an
/// IL2CPP method call and drop it.
pub unsafe trait Args {
    /// How many arguments in the tuple.
    fn len(&self) -> usize;

    /// True when there are no arguments.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pack each argument as `*mut c_void`.
    /// Pointers are transient — consume immediately.
    fn to_arg_ptrs(&self) -> Vec<*mut c_void>;
}

unsafe impl Args for () {
    fn len(&self) -> usize {
        0
    }
    fn to_arg_ptrs(&self) -> Vec<*mut c_void> {
        Vec::new()
    }
}

// ── tuple impls: tuples of 1 through 16 elements ──────────────────────────

macro_rules! impl_args_tuple {
    ($($T:ident),+) => {
        #[allow(non_snake_case)]
        unsafe impl<$($T: Il2CppValueType),*> Args for ($($T,)*) {
            fn len(&self) -> usize {
                let ($($T,)*) = self;
                let mut n = 0;
                $(let _ = $T; n += 1;)*
                n
            }

            fn to_arg_ptrs(&self) -> Vec<*mut c_void> {
                let ($($T,)*) = self;
                vec![$($T as *const $T as *mut c_void),*]
            }
        }
    };
}

impl_args_tuple!(A);
impl_args_tuple!(A, B);
impl_args_tuple!(A, B, C);
impl_args_tuple!(A, B, C, D);
impl_args_tuple!(A, B, C, D, E);
impl_args_tuple!(A, B, C, D, E, F);
impl_args_tuple!(A, B, C, D, E, F, G);
impl_args_tuple!(A, B, C, D, E, F, G, H);
impl_args_tuple!(A, B, C, D, E, F, G, H, I);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
