//! Optional, version-specific managed class registration implemented in Rust.
mod builder;
mod decoder;
mod enumerator;
mod layout;
mod runtime;
pub use builder::{ClassBuilder, ManagedMethod};
pub use enumerator::{UnityCoroutine, YieldContext, YieldInstruction};
pub use runtime::{InjectionRuntime, InternalSymbols};

/// Resolve the C# type represented by a Rust FFI value.
pub fn type_of<T: crate::Il2CppValueType>() -> crate::Result<crate::Class> {
    let name = T::MANAGED_NAME.or_else(|| (std::any::TypeId::of::<T>() == std::any::TypeId::of::<crate::Object>()).then_some("System.Object"))
        .ok_or_else(|| crate::Error::Bridge("this value type needs an explicit managed name (#[cleat::value_type(\"Namespace.Type\")])".into()))?;
    crate::Class::find(name)
}

/// # Safety
/// Used by generated invokers; ptr must represent the corresponding managed argument.
#[doc(hidden)]
pub unsafe fn read_argument<T: crate::Il2CppValueType>(ptr: *mut std::ffi::c_void) -> T {
    if T::IS_REFERENCE {
        unsafe { (&ptr as *const *mut std::ffi::c_void).cast::<T>().read() }
    } else {
        unsafe { ptr.cast::<T>().read_unaligned() }
    }
}
/// # Safety
/// method must have the declared return type T and be a valid native MethodInfo.
#[doc(hidden)]
pub unsafe fn box_return<T: crate::Il2CppValueType>(
    method: *mut std::ffi::c_void,
    value: T,
) -> *mut std::ffi::c_void {
    if std::mem::size_of::<T>() == 0 {
        return std::ptr::null_mut();
    }
    if T::IS_REFERENCE {
        return unsafe { value.to_arg_ptr() };
    }
    unsafe {
        let ty = crate::bridge::api::method_get_return_type(method);
        let class = crate::bridge::api::class_from_type(ty);
        crate::bridge::api::value_box(class, (&value as *const T).cast_mut().cast())
    }
}
