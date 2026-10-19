//! Managed `T[]` array wrapper.

use std::ffi::c_void;

use il2cpp_bridge_rs as bridge;

use crate::{Error, Il2CppClass, Result};

/// Wrapper around a managed `T[]` array.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Il2CppArray<T> {
    pub(crate) ptr: *mut bridge::structs::collections::Il2cppArray<T>,
}

/// An array retained by the GC, including all its reference elements.
/// Keep this snapshot across coroutine yields when the original list can change.
pub struct RootedArray<T: crate::Il2CppValueType> {
    array: Il2CppArray<T>,
    _root: crate::GcHandle,
}
impl<T: crate::Il2CppValueType> RootedArray<T> {
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.array.len()).map(|index| self.array.get(index).expect("snapshot index"))
    }
}
// The pinned GC handle preserves the address when native ownership moves.
// As with GcHandle, callers must attach threads and synchronize managed access.
unsafe impl<T: crate::Il2CppValueType + Send> Send for RootedArray<T> {}
unsafe impl<T: crate::Il2CppValueType + Sync> Sync for RootedArray<T> {}
impl<T: crate::Il2CppValueType> std::ops::Deref for RootedArray<T> {
    type Target = Il2CppArray<T>;
    fn deref(&self) -> &Self::Target {
        &self.array
    }
}

impl<T: crate::Il2CppValueType> Il2CppArray<T> {
    pub fn rooted(self) -> Result<RootedArray<T>> {
        let root = self.as_object().root()?;
        Ok(RootedArray {
            array: self,
            _root: root,
        })
    }
    /// Allocate and copy a slice. Reference elements receive GC write barriers.
    pub fn from_slice(element_class: &Il2CppClass, values: &[T]) -> Result<Self> {
        let _values = if T::IS_REFERENCE {
            values
                .iter()
                .filter_map(|value| {
                    let pointer = unsafe { value.to_arg_ptr() };
                    (!pointer.is_null()).then(|| {
                        crate::GcHandle::pinned(unsafe { crate::Object::from_raw(pointer) })
                    })
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let mut array = Self::new(element_class, values.len())?;
        let _array = crate::GcHandle::pinned(array.as_object())?;
        if T::IS_REFERENCE {
            let ty = bridge::structs::Type {
                address: element_class.inner.ty,
                name: format!("{}.{}", element_class.namespace(), element_class.name()),
                size: 0,
            };
            for (index, value) in values.iter().copied().enumerate() {
                value.validate_value(&ty)?;
                unsafe { array.set_unchecked(index, value) };
            }
        } else {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    values.as_ptr(),
                    array.data_ptr().cast_mut(),
                    values.len(),
                )
            };
        }
        Ok(array)
    }

    pub fn as_object(&self) -> crate::Object {
        unsafe { crate::Object::from_raw(self.as_ptr()) }
    }

    /// Allocate a new managed array of the given element type and size.
    pub fn new(element_class: &Il2CppClass, size: usize) -> Result<Self> {
        T::validate_type(&bridge::structs::Type {
            address: element_class.inner.ty,
            name: format!("{}.{}", element_class.namespace(), element_class.name()),
            size: 0,
        })?;
        let ptr = bridge::structs::collections::Il2cppArray::<T>::new(&element_class.inner, size);
        if ptr.is_null() {
            return Err(Error::Bridge(
                "Il2cppArray::new returned null — is IL2CPP initialized?".into(),
            ));
        }
        Ok(Self { ptr })
    }

    /// Wrap an existing raw IL2CPP array pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer to a managed `T[]` array.
    pub unsafe fn from_raw(ptr: *mut c_void) -> Self {
        Self {
            ptr: ptr as *mut bridge::structs::collections::Il2cppArray<T>,
        }
    }

    /// Returns the raw array pointer (for use with iCalls).
    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr as *mut c_void
    }

