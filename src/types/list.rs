//! Direct memory mapping of .NET's `List<T>`.

use std::ffi::c_void;
use std::marker::PhantomData;

use il2cpp_bridge_rs as bridge;
use il2cpp_bridge_rs::structs::Field;

use super::Il2CppValueType;
use crate::{Error, Result};

/// Direct memory mapping of .NET's `List<T>`.
///
/// This is a `#[repr(C)]` struct laid out to match the managed
/// `System.Collections.Generic.List<T>` fields exactly.
///
/// Mutations should go through C# method calls (`Add`, `Remove`, etc.) —
/// writing via [`store_field`](Il2CppValueType::store_field) is blocked.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Il2CppList<T: Copy> {
    pub klass: *mut c_void,
    pub monitor: *mut c_void,
    pub items: *mut c_void,
    pub size: i32,
    pub version: i32,
    pub _phantom: PhantomData<T>,
}

unsafe impl<T: Copy + 'static> Il2CppValueType for Il2CppList<T> {
    unsafe fn load_field(field: &Field) -> Result<Self> {
        let ptr = unsafe { field.get_value::<*mut c_void>().map_err(Error::Bridge)? };
        Ok(unsafe { std::ptr::read(ptr as *const Self) })
    }

    #[cold]
    unsafe fn store_field(_field: &Field, _val: Self) -> Result<()> {
        Err(Error::Bridge(
            "Il2CppList mutation should be done through C# methods (Add/Remove)".into(),
        ))
    }

    unsafe fn invoke_result(
        method: &bridge::structs::Method,
        args: &[*mut c_void],
    ) -> Result<Self> {
        let ptr = unsafe { method.call::<*mut c_void>(args).map_err(Error::Bridge)? };
        Ok(unsafe { std::ptr::read(ptr as *const Self) })
    }
}

impl<T: Copy + 'static> Il2CppList<T> {
    fn items_ptr(&self) -> *mut bridge::structs::collections::Il2cppArray<T> {
        self.items as *mut bridge::structs::collections::Il2cppArray<T>
    }

    fn items_array(&self) -> Option<&bridge::structs::collections::Il2cppArray<T>> {
        if self.items.is_null() {
            None
        } else {
            unsafe { Some(&*self.items_ptr()) }
        }
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.size.max(0) as usize
    }

    /// True when the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the element at `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        self.items_array()
            .and_then(|arr| (index < arr.len()).then(|| arr.get(index)))
    }

    /// Collect all elements into a Rust `Vec`.
    pub fn to_vec(&self) -> Vec<T> {
        (0..self.len()).filter_map(|i| self.get(i)).collect()
    }

    /// Iterate over elements (bounds-checked).
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let bound = self
            .items_array()
            .map(|arr| self.len().min(arr.len()))
            .unwrap_or(0);
        (0..bound).filter_map(|i| self.get(i))
    }
}
