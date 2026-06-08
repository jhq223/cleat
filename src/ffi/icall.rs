//! IL2CPP internal call (iCall) resolution and invocation.
//!
//! Used for `[NativeHeader]` / `extern` methods that can't be reached
//! through managed method lookup (e.g. Unity AssetBundle loading APIs).

use std::ffi::{CString, c_void};

use crate::{Error, Result};

/// Resolve an IL2CPP internal call by its fully qualified name.
///
/// iCall names follow the format `"Assembly.Class::Method(args)"`.
///
/// Returns the raw native function pointer.
pub fn resolve(name: &str) -> Result<*mut c_void> {
    let c_name =
        CString::new(name).map_err(|_| Error::Bridge("iCall name contains null byte".into()))?;
    let ptr = unsafe { il2cpp_bridge_rs::api::resolve_icall(c_name.as_ptr()) };
    if ptr.is_null() {
        return Err(Error::Bridge(format!("iCall not found: {name}")));
    }
    Ok(ptr)
}

macro_rules! impl_icall_invoke {
    (0, $name:ident,) => {
        /// # Safety
        ///
        /// `fn_ptr` must be a valid function pointer obtained from
        /// [`resolve`] with a matching signature.
        pub unsafe fn $name(fn_ptr: *mut c_void) -> *mut c_void {
            let f: extern "C" fn() -> *mut c_void = unsafe { std::mem::transmute(fn_ptr) };
            f()
        }
    };
    ($name:ident, $($pn:ident : $pt:ty),+) => {
        /// # Safety
        ///
        /// `fn_ptr` must be a valid function pointer obtained from
        /// [`resolve`] with a matching signature.
        pub unsafe fn $name(fn_ptr: *mut c_void, $($pn: $pt),+) -> *mut c_void {
            let f: extern "C" fn($($pt),+) -> *mut c_void =
                unsafe { std::mem::transmute(fn_ptr) };
            f($($pn),+)
        }
    };
}

impl_icall_invoke!(0, invoke_0,);
impl_icall_invoke!(invoke_1, a: *mut c_void);
impl_icall_invoke!(invoke_2, a: *mut c_void, b: *mut c_void);
impl_icall_invoke!(invoke_3, a: *mut c_void, b: *mut c_void, c: *mut c_void);
impl_icall_invoke!(invoke_4, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void);
impl_icall_invoke!(invoke_5, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void);
impl_icall_invoke!(invoke_6, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void);
impl_icall_invoke!(invoke_7, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void);
impl_icall_invoke!(invoke_8, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void);
impl_icall_invoke!(invoke_9, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void);
impl_icall_invoke!(invoke_10, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void, j: *mut c_void);
impl_icall_invoke!(invoke_11, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void, j: *mut c_void, k: *mut c_void);
impl_icall_invoke!(invoke_12, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void, j: *mut c_void, k: *mut c_void, l: *mut c_void);
impl_icall_invoke!(invoke_13, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void, j: *mut c_void, k: *mut c_void, l: *mut c_void, m: *mut c_void);
impl_icall_invoke!(invoke_14, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void, j: *mut c_void, k: *mut c_void, l: *mut c_void, m: *mut c_void, n: *mut c_void);
impl_icall_invoke!(invoke_15, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void, f: *mut c_void, g: *mut c_void, h: *mut c_void, i: *mut c_void, j: *mut c_void, k: *mut c_void, l: *mut c_void, m: *mut c_void, n: *mut c_void, o: *mut c_void);