    /// Read element at `index`, returning `None` on out-of-bounds.
    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        Some(unsafe { self.get_unchecked(index) })
    }

    /// Write an element, checking bounds and applying the GC write barrier.
    pub fn set(&mut self, index: usize, value: impl crate::IntoIl2Cpp<Value = T>) -> Result<()> {
        if index >= self.len() {
            return Err(Error::Bridge("array index out of bounds".into()));
        }
        let _array = crate::GcHandle::pinned(self.as_object())?;
        let value = value.into_il2cpp()?;
        let pointer = if T::IS_REFERENCE {
            unsafe { value.to_arg_ptr() }
        } else {
            std::ptr::null_mut()
        };
        let _value = (!pointer.is_null())
            .then(|| crate::GcHandle::pinned(unsafe { crate::Object::from_raw(pointer) }))
            .transpose()?;
        let class = unsafe { bridge::api::object_get_class(self.as_ptr()) };
        let element = unsafe { bridge::api::class_get_element_class(class) };
        let element = bridge::api::cache::class_from_ptr(element)
            .ok_or_else(|| Error::Bridge("array has no element metadata".into()))?;
        value.validate_value(&bridge::structs::Type {
            address: element.ty,
            name: format!("{}.{}", element.namespace, element.name),
            size: 0,
        })?;
        unsafe { self.set_unchecked(index, value) };
        Ok(())
    }

    /// Read element at `index` without bounds checking.
    ///
    /// # Safety
    ///
    /// `index` must be less than [`len`](Self::len).
    pub unsafe fn get_unchecked(&self, index: usize) -> T {
        unsafe { (*self.ptr).get(index) }
    }

    /// Write element at `index` without bounds checking.
    ///
    /// # Safety
    ///
    /// `index` must be less than [`len`](Self::len) and value must be assignable
    /// to the array's actual element type (which can be narrower than T).
    pub unsafe fn set_unchecked(&mut self, index: usize, value: T) {
        if T::IS_REFERENCE {
            let slot = unsafe { self.data_ptr().add(index).cast_mut().cast::<*mut c_void>() };
            unsafe { bridge::api::gc_wbarrier_set_field(self.as_ptr(), slot, value.to_arg_ptr()) };
        } else {
            unsafe { (*self.ptr).set(index, value) }
        }
    }

    /// Number of elements in the array.
    pub fn len(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        unsafe { (*self.ptr).len() }
    }

    /// True when the array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copy the entire array into a Rust `Vec<T>`.
    pub fn to_vec(&self) -> Vec<T> {
        if self.ptr.is_null() {
            return Vec::new();
        }
        unsafe { (*self.ptr).to_vector() }
    }

    /// Raw pointer to the first element — skips past the
    /// `Il2CppArray` header.
    ///
    /// Use together with [`len`](Self::len) when you need to walk
    /// an array whose element type Rust doesn't know about
    /// (e.g. `Dictionary.Entry`).
    pub fn data_ptr(&self) -> *const T {
        if self.ptr.is_null() {
            return std::ptr::NonNull::<T>::dangling().as_ptr();
        }
        unsafe { (*self.ptr).get_pointer() }
    }

    /// Borrow as a Rust slice.
    ///
    /// # Safety
    ///
    /// The slice is valid until the next GC run. Don't hold it across
    /// any managed allocations or method calls.
    pub unsafe fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.data_ptr(), self.len()) }
    }

    /// Iterate over elements (bounds-checked).
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let bound = self.len();
        (0..bound).filter_map(|i| self.get(i))
    }
}

unsafe impl<T: crate::Il2CppValueType> crate::Il2CppValueType for Il2CppArray<T> {
    const IS_REFERENCE: bool = true;
    fn validate_type(ty: &bridge::structs::Type) -> Result<()> {
        let class = unsafe { bridge::api::class_from_type(ty.address) };
        if class.is_null() || unsafe { bridge::api::class_get_rank(class) } != 1 {
            return Err(Error::Bridge(format!(
                "expected one-dimensional array, found {}",
                ty.name
            )));
        }
        let element = unsafe { bridge::api::class_get_element_class(class) };
        let element = bridge::api::cache::class_from_ptr(element)
            .ok_or_else(|| Error::Bridge("array has no element metadata".into()))?;
        T::validate_type(&bridge::structs::Type {
            address: element.ty,
            name: format!("{}.{}", element.namespace, element.name),
            size: 0,
        })
    }
    unsafe fn load_field(field: &bridge::structs::Field) -> Result<Self> {
        Ok(unsafe { Self::from_raw(field.get_value::<*mut c_void>().map_err(Error::Bridge)?) })
    }
    unsafe fn store_field(field: &bridge::structs::Field, value: Self) -> Result<()> {
        unsafe { field.set_value(value.as_ptr()).map_err(Error::Bridge) }
    }
    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self.as_ptr()
    }
}
