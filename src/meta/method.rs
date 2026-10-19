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
    /// Specialize a generic method (alias of inflate).
    pub fn generic(&self, type_args: &[&Il2CppClass]) -> Result<Self> {
        self.inflate(type_args)
    }

    #[doc(hidden)]
    pub fn validate_parameter<T: Il2CppValueType>(&self, index: usize) -> Result<()> {
        let arg = self
            .inner
            .args
            .get(index)
            .ok_or_else(|| Error::Bridge("argument index out of range".into()))?;
        if arg.type_info.is_byref() {
            return Err(Error::Hook(
                "ref/out hook parameters are not supported by this macro".into(),
            ));
        }
        T::validate_type(&arg.type_info)
    }
    #[doc(hidden)]
    pub fn validate_hook_return<T: Il2CppValueType>(&self) -> Result<()> {
        if std::mem::size_of::<T>() == 0 {
            if !self.inner.return_type.matches_name("System.Void") {
                return Err(Error::Hook(
                    "hook return type must match the managed return type".into(),
                ));
            }
            Ok(())
        } else {
            T::validate_type(&self.inner.return_type)
        }
    }
    pub fn name(&self) -> &str {
        &self.inner.name
    }
    pub fn is_static(&self) -> bool {
        self.inner.is_static
    }
    pub fn parameter_count(&self) -> usize {
        self.inner.args.len()
    }
    pub fn parameter_types(&self) -> impl Iterator<Item = &str> {
        self.inner
            .args
            .iter()
            .map(|arg| arg.type_info.name.as_str())
    }
    pub fn native(&self) -> Result<crate::MethodPtr> {
        if self.inner.function.is_null() {
            return Err(Error::MethodNotFound(format!(
                "{} has no native entry point",
                self.name()
            )));
        }
        Ok(crate::MethodPtr {
            function: self.inner.function,
            method_info: self.inner.address,
        })
    }
    /// Bind an instance method to an object compatible with its declaring type.
    pub fn bind(&self, object: &crate::Il2CppObject) -> Result<Self> {
        if self.is_static() {
            return Err(Error::Bridge("cannot bind a static method".into()));
        }
        if object.is_null() {
            return Err(Error::NullObject);
        }
        let declaring = bridge::api::cache::class_from_ptr(self.inner.declaring_type)
            .ok_or_else(|| Error::Bridge("method has no declaring type".into()))?;
        if !declaring.is_assignable_from(&object.class()?.inner) {
            return Err(Error::Bridge(
                "object is incompatible with method's declaring type".into(),
            ));
        }
        let mut method = self.clone();
        method.inner.instance = Some(object.raw_ptr());
        Ok(method)
    }
    /// Resolve virtual/interface dispatch for the bound object.
    pub fn virtual_method(&self) -> Result<Self> {
        let object = self
            .inner
            .instance
            .ok_or_else(|| Error::Bridge("virtual dispatch requires an instance".into()))?;
        let ptr = unsafe { bridge::api::object_get_virtual_method(object, self.inner.address) };
        let mut inner = bridge::api::cache::method_from_ptr(ptr)
            .ok_or_else(|| Error::MethodNotFound(self.name().into()))?;
        inner.instance = Some(object);
        Ok(Self { inner })
    }
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
        let _instance = self
            .inner
            .instance
            .map(|ptr| crate::GcHandle::pinned(unsafe { crate::Object::from_raw(ptr) }))
            .transpose()?;
        let mut args = args.prepare()?;
        args.validate(&self.inner)?;
        if std::mem::size_of::<T>() != 0 {
            T::validate_type(&self.inner.return_type)?;
        }
        let arg_ptrs = args.to_arg_ptrs();
        unsafe { T::invoke_result(&self.invocation_method()?, &arg_ptrs) }
    }

    /// Call the method, ignoring any return value (void).
    pub fn invoke_void(&self, args: impl Args) -> Result<()> {
        let _instance = self
            .inner
            .instance
            .map(|ptr| crate::GcHandle::pinned(unsafe { crate::Object::from_raw(ptr) }))
            .transpose()?;
        let mut args = args.prepare()?;
        args.validate(&self.inner)?;
        let arg_ptrs = args.to_arg_ptrs();
        unsafe {
            self.invocation_method()?
                .call::<()>(&arg_ptrs)
                .map_err(Error::Bridge)
        }
    }

    /// Invoke preserving IL2CPP's boxed return object (useful for enumerators).
    pub fn invoke_boxed(&self, args: impl Args) -> Result<crate::Object> {
        let _instance = self
            .inner
            .instance
            .map(|ptr| crate::GcHandle::pinned(unsafe { crate::Object::from_raw(ptr) }))
            .transpose()?;
        let mut args = args.prepare()?;
        args.validate(&self.inner)?;
        let method = self.invocation_method()?;
        let pointers = args.to_arg_ptrs();
        if pointers.len() != method.args.len() {
            return Err(Error::Bridge("argument count mismatch".into()));
        }
        let result = bridge::api::invoke_method(
            method.address,
            if method.is_static {
                std::ptr::null_mut()
            } else {
                method.instance.ok_or(Error::NullObject)?
            },
            if pointers.is_empty() {
                std::ptr::null()
            } else {
                pointers.as_ptr()
            },
        )
        .map_err(Error::Bridge)?;
        Ok(unsafe { crate::Object::from_raw(result) })
    }

    pub(crate) fn invocation_method(&self) -> Result<bridge::structs::Method> {
        let mut method = self.inner.clone();
        if !method.is_static {
            let instance = method
                .instance
                .filter(|ptr| !ptr.is_null())
                .ok_or(Error::NullObject)?;
            // runtime_invoke receives the payload for a boxed value-type this.
            if unsafe { bridge::api::class_is_valuetype(method.declaring_type) } {
                let payload = unsafe { bridge::api::object_unbox(instance) };
                if payload.is_null() {
                    return Err(Error::NullObject);
                }
                method.instance = Some(payload);
            }
        }
        Ok(method)
    }
}

// Method metadata is immutable — safe to share between threads.
unsafe impl Send for MethodInfo {}
unsafe impl Sync for MethodInfo {}
