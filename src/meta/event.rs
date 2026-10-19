//! Event access through managed add/remove/raise methods.
use crate::{Args, Error, Il2CppClass, Il2CppObject, Il2CppValueType, MethodInfo, Result};

#[derive(Clone)]
pub struct Event {
    name: String,
    add: Option<MethodInfo>,
    remove: Option<MethodInfo>,
    raise: Option<MethodInfo>,
}
impl Event {
    pub(crate) fn find(
        class: &Il2CppClass,
        name: &str,
        instance: Option<Il2CppObject>,
    ) -> Result<Self> {
        let accessor = |prefix: &str| -> Result<Option<MethodInfo>> {
            match class.method(format!("{prefix}_{name}").as_str()) {
                Ok(method) => Ok(Some(match instance {
                    Some(object) if !method.is_static() => method.bind(&object)?,
                    _ => method,
                })),
                Err(Error::MethodNotFound(_)) => Ok(None),
                Err(error) => Err(error),
            }
        };
        let event = Self {
            name: name.into(),
            add: accessor("add")?,
            remove: accessor("remove")?,
            raise: accessor("raise")?,
        };
        if event.add.is_none() && event.remove.is_none() && event.raise.is_none() {
            return Err(Error::EventNotFound(name.into()));
        }
        Ok(event)
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn add(&self, delegate: impl Into<Il2CppObject>) -> Result<()> {
        self.add
            .as_ref()
            .ok_or_else(|| Error::Bridge(format!("{} has no add accessor", self.name)))?
            .invoke_void((delegate.into(),))
    }
    pub fn remove(&self, delegate: impl Into<Il2CppObject>) -> Result<()> {
        self.remove
            .as_ref()
            .ok_or_else(|| Error::Bridge(format!("{} has no remove accessor", self.name)))?
            .invoke_void((delegate.into(),))
    }
    /// Only works when the runtime exposes a raise accessor.
    pub fn raise<T: Il2CppValueType>(&self, args: impl Args) -> Result<T> {
        self.raise
            .as_ref()
            .ok_or_else(|| Error::Bridge(format!("{} has no raise accessor", self.name)))?
            .invoke(args)
    }
}
