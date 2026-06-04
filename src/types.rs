use std::ffi::c_void;
use std::fmt::{self, Debug};
use std::marker::PhantomData;

use il2cpp_bridge_rs as bridge;

use crate::{Error, Il2CppClass, Il2CppValueType, Result};

// ── Il2CppList<T> ─────────────────────────────────────────────────────────

/// Direct memory mapping of .NET's `List<T>`.
///
/// This is a `#[repr(C)]` struct laid out to match the managed
/// `System.Collections.Generic.List<T>` fields exactly, so you can read it
/// straight out of IL2CPP memory with `ptr::read`.
///
/// You should not mutate the list through these fields — use C# method
/// calls (`Add`, `Remove`, etc.) instead.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Il2CppList<T: Copy> {
    /// IL2CPP class pointer for `List<T>`.
    pub klass: *mut c_void,
    /// Monitor object (used for `lock` statements in C#).
    pub monitor: *mut c_void,
    /// Pointer to the underlying `T[]` array.
    pub items: *mut c_void,
    /// Number of elements currently in the list.
    pub size: i32,
    /// Mod count — incremented on every mutation (used to detect concurrent
    /// modification during enumeration).
    pub version: i32,
    /// Marker so the compiler knows which `T` this list holds.
    pub _phantom: PhantomData<T>,
}

unsafe impl<T: Copy + 'static> Il2CppValueType for Il2CppList<T> {
    unsafe fn load_field(field: &il2cpp_bridge_rs::structs::Field) -> Result<Self> {
        let ptr = unsafe {
            field
                .get_value::<*mut c_void>()
                .map_err(Error::Bridge)?
        };
        Ok(unsafe { std::ptr::read(ptr as *const Self) })
    }

    // Writing list fields directly is deliberately blocked — mutations
    // should go through C# methods (Add, Remove, etc.).
    #[cold]
    unsafe fn store_field(_field: &il2cpp_bridge_rs::structs::Field, _val: Self) -> Result<()> {
        Err(Error::Bridge(
            "Il2CppList mutation should be done through C# methods (Add/Remove)".into(),
        ))
    }

    unsafe fn invoke_result(
        method: &il2cpp_bridge_rs::structs::Method,
        args: &[*mut std::ffi::c_void],
    ) -> Result<Self> {
        let ptr = unsafe {
            method
                .call::<*mut c_void>(args)
                .map_err(Error::Bridge)?
        };
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
    ///
    /// Copies element-by-element through `get()`. No raw pointer is held
    /// across a GC point, so this is safe even if the GC runs concurrently.
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

// ── Il2CppString ──────────────────────────────────────────────────────────

/// A managed `System.String` — cheap to copy (it's just a pointer).
///
/// # Conversions
///
/// * `From<&str>` / `Il2CppString::new(s)` — allocate a new managed string.
/// * `to_string_lossy()` — read the string back (returns `"<null>"` or
///   `"<invalid UTF-16>"` for edge cases instead of panicking).
/// * `String::try_from(...)` — fallible conversion that returns an error on
///   null or malformed UTF-16.
#[derive(Clone, Copy)]
pub struct Il2CppString {
    pub(crate) ptr: *mut bridge::structs::Il2cppString,
}

unsafe impl Il2CppValueType for Il2CppString {
    unsafe fn load_field(field: &il2cpp_bridge_rs::structs::Field) -> Result<Self> {
        let ptr = unsafe {
            field
                .get_value::<*mut bridge::structs::Il2cppString>()
                .map_err(Error::Bridge)?
        };
        Ok(Il2CppString { ptr })
    }

    unsafe fn store_field(field: &il2cpp_bridge_rs::structs::Field, val: Self) -> Result<()> {
        unsafe {
            field
                .set_value::<*mut bridge::structs::Il2cppString>(val.ptr)
                .map_err(Error::Bridge)
        }
    }
}

impl Il2CppString {
    /// Allocate a new managed string from a Rust `&str`.
    ///
    /// # Panics
    ///
    /// If the IL2CPP runtime hasn't been initialised yet, or if the GC is
    /// out of memory.
    pub fn new(s: &str) -> Self {
        let ptr = bridge::structs::Il2cppString::new(s);
        assert!(
            !ptr.is_null(),
            "Il2CppString::new returned null — is IL2CPP initialized?"
        );
        Self { ptr }
    }

    /// Read the string contents into a Rust `String`.
    ///
    /// Never panics. Returns `"<null>"` if the pointer is dead, or
    /// `"<invalid UTF-16>"` on encoding issues.
    pub fn to_string_lossy(&self) -> String {
        if self.ptr.is_null() {
            return "<null>".into();
        }
        unsafe { (*self.ptr).to_string() }.unwrap_or_else(|| "<invalid UTF-16>".into())
    }

    /// Length in UTF-16 code units (the same as `string.Length` in C#).
    /// Returns 0 for a null pointer.
    pub fn len(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        unsafe { (*self.ptr).length as usize }
    }

    /// True when the string is empty (or the pointer is null).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Display for Il2CppString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_lossy())
    }
}

impl Debug for Il2CppString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

impl From<&str> for Il2CppString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl TryFrom<&Il2CppString> for String {
    type Error = Error;

    fn try_from(s: &Il2CppString) -> Result<Self> {
        if s.ptr.is_null() {
            return Err(Error::Bridge("Il2CppString pointer is null".into()));
        }
        unsafe { (*s.ptr).to_string() }
            .ok_or_else(|| Error::Bridge("invalid UTF-16 in IL2CPP string".into()))
    }
}

// ── Il2CppArray<T> ────────────────────────────────────────────────────────

/// Wrapper around a managed `T[]` array.
pub struct Il2CppArray<T> {
    pub(crate) ptr: *mut bridge::structs::collections::Il2cppArray<T>,
}

impl<T: Copy + 'static> Il2CppArray<T> {
    /// Allocate a new managed array of the given element type and size.
    ///
    /// Returns an error if allocation fails (e.g. IL2CPP not initialised).
    pub fn new(element_class: &Il2CppClass, size: usize) -> Result<Self> {
        let ptr =
            bridge::structs::collections::Il2cppArray::<T>::new(&element_class.inner, size);
        if ptr.is_null() {
            return Err(Error::Bridge(
                "Il2cppArray::new returned null — is IL2CPP initialized?".into(),
            ));
        }
        Ok(Self { ptr })
    }

    /// Read element at `index`. No bounds check — out-of-bounds access is
    /// undefined behaviour.
    ///
    /// # Safety
    ///
    /// Caller must guarantee `index < self.len()`. Use [`at`](Self::at)
    /// for a bounds-checked version.
    pub fn get(&self, index: usize) -> T {
        unsafe { (*self.ptr).get(index) }
    }

    /// Read element at `index`, returning `None` on out-of-bounds.
    pub fn at(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        Some(self.get(index))
    }

    /// Write element at `index`. No bounds check — out-of-bounds access is
    /// undefined behaviour.
    ///
    /// # Safety
    ///
    /// Caller must guarantee `index < self.len()`.
    pub fn set(&mut self, index: usize, value: T) {
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
}
