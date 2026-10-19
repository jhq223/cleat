//! Managed property access, including indexers.
use crate::{Args, Error, Il2CppClass, Il2CppObject, Il2CppValueType, MethodInfo, Result};

#[derive(Clone)]
pub struct Property {
    name: String,
    getter: Option<MethodInfo>,
    setter: Option<MethodInfo>,
}

impl Property {
    pub(crate) fn find(
        class: &Il2CppClass,
        name: &str,
        instance: Option<Il2CppObject>,
    ) -> Result<Self> {
        let cname =
            std::ffi::CString::new(name).map_err(|_| Error::PropertyNotFound(name.into()))?;
        let mut current = Some(class.clone());
        let mut ptr = std::ptr::null_mut();
        while let Some(class) = current {
            ptr = unsafe {
                il2cpp_bridge_rs::api::class_get_property_from_name(
                    class.inner.address,
                    cname.as_ptr(),
                )
            };
            if !ptr.is_null() {
                break;
            }
            current = class.parent();
        }
        if ptr.is_null() {
            return Err(Error::PropertyNotFound(name.into()));
        }
        let getter = il2cpp_bridge_rs::api::cache::method_from_ptr(unsafe {
            il2cpp_bridge_rs::api::property_get_get_method(ptr)
        });
        let setter = il2cpp_bridge_rs::api::cache::method_from_ptr(unsafe {
            il2cpp_bridge_rs::api::property_get_set_method(ptr)
        });
        let bind = |method: il2cpp_bridge_rs::structs::Method| -> Result<MethodInfo> {
            let method = MethodInfo { inner: method };
            match instance {
                Some(object) if !method.is_static() => method.bind(&object),
                _ => Ok(method),
            }
        };
        Ok(Self {
            name: name.into(),
            getter: getter.map(&bind).transpose()?,
            setter: setter.map(bind).transpose()?,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn can_read(&self) -> bool {
        self.getter.is_some()
    }
    pub fn can_write(&self) -> bool {
        self.setter.is_some()
    }
    pub fn getter(&self) -> Option<&MethodInfo> {
        self.getter.as_ref()
    }
    pub fn setter(&self) -> Option<&MethodInfo> {
        self.setter.as_ref()
    }
    pub fn get<T: Il2CppValueType>(&self) -> Result<T> {
        self.get_with(())
    }
    pub fn set(&self, value: impl crate::IntoIl2Cpp) -> Result<()> {
        self.set_with((value,))
    }
    /// Supply the indexer arguments, e.g. `(index,)`.
    pub fn get_with<T: Il2CppValueType>(&self, args: impl Args) -> Result<T> {
        self.getter
            .as_ref()
            .ok_or_else(|| Error::Bridge(format!("{} has no getter", self.name)))?
            .invoke(args)
    }
    /// Supply indexer arguments followed by the new value.
    pub fn set_with(&self, args: impl Args) -> Result<()> {
        self.setter
            .as_ref()
            .ok_or_else(|| Error::Bridge(format!("{} has no setter", self.name)))?
            .invoke_void(args)
    }
}
