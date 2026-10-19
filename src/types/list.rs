//! Managed indexed collections (`List<T>`, `Collection<T>`, and similar types).
//! Reads and mutations use managed methods, without depending on private fields.
use crate::{Error, Il2CppObject, Il2CppValueType, Result, bridge};
use std::{ffi::c_void, marker::PhantomData};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Il2CppList<T> {
    object: Il2CppObject,
    marker: PhantomData<T>,
}

impl<T: Il2CppValueType> TryFrom<Il2CppObject> for Il2CppList<T> {
    type Error = Error;
    fn try_from(object: Il2CppObject) -> Result<Self> {
        let method = object.method(("get_Item", 1usize))?.invocation_method()?;
        i32::validate_type(&method.args[0].type_info)?;
        T::validate_type(&method.return_type)?;
        object
            .method(("get_Count", 0usize))?
            .validate_hook_return::<i32>()?;
        Ok(Self {
            object,
            marker: PhantomData,
        })
    }
}
impl<T: Il2CppValueType> Il2CppList<T> {
    /// # Safety
    /// `ptr` must be a live List<T> whose element ABI matches T, or null.
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
        let size: i32 = self.object.call("get_Count", ())?;
        usize::try_from(size).map_err(|_| Error::Bridge("negative list count".into()))
    }
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    pub fn get(&self, index: usize) -> Result<Option<T>> {
        if index >= self.len()? {
            return Ok(None);
        }
        self.object.call("get_Item", (index as i32,)).map(Some)
    }
    pub fn set(&self, index: usize, value: impl crate::IntoIl2Cpp<Value = T>) -> Result<()> {
        let index =
            i32::try_from(index).map_err(|_| Error::Bridge("list index exceeds Int32".into()))?;
        self.object.call("set_Item", (index, value))
    }
    pub fn push(&self, value: impl crate::IntoIl2Cpp<Value = T>) -> Result<()> {
        self.object.call("Add", (value,))
    }
    pub fn remove(&self, value: impl crate::IntoIl2Cpp<Value = T>) -> Result<bool> {
        self.object.call("Remove", (value,))
    }
    pub fn clear(&self) -> Result<()> {
        self.object.call("Clear", ())
    }
    /// Copy the collection using `CopyTo`, or its indexer if `CopyTo` was stripped.
    /// Supports indexed collections such as `ObservableCollection<T>` which do
    /// not have `List<T>.ToArray`. Call on the collection's owning thread.
    pub fn to_vec(&self) -> Result<Vec<T>> {
        Ok(self.snapshot()?.to_vec())
    }
    /// Copy into a GC-rooted managed array. Removed list items remain alive
    /// while this snapshot is held, including across coroutine yields.
    pub fn snapshot(&self) -> Result<super::array::RootedArray<T>> {
        let _collection = self.object.root()?;
        let count = self.len()?;
        let getter = self.object.method(("get_Item", 1usize))?;
        let element = unsafe { bridge::api::class_from_type(getter.inner.return_type.address) };
        let inner = bridge::api::cache::class_from_ptr(element)
            .ok_or_else(|| Error::Bridge("collection has no element metadata".into()))?;
        let mut array = crate::Il2CppArray::<T>::new(&crate::Class { inner }, count)?;
        let snapshot = array.rooted()?;
        if count == 0 {
            return Ok(snapshot);
        }
        match self.object.method(("CopyTo", 2usize)) {
            Ok(copy) => {
                copy.invoke_void((array, 0i32))?;
            }
            Err(Error::MethodNotFound(_)) => {
                for index in 0..count {
                    array.set(index, getter.invoke::<T>((index as i32,))?)?;
                }
            }
            Err(error) => return Err(error),
        }
        Ok(snapshot)
    }
    pub fn iter(&self) -> Result<std::vec::IntoIter<T>> {
        Ok(self.to_vec()?.into_iter())
    }
}
unsafe impl<T: Il2CppValueType> Il2CppValueType for Il2CppList<T> {
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
