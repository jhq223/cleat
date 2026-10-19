//! Assembly discovery without assuming Assembly-CSharp exists.
use crate::{Class, Error, Result, bridge};
use std::sync::Arc;

#[derive(Clone)]
pub struct Assembly {
    inner: Arc<bridge::structs::Assembly>,
}
impl Assembly {
    pub fn find(name: &str) -> Result<Self> {
        bridge::api::cache::assembly(name)
            .map(|inner| Self { inner })
            .ok_or_else(|| Error::AssemblyNotFound(name.into()))
    }
    pub fn all() -> Vec<Self> {
        let mut assemblies: Vec<_> = bridge::api::cache::CACHE
            .assemblies
            .iter()
            .map(|entry| Self {
                inner: entry.value().clone(),
            })
            .collect();
        assemblies.sort_by(|a, b| a.name().cmp(b.name()));
        assemblies
    }
    pub fn name(&self) -> &str {
        &self.inner.name
    }
    pub fn class(&self, name: &str) -> Result<Class> {
        self.inner
            .class(name)
            .map(|inner| Class { inner })
            .ok_or_else(|| Error::ClassNotFound(name.into()))
    }
    pub fn classes(&self) -> Vec<Class> {
        self.inner
            .classes
            .iter()
            .cloned()
            .map(|inner| Class { inner })
            .collect()
    }
}
