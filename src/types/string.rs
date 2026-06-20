//! Managed `System.String` wrapper.

use std::ffi::c_void;
use std::fmt::{self, Debug};

use il2cpp_bridge_rs as bridge;
use il2cpp_bridge_rs::structs::Field;

use super::Il2CppValueType;
use crate::{Error, Result};

/// A managed `System.String` — cheap to copy (it's just a pointer).
///
/// # Conversions
///
/// * `From<&str>` / `Il2CppString::new(s)` — allocate a new managed string.
/// * `to_string_lossy()` — read the string back without panicking.
/// * `String::try_from(...)` — fallible conversion.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Il2CppString {
    pub(crate) ptr: *mut bridge::structs::Il2cppString,
}

unsafe impl Il2CppValueType for Il2CppString {
    unsafe fn load_field(field: &Field) -> Result<Self> {
        let ptr = unsafe {
            field
                .get_value::<*mut bridge::structs::Il2cppString>()
                .map_err(Error::Bridge)?
        };
        Ok(Il2CppString { ptr })
    }

    unsafe fn store_field(field: &Field, val: Self) -> Result<()> {
        unsafe {
            field
                .set_value::<*mut bridge::structs::Il2cppString>(val.ptr)
                .map_err(Error::Bridge)
        }
    }
}

impl Il2CppString {
    /// Allocate a new managed string from a Rust `&str`.
    pub fn new(s: &str) -> Self {
        let ptr = bridge::structs::Il2cppString::new(s);
        assert!(
            !ptr.is_null(),
            "Il2CppString::new returned null — is IL2CPP initialized?"
        );
        Self { ptr }
    }

    /// Returns the raw IL2CPP string pointer (for use with iCalls).
    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr as *mut c_void
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

    /// Length in UTF-16 code units (same as `string.Length` in C#).
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

    /// Wrap a raw `Il2CppString*` obtained from outside the typed field
    /// API — for example from a `Dictionary<K,V>` entry or a `List<T>`
    /// item slot.
    ///
    /// # Safety
    ///
    /// `ptr` must be null or a valid pointer to a managed `System.String`.
    pub unsafe fn from_raw_ptr(ptr: *mut c_void) -> Self {
        Self {
            ptr: ptr as *mut bridge::structs::Il2cppString,
        }
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

impl Default for Il2CppString {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
        }
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
