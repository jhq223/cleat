//! Dictionary access through managed methods, independent of private layouts.
use crate::{Error, GcHandle, Il2CppObject, Il2CppValueType, Result, bridge};
use std::{ffi::c_void, marker::PhantomData, mem::MaybeUninit};

/// A reference to a managed `Dictionary<K,V>`. No field offsets are assumed.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Il2CppDictionary<K, V> {
    object: Il2CppObject,
    marker: PhantomData<(K, V)>,
}

impl<K: Il2CppValueType, V: Il2CppValueType> TryFrom<Il2CppObject> for Il2CppDictionary<K, V> {
    type Error = Error;
    fn try_from(object: Il2CppObject) -> Result<Self> {
        let method = object
            .method(("TryGetValue", 2usize))?
            .invocation_method()?;
        K::validate_type(&method.args[0].type_info)?;
        V::validate_type(&method.args[1].type_info)?;
        bool::validate_type(&method.return_type)?;
        if !method.args[1].type_info.is_byref() {
            return Err(Error::Bridge(
                "dictionary output must be by-reference".into(),
            ));
        }
        Ok(Self {
            object,
            marker: PhantomData,
        })
    }
}

impl<K: Il2CppValueType, V: Il2CppValueType> Il2CppDictionary<K, V> {
    /// # Safety
    /// The object must be a live Dictionary whose key/value types match K/V.
    pub unsafe fn from_raw(ptr: *mut c_void) -> Self {
        Self {
            object: unsafe { Il2CppObject::from_raw(ptr) },
            marker: PhantomData,
        }
    }
    pub fn as_object(&self) -> Il2CppObject {
        self.object
    }
    pub fn len(&self) -> Result<usize> {
        let count: i32 = self.object.call("get_Count", ())?;
        usize::try_from(count).map_err(|_| Error::Bridge("negative dictionary count".into()))
    }
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    pub fn contains_key(&self, key: impl crate::IntoIl2Cpp<Value = K>) -> Result<bool> {
        self.object.call("ContainsKey", (key,))
    }
    /// Uses TryGetValue in one managed call; absent keys return None.
    pub fn get(&self, key: impl crate::IntoIl2Cpp<Value = K>) -> Result<Option<V>> {
        let _dictionary = GcHandle::pinned(self.object)?;
        let key = key.into_il2cpp()?;
        let _key = if K::IS_REFERENCE && !unsafe { key.to_arg_ptr() }.is_null() {
            Some(GcHandle::pinned(unsafe {
                Il2CppObject::from_raw(key.to_arg_ptr())
            })?)
        } else {
            None
        };
        let method = self
            .object
            .method(("TryGetValue", 2usize))?
            .invocation_method()?;
        key.validate_value(&method.args[0].type_info)?;
        bool::validate_type(&method.return_type)?;
        if !method.args[1].type_info.is_byref() {
            return Err(Error::Bridge(
                "TryGetValue output is not by-reference".into(),
            ));
        }
        V::validate_type(&method.args[1].type_info)?;
        let mut value = MaybeUninit::<V>::uninit();
        let found: bool = unsafe {
            method
                .call(&[key.to_arg_ptr(), value.as_mut_ptr().cast()])
                .map_err(Error::Bridge)?
        };
        if found {
            Ok(Some(unsafe { value.assume_init() }))
        } else {
            Ok(None)
        }
    }
    /// Insert or replace a key, preserving the managed comparer and GC barriers.
    pub fn set(
        &self,
        key: impl crate::IntoIl2Cpp<Value = K>,
        value: impl crate::IntoIl2Cpp<Value = V>,
    ) -> Result<()> {
        self.object.call("set_Item", (key, value))
    }
    pub fn remove(&self, key: impl crate::IntoIl2Cpp<Value = K>) -> Result<bool> {
        self.object.call("Remove", (key,))
    }
    pub fn clear(&self) -> Result<()> {
        self.object.call("Clear", ())
    }
    /// Enumerate with the managed enumerator. Concurrent mutations report errors.
    pub fn iter(&self) -> Result<DictionaryIter<K, V>> {
        let dictionary = GcHandle::new(self.object)?;
        let enumerator = self
            .object
            .method(("GetEnumerator", 0usize))?
            .invoke_boxed(())?;
        Ok(DictionaryIter {
            enumerator: GcHandle::pinned(enumerator)?,
            _dictionary: dictionary,
            done: false,
            marker: PhantomData,
        })
    }
    pub fn to_vec(&self) -> Result<Vec<(K, V)>> {
        self.iter()?.collect()
    }
}

unsafe impl<K: Il2CppValueType, V: Il2CppValueType> Il2CppValueType for Il2CppDictionary<K, V> {
    const IS_REFERENCE: bool = true;
    unsafe fn load_field(field: &bridge::structs::Field) -> Result<Self> {
        Ok(unsafe { Self::from_raw(field.get_value::<*mut c_void>().map_err(Error::Bridge)?) })
    }
    unsafe fn store_field(field: &bridge::structs::Field, value: Self) -> Result<()> {
        unsafe {
            field
                .set_value(value.object.raw_ptr())
                .map_err(Error::Bridge)
        }
    }
    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self.object.raw_ptr()
    }
}

pub struct DictionaryIter<K, V> {
    enumerator: GcHandle,
    _dictionary: GcHandle,
    done: bool,
    marker: PhantomData<(K, V)>,
}
impl<K: Il2CppValueType, V: Il2CppValueType> Iterator for DictionaryIter<K, V> {
    type Item = Result<(K, V)>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let result = (|| -> Result<Option<(K, V)>> {
            let enumerator = self.enumerator.object();
            if !enumerator.call::<bool>("MoveNext", ())? {
                return Ok(None);
            }
            let pair = enumerator
                .method(("get_Current", 0usize))?
                .invoke_boxed(())?;
            let pair = GcHandle::pinned(pair)?;
            Ok(Some((
                pair.object().call("get_Key", ())?,
                pair.object().call("get_Value", ())?,
            )))
        })();
        match result {
            Ok(Some(pair)) => Some(Ok(pair)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }
}
impl<K, V> Drop for DictionaryIter<K, V> {
    fn drop(&mut self) {
        if let Ok(dispose) = self.enumerator.object().method(("Dispose", 0usize))
            && let Err(error) = dispose.invoke_void(())
        {
            log::debug!("dictionary enumerator Dispose: {error}");
        }
    }
}
