//! Managed `T[]` array wrapper.

use std::ffi::c_void;

use il2cpp_bridge_rs as bridge;

use crate::{Error, Il2CppClass, Result};

/// Wrapper around a managed `T[]` array.
pub struct Il2CppArray<T> {
    pub(crate) ptr: *mut bridge::structs::collections::Il2cppArray<T>,
}

impl<T: Copy + 'static> Il2CppArray<T> {
    /// Allocate a new managed array of the given element type and size.
    pub fn new(element_class: &Il2CppClass, size: usize) -> Result<Self> {
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
    /// `index` must be less than [`len`](Self::len).
    pub unsafe fn set_unchecked(&mut self, index: usize, value: T) {
        unsafe { (*self.ptr).set(index, value) }
    }

    /// Number of elements in the array.
    pub fn len(&self) -> usize {
        unsafe { (*self.ptr).len() }
    }

    /// True when the array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copy the entire array into a Rust `Vec<T>`.
    pub fn to_vec(&self) -> Vec<T> {
        unsafe { (*self.ptr).to_vector() }
    }

    /// Raw pointer to the first element — skips past the
    /// `Il2CppArray` header.
    ///
    /// Use together with [`len`](Self::len) when you need to walk
    /// an array whose element type Rust doesn't know about
    /// (e.g. `Dictionary.Entry`).
    pub fn data_ptr(&self) -> *const T {
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
