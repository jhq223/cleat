//! IL2CPP method-pointer resolution and invocation.
//!
//! Resolves the native `methodPointer` from the IL2CPP class method table
//! and calls it directly. This works for **all** managed methods regardless
//! of binding attribute (`[MethodImpl(MethodImplOptions.InternalCall)]` iCall
//! bindings **and** `[FreeFunction]` bindings introduced in Unity 6), because
//! every `MethodInfo` carries a `methodPointer`.
//!
//! The IL2CPP `methodPointer` calling convention appends a trailing
//! `MethodInfo*` parameter:
//!
//! ```text
//! static:   Ret fn(Arg1, Arg2, ..., MethodInfo* method)
//! instance: Ret fn(void* this, Arg1, Arg2, ..., MethodInfo* method)
//! ```
//!
//! The `invoke_method_*` (static) and `invoke_method_inst_*` (instance)
//! helpers pass this trailing parameter automatically.
//!
//! For regular C# managed methods, prefer [`Il2CppObject::invoke`] or
//! [`Il2CppClass::invoke_static`] which go through `il2cpp_runtime_invoke`.
//! Use this module only for Unity engine internal methods (`internal extern`,
//! `[FreeFunction]`) that may not have a working `invoker_method`.

use std::ffi::c_void;

use crate::{Error, Il2CppClass, Result};

/// A resolved method pointer pair: the raw `methodPointer` and its
/// associated `MethodInfo*`.
///
/// Obtain via [`resolve_method`]. Pass to `invoke_method_*` (static) or
/// `invoke_method_inst_*` (instance) helpers to call it.
#[derive(Clone, Copy, Debug)]
pub struct MethodPtr {
    /// The IL2CPP `methodPointer` — the compiled native entry point.
    pub function: *mut c_void,
    /// The `MethodInfo*` — passed as the trailing argument.
    pub method_info: *mut c_void,
}

unsafe impl Send for MethodPtr {}
unsafe impl Sync for MethodPtr {}

/// Resolve a managed method by class name and method name.
///
/// Goes through the IL2CPP class method table and returns the raw
/// `methodPointer` + `MethodInfo*`. Works for both iCall-bound **and**
/// `[FreeFunction]`-bound methods (Unity 6+).
///
/// # Example
///
/// ```ignore
/// let m = cleat::resolve_method(
///     "UnityEngine.AssetBundleModule.dll",
///     "UnityEngine.AssetBundle",
///     "LoadFromMemoryAsync_Internal",
/// )?;
/// let req = unsafe {
///     cleat::invoke_method_2(m, arr.as_ptr(), std::ptr::null_mut())
/// };
/// ```
pub fn resolve_method(assembly: &str, class_name: &str, method_name: &str) -> Result<MethodPtr> {
    let class = Il2CppClass::find_with(class_name, assembly)?;
    let method = class.method_ptr(method_name)?;
    if method.function.is_null() {
        return Err(Error::MethodNotFound(format!(
            "{class_name}.{method_name} (null methodPointer)"
        )));
    }
    Ok(MethodPtr {
        function: method.function,
        method_info: method.address,
    })
}

// ── static method-pointer invokers ───────────────────────────────────────
//
// Signature: fn(Arg1, ..., MethodInfo*) -> *mut c_void

macro_rules! impl_method_invoke {
    (0, $name:ident,) => {
        /// # Safety
        ///
        /// `m` must be a valid [`MethodPtr`] obtained from [`resolve_method`]
        /// for a static method with no parameters.
        pub unsafe fn $name(m: MethodPtr) -> *mut c_void {
            let f: extern "C" fn(*mut c_void) -> *mut c_void =
                unsafe { std::mem::transmute(m.function) };
            f(m.method_info)
        }
    };
    ($name:ident, $($pn:ident : $pt:ty),+) => {
        /// # Safety
        ///
        /// `m` must be a valid [`MethodPtr`] obtained from [`resolve_method`]
        /// for a static method with matching parameter count and types.
        pub unsafe fn $name(m: MethodPtr, $($pn: $pt),+) -> *mut c_void {
            let f: extern "C" fn($($pt),+ , *mut c_void) -> *mut c_void =
                unsafe { std::mem::transmute(m.function) };
            f($($pn),+ , m.method_info)
        }
    };
}

impl_method_invoke!(0, invoke_method_0,);
impl_method_invoke!(invoke_method_1, a: *mut c_void);
impl_method_invoke!(invoke_method_2, a: *mut c_void, b: *mut c_void);
impl_method_invoke!(invoke_method_3, a: *mut c_void, b: *mut c_void, c: *mut c_void);
impl_method_invoke!(invoke_method_4, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void);
impl_method_invoke!(invoke_method_5, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void);

// ── instance method-pointer invokers ────────────────────────────────────
//
// Signature: fn(void* this, Arg1, ..., MethodInfo*) -> *mut c_void

macro_rules! impl_method_invoke_inst {
    (0, $name:ident,) => {
        /// # Safety
        ///
        /// `m` must be a valid [`MethodPtr`] obtained from [`resolve_method`]
        /// for an instance method with no parameters.
        pub unsafe fn $name(m: MethodPtr, this: *mut c_void) -> *mut c_void {
            let f: extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
                unsafe { std::mem::transmute(m.function) };
            f(this, m.method_info)
        }
    };
    ($name:ident, $($pn:ident : $pt:ty),+) => {
        /// # Safety
        ///
        /// `m` must be a valid [`MethodPtr`] obtained from [`resolve_method`]
        /// for an instance method with matching parameter count and types.
        pub unsafe fn $name(m: MethodPtr, this: *mut c_void, $($pn: $pt),+) -> *mut c_void {
            let f: extern "C" fn(*mut c_void, $($pt),+ , *mut c_void) -> *mut c_void =
                unsafe { std::mem::transmute(m.function) };
            f(this, $($pn),+ , m.method_info)
        }
    };
}

impl_method_invoke_inst!(0, invoke_method_inst_0,);
impl_method_invoke_inst!(invoke_method_inst_1, a: *mut c_void);
impl_method_invoke_inst!(invoke_method_inst_2, a: *mut c_void, b: *mut c_void);
impl_method_invoke_inst!(invoke_method_inst_3, a: *mut c_void, b: *mut c_void, c: *mut c_void);
impl_method_invoke_inst!(invoke_method_inst_4, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void);
impl_method_invoke_inst!(invoke_method_inst_5, a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void, e: *mut c_void);

// ── tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_ptr_is_copy_send_sync() {
        fn assert_copy<T: Copy>() {}
        fn assert_send<T: Send + Sync>() {}
        assert_copy::<MethodPtr>();
        assert_send::<MethodPtr>();
    }

    #[test]
    fn method_ptr_debug_format() {
        let m = MethodPtr {
            function: 0x1000 as *mut c_void,
            method_info: 0x2000 as *mut c_void,
        };
        let s = format!("{m:?}");
        assert!(s.contains("MethodPtr"));
    }
}
