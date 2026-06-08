//! IL2CPP method handle — generic specialisation and invocation.

use il2cpp_bridge_rs as bridge;

use crate::{Args, Error, Il2CppClass, Il2CppValueType, Result};

/// Handle to an IL2CPP method.
///
/// Thread-safe — method metadata is immutable once loaded.
/// Instance methods are automatically bound to `this` when obtained
/// through [`Il2CppObject::method`](crate::Il2CppObject::method).
#[derive(Clone)]
pub struct MethodInfo {
    pub(crate) inner: bridge::structs::Method,
}

impl MethodInfo {
    /// Specialise a generic method by supplying concrete type arguments.
    ///
    /// The instance binding (`this`) is carried over from the original
    /// handle automatically.
    ///
    /// ```ignore
    /// let data: Il2CppObject = obj
    ///     .method("GetMasterData")?
    ///     .inflate(&[&servant_class])?
    ///     .invoke(())?;
    /// ```
    pub fn inflate(&self, type_args: &[&Il2CppClass]) -> Result<Self> {
        let classes: Vec<&bridge::structs::Class> = type_args.iter().map(|c| &c.inner).collect();
        let mut inflated = self.inner.inflate(&classes).map_err(Error::Bridge)?;
        inflated.instance = self.inner.instance;
        Ok(Self { inner: inflated })
    }

    /// Call the method and get the return value.
    ///
    /// `args` is a tuple — `()`, `(42,)`, `(1, 2.0)`, etc.
    pub fn invoke<T: Il2CppValueType>(&self, args: impl Args) -> Result<T> {
        let arg_ptrs = args.to_arg_ptrs();
        unsafe { T::invoke_result(&self.inner, &arg_ptrs) }
    }

    /// Call the method, ignoring any return value (void).
    pub fn invoke_void(&self, args: impl Args) -> Result<()> {
        let arg_ptrs = args.to_arg_ptrs();
        unsafe { self.inner.call::<()>(&arg_ptrs).map_err(Error::Bridge) }
    }
}

// Method metadata is immutable — safe to share between threads.
unsafe impl Send for MethodInfo {}
unsafe impl Sync for MethodInfo {}
