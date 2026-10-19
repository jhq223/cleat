//! Managed delegates; construction and combination use System.Delegate.
use crate::{Args, Class, GcHandle, Il2CppArray, Il2CppValueType, MethodInfo, Object, Result};

pub struct Delegate {
    root: GcHandle,
}
impl Delegate {
    /// Create a delegate from a static method or an instance-bound method.
    pub fn new(delegate_type: &Class, method: &MethodInfo) -> Result<Self> {
        let ty = delegate_type.type_object()?;
        let method_object = unsafe { Object::from_raw(method.inner.get_object()) };
        let method_root = GcHandle::new(method_object)?;
        let class = Class::find("System.Delegate")?;
        let object = if method.is_static() {
            class.call(
                (
                    "CreateDelegate",
                    ["System.Type", "System.Reflection.MethodInfo"],
                ),
                (ty, method_root.object()),
            )?
        } else {
            let instance = method.inner.instance.ok_or(crate::Error::NullObject)?;
            let instance = unsafe { Object::from_raw(instance) };
            class.call(
                (
                    "CreateDelegate",
                    [
                        "System.Type",
                        "System.Object",
                        "System.Reflection.MethodInfo",
                    ],
                ),
                (ty, instance, method_root.object()),
            )?
        };
        Self::from_object(object)
    }
    pub fn from_object(object: Object) -> Result<Self> {
        if !object.is_instance_of(&Class::find("System.Delegate")?)? {
            return Err(crate::Error::Bridge(
                "object is not a managed delegate".into(),
            ));
        }
        Ok(Self {
            root: GcHandle::new(object)?,
        })
    }
    pub fn as_object(&self) -> Object {
        self.root.object()
    }
    pub fn invoke<T: Il2CppValueType>(&self, args: impl Args) -> Result<T> {
        self.as_object().call("Invoke", args)
    }
    pub fn combine(&self, other: &Self) -> Result<Self> {
        let object = Class::find("System.Delegate")?.call(
            ("Combine", ["System.Delegate", "System.Delegate"]),
            (self.as_object(), other.as_object()),
        )?;
        Self::from_object(object)
    }
    pub fn remove(&self, other: &Self) -> Result<Option<Self>> {
        let object: Object = Class::find("System.Delegate")?.call(
            ("Remove", ["System.Delegate", "System.Delegate"]),
            (self.as_object(), other.as_object()),
        )?;
        if object.is_null() {
            Ok(None)
        } else {
            Self::from_object(object).map(Some)
        }
    }
    pub fn invocation_list(&self) -> Result<Vec<Self>> {
        let list: Il2CppArray<Object> = self.as_object().call("GetInvocationList", ())?;
        let _root = GcHandle::pinned(unsafe { Object::from_raw(list.as_ptr()) })?;
        list.iter().map(Self::from_object).collect()
    }
}
impl From<&Delegate> for Object {
    fn from(delegate: &Delegate) -> Self {
        delegate.as_object()
    }
}
