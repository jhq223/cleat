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

impl TryFrom<crate::Object> for Il2CppString {
    type Error = Error;
    fn try_from(object: crate::Object) -> Result<Self> {
        if object.is_null() {
            return Err(Error::NullObject);
        }
        let class = object.class()?;
        if class.namespace() != "System" || class.name() != "String" {
            return Err(Error::Bridge(format!(
                "expected System.String, found {}.{}",
                class.namespace(),
                class.name()
            )));
        }
        Ok(unsafe { Self::from_raw_ptr(object.raw_ptr()) })
    }
}

unsafe impl Il2CppValueType for Il2CppString {
    const IS_REFERENCE: bool = true;
    const MANAGED_NAME: Option<&'static str> = Some("System.String");
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

    unsafe fn to_arg_ptr(&self) -> *mut c_void {
        self.as_ptr()
    }
}

impl Il2CppString {
    /// Allocate a new managed string from a Rust `&str`.
    pub fn new(s: &str) -> Self {
        Self::try_new(s).expect("could not allocate managed string")
    }

    pub fn try_new(s: &str) -> Result<Self> {
        Self::from_utf16(&s.encode_utf16().collect::<Vec<_>>())
    }

    /// Allocate from UTF-16 code units, preserving NULs and unpaired surrogates.
    pub fn from_utf16(units: &[u16]) -> Result<Self> {
        if !bridge::api::is_loaded() {
            return Err(Error::Bridge("IL2CPP is not initialized".into()));
        }
        let len = i32::try_from(units.len())
            .map_err(|_| Error::Bridge("string exceeds Int32 length".into()))?;
        let ptr = unsafe { bridge::api::string_new_utf16(units.as_ptr(), len) };
        if ptr.is_null() {
            return Err(Error::NullObject);
        }
        Ok(Self { ptr: ptr.cast() })
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn as_object(&self) -> crate::Object {
        unsafe { crate::Object::from_raw(self.as_ptr()) }
    }

    /// Copy the original UTF-16 units, without decoding or assuming byte offsets.
    pub fn to_utf16(&self) -> Result<Vec<u16>> {
        if self.is_null() {
            return Err(Error::NullObject);
        }
        let string = unsafe { &*self.ptr };
        let length = usize::try_from(string.length)
            .map_err(|_| Error::Bridge("invalid managed string length".into()))?;
        Ok(unsafe { std::slice::from_raw_parts(string.chars.as_ptr(), length) }.to_vec())
    }

    /// Returns the raw IL2CPP string pointer (for use with iCalls).
    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr as *mut c_void
    }

    /// Read the string contents into a Rust `String`.
    ///
    /// Null returns "<null>"; malformed UTF-16 uses replacement characters.
    /// Like other managed handles, this must refer to a live object.
    pub fn to_string_lossy(&self) -> String {
        if self.ptr.is_null() {
            return "<null>".into();
        }
        let string = unsafe { &*self.ptr };
        if string.length < 0 {
            return "<invalid length>".into();
        }
        String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(string.chars.as_ptr(), string.length as usize)
        })
    }

    /// Length in UTF-16 code units (same as `string.Length` in C#).
    pub fn len(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        usize::try_from(unsafe { (*self.ptr).length }).unwrap_or(0)
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
