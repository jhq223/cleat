//! `System.Collections.Generic.Dictionary<K, V>` memory mapping.
//!
//! Direct `#[repr(C)]` mapping of .NET's `Dictionary<K, V>` internal layout
//! (64-bit IL2CPP).  Provides typed iteration over the internal `entries`
//! array so callers no longer need to hand-compute offsets.
//!
//! ## Layout
//!
//! ```text
//! offset  field
//! 0x00    klass
//! 0x08    monitor
//! 0x10    buckets   (*mut Il2CppArray<i32>)
//! 0x18    entries   (*mut Il2CppArray<RawEntry>)
//! 0x20    keys      (*mut Il2CppArray<K>)   — lazily populated, may be null
//! 0x28    values    (*mut Il2CppArray<V>)   — lazily populated, may be null
//! 0x30    touched_slots  (i32)
//! 0x34    empty_slot     (i32)
//! 0x38    count          (i32)
//! ```
//!
//! ## Entry layout
//!
//! ```text
//! offset  field
//! 0x00    hashCode  (i32)
//! 0x04    next      (i32)
//! 0x08    key       (K)
//! 0x08 + size_of::<K>()  value  (V)  — aligned to align_of::<V>()
//! ```
//!
//! Entry total size is computed from `K` and `V` sizes/alignments.

use std::ffi::c_void;
use std::marker::PhantomData;

use il2cpp_bridge_rs::structs::Field;

use super::Il2CppValueType;
use crate::{Error, Il2CppArray, Result};

// ── struct layout ────────────────────────────────────────────────────────

/// `#[repr(C)]` mapping of `System.Collections.Generic.Dictionary<K, V>`.
///
/// `K` and `V` must be `Copy` and match the IL2CPP element sizes
/// (reference types are pointer-sized = 8 bytes on 64-bit).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Il2CppDictionary<K: Copy + 'static, V: Copy + 'static> {
    pub klass: *mut c_void,
    pub monitor: *mut c_void,
    pub buckets: *mut c_void, // +0x10  Il2CppArray<i32>
    pub entries: *mut c_void, // +0x18  Il2CppArray<Entry>
    pub keys: *mut c_void,    // +0x20  Il2CppArray<K>
    pub values: *mut c_void,  // +0x28  Il2CppArray<V>
    pub touched_slots: i32,   // +0x30
    pub empty_slot: i32,      // +0x34
    pub count: i32,           // +0x38
    _phantom: PhantomData<(K, V)>,
}

// ── entry layout helpers ─────────────────────────────────────────────────

/// Byte offset of `key` within a Dictionary entry (after hashCode + next).
const ENTRY_HEADER_SIZE: usize = 8; // i32 hashCode + i32 next

/// Round `addr` up to the next multiple of `align`.
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Byte offset of `value` within an entry (after key, aligned to V).
pub(crate) const fn value_offset<K: Copy + 'static, V: Copy + 'static>() -> usize {
    let after_key = ENTRY_HEADER_SIZE + std::mem::size_of::<K>();
    align_up(after_key, std::mem::align_of::<V>())
}

/// Total entry size (after value, aligned to pointer width).
pub(crate) const fn entry_size<K: Copy + 'static, V: Copy + 'static>() -> usize {
    let after_val = value_offset::<K, V>() + std::mem::size_of::<V>();
    align_up(after_val, std::mem::size_of::<usize>())
}

// ── Il2CppValueType impl ─────────────────────────────────────────────────

unsafe impl<K: Copy + 'static, V: Copy + 'static> Il2CppValueType for Il2CppDictionary<K, V> {
    unsafe fn load_field(field: &Field) -> Result<Self> {
        let ptr = unsafe { field.get_value::<*mut c_void>().map_err(Error::Bridge)? };
        if ptr.is_null() {
            return Err(Error::Bridge("Dictionary pointer is null".into()));
        }
        Ok(unsafe { std::ptr::read(ptr as *const Self) })
    }

    #[cold]
    unsafe fn store_field(_field: &Field, _val: Self) -> Result<()> {
        Err(Error::Bridge(
            "Il2CppDictionary mutation should be done through C# methods".into(),
        ))
    }

    unsafe fn invoke_result(
        method: &il2cpp_bridge_rs::structs::Method,
        args: &[*mut c_void],
    ) -> Result<Self> {
        let ptr = unsafe { method.call::<*mut c_void>(args).map_err(Error::Bridge)? };
        if ptr.is_null() {
            return Err(Error::Bridge("Dictionary return is null".into()));
        }
        Ok(unsafe { std::ptr::read(ptr as *const Self) })
    }
}

// ── public API ───────────────────────────────────────────────────────────

impl<K: Copy + 'static, V: Copy + 'static> Il2CppDictionary<K, V> {
    /// Number of entries.
    pub fn len(&self) -> usize {
        self.count.max(0) as usize
    }

    /// True when the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over `(K, V)` pairs by walking the internal `entries` array.
    ///
    /// Skips entries whose `hashCode < 0` (unused slots use `hashCode = -1`).
    pub fn iter(&self) -> DictionaryIter<'_, K, V> {
        let entries_arr = if self.entries.is_null() {
            None
        } else {
            // The entries array is an Il2CppArray of raw bytes.
            // We cast through c_void and interpret the data manually.
            Some(unsafe { Il2CppArray::<u8>::from_raw(self.entries) })
        };

        DictionaryIter {
            arr: entries_arr,
            index: 0,
            count: self.len(),
            _phantom: PhantomData,
        }
    }

    /// Set the value at entry `index` (bounds-checked).
    ///
    /// # Safety
    ///
    /// `index` must be a valid entry index with `hashCode >= 0`.
    pub unsafe fn set_entry_value(&self, arr: &Il2CppArray<u8>, index: usize, val: V) {
        if index >= arr.len() {
            return;
        }
        let data = arr.data_ptr();
        let entry = unsafe { data.add(index * entry_size::<K, V>()) };
        let val_ptr = unsafe { entry.add(value_offset::<K, V>()) as *mut V };
        unsafe { std::ptr::write(val_ptr, val) };
    }

    /// Get a raw handle to the entries array for low-level access.
    pub fn entries_array(&self) -> Option<Il2CppArray<u8>> {
        if self.entries.is_null() {
            None
        } else {
            Some(unsafe { Il2CppArray::<u8>::from_raw(self.entries) })
        }
    }
}

// ── iterator ──────────────────────────────────────────────────────────────

/// Iterator over `Dictionary<K, V>` entries, yielding `(K, V)` pairs.
pub struct DictionaryIter<'a, K: Copy + 'static, V: Copy + 'static> {
    arr: Option<Il2CppArray<u8>>,
    index: usize,
    count: usize,
    _phantom: PhantomData<&'a Il2CppDictionary<K, V>>,
}

impl<K: Copy + 'static, V: Copy + 'static> Iterator for DictionaryIter<'_, K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let arr = self.arr.as_ref()?;
        let esize = entry_size::<K, V>();
        let voff = value_offset::<K, V>();

        while self.index < self.count.min(arr.len()) {
            let i = self.index;
            self.index += 1;

            let data = arr.data_ptr();
            let entry = unsafe { data.add(i * esize) };

            // Skip unused entries (hashCode < 0).
            let hash = unsafe { std::ptr::read(entry as *const i32) };
            if hash < 0 {
                continue;
            }

            let key = unsafe { std::ptr::read(entry.add(ENTRY_HEADER_SIZE) as *const K) };
            let val = unsafe { std::ptr::read(entry.add(voff) as *const V) };
            return Some((key, val));
        }
        None
    }
}

// ── tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── entry layout ──

    #[test]
    fn entry_size_ptr_ptr() {
        // Dictionary<string, object> on 64-bit
        // hashCode(4) + next(4) + key(8) + value(8) = 24
        type K = *mut c_void; // 8 bytes, align 8
        type V = *mut c_void;
        assert_eq!(entry_size::<K, V>(), 24);
        assert_eq!(value_offset::<K, V>(), 16);
    }

    #[test]
    fn entry_size_i32_i32() {
        // Dictionary<int, int>
        // hashCode(4) + next(4) + key(4) + value(4) = 16
        assert_eq!(entry_size::<i32, i32>(), 16);
        assert_eq!(value_offset::<i32, i32>(), 12);
    }

    #[test]
    fn entry_size_i32_i64() {
        // Dictionary<int, long>
        // hashCode(4) + next(4) + key(4) + pad(4) + value(8) = 24
        assert_eq!(entry_size::<i32, i64>(), 24);
        assert_eq!(value_offset::<i32, i64>(), 16); // after_key=12, align to 8 → 16
    }

    #[test]
    fn entry_size_i64_i64() {
        // Dictionary<long, long>
        // hashCode(4) + next(4) + key(8) + value(8) = 24
        assert_eq!(entry_size::<i64, i64>(), 24);
        assert_eq!(value_offset::<i64, i64>(), 16);
    }

    // ── struct size ──

    #[test]
    fn dict_struct_size() {
        // 6 pointers (48) + 3 i32 (12) + PhantomData (0) = 60
        // With alignment padding: 64 (align to 8 for pointer fields)
        // Actually: 6*8 + 3*4 = 48 + 12 = 60, padded to 64
        assert_eq!(
            std::mem::size_of::<Il2CppDictionary<*mut c_void, *mut c_void>>(),
            64
        );
    }

    #[test]
    fn dict_struct_offsets() {
        // Verify field offsets match the IL2CPP layout
        use std::mem::offset_of;
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, klass), 0);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, monitor), 8);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, buckets), 16);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, entries), 24);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, keys), 32);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, values), 40);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, touched_slots), 48);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, empty_slot), 52);
        assert_eq!(offset_of!(Il2CppDictionary<u8, u8>, count), 56);
    }
}
