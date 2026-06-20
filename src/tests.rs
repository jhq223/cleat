//! Compile-time and layout regression tests.
//!
//! These tests run on any platform (no IL2CPP runtime needed) and verify
//! that struct layouts, offsets, and type traits are correct.

#[cfg(test)]
mod tests {
    use crate::*;

    // ── MethodPtr ──

    #[test]
    fn method_ptr_is_copy_send_sync() {
        fn assert_copy<T: Copy>() {}
        fn assert_send_sync<T: Send + Sync>() {}
        assert_copy::<ffi::method_ptr::MethodPtr>();
        assert_send_sync::<ffi::method_ptr::MethodPtr>();
    }

    // ── Il2CppDictionary layout ──

    #[test]
    fn dict_struct_size() {
        // 6 pointers (48) + 3 i32 (12) = 60, padded to 64
        assert_eq!(
            std::mem::size_of::<Il2CppDictionary<*mut std::ffi::c_void, *mut std::ffi::c_void>>(),
            64
        );
    }

    #[test]
    fn dict_struct_offsets() {
        use std::mem::offset_of;
        type D = Il2CppDictionary<u8, u8>;
        assert_eq!(offset_of!(D, klass), 0);
        assert_eq!(offset_of!(D, monitor), 8);
        assert_eq!(offset_of!(D, buckets), 16);
        assert_eq!(offset_of!(D, entries), 24);
        assert_eq!(offset_of!(D, keys), 32);
        assert_eq!(offset_of!(D, values), 40);
        assert_eq!(offset_of!(D, touched_slots), 48);
        assert_eq!(offset_of!(D, empty_slot), 52);
        assert_eq!(offset_of!(D, count), 56);
    }

    // ── Dictionary entry size calculation ──

    #[test]
    fn entry_size_ptr_ptr() {
        // Dictionary<string, object>: 4+4+8+8 = 24
        assert_eq!(
            types::dictionary::entry_size::<*mut std::ffi::c_void, *mut std::ffi::c_void>(),
            24
        );
    }

    #[test]
    fn entry_size_i32_i32() {
        // Dictionary<int, int>: 4+4+4+4 = 16
        assert_eq!(types::dictionary::entry_size::<i32, i32>(), 16);
    }

    #[test]
    fn entry_size_i32_i64() {
        // Dictionary<int, long>: 4+4+4+pad4+8 = 24
        assert_eq!(types::dictionary::entry_size::<i32, i64>(), 24);
    }

    #[test]
    fn entry_size_i64_i64() {
        // Dictionary<long, long>: 4+4+8+8 = 24
        assert_eq!(types::dictionary::entry_size::<i64, i64>(), 24);
    }

    #[test]
    fn entry_value_offset_ptr_ptr() {
        // key at 8, value at 16
        assert_eq!(
            types::dictionary::value_offset::<*mut std::ffi::c_void, *mut std::ffi::c_void>(),
            16
        );
    }

    #[test]
    fn entry_value_offset_i32_i32() {
        // key at 8, value at 12
        assert_eq!(types::dictionary::value_offset::<i32, i32>(), 12);
    }

    // ── Il2CppList layout ──

    #[test]
    fn list_struct_size() {
        // 2 pointers (16) + 1 pointer (8) + 2 i32 (8) = 32
        assert_eq!(std::mem::size_of::<Il2CppList<i32>>(), 32);
    }

    // ── Error Display ──

    #[test]
    fn error_display() {
        assert_eq!(
            Error::ClassNotFound("Foo".into()).to_string(),
            "class not found: Foo"
        );
        assert_eq!(
            Error::MethodNotFound("Bar".into()).to_string(),
            "method not found: Bar"
        );
        assert_eq!(
            Error::FieldNotFound("baz".into()).to_string(),
            "field not found: baz"
        );
        assert_eq!(Error::Hook("crash".into()).to_string(), "hook error: crash");
        assert_eq!(
            Error::Bridge("oops".into()).to_string(),
            "bridge error: oops"
        );
    }

    // ── Il2CppValueType trait bounds ──

    #[test]
    fn primitive_value_type_bounds() {
        fn assert_valuetype<T: Il2CppValueType>() {}
        assert_valuetype::<i32>();
        assert_valuetype::<f32>();
        assert_valuetype::<bool>();
        assert_valuetype::<Il2CppObject>();
        assert_valuetype::<Il2CppString>();
    }

    // ── Args trait ──

    #[test]
    fn args_zero_len() {
        assert_eq!(().len(), 0);
        assert!(().is_empty());
    }

    #[test]
    fn args_tuple_len() {
        assert_eq!((1i32,).len(), 1);
        assert_eq!((1i32, 2f32,).len(), 2);
        assert_eq!((1i32, 2f32, 3i64,).len(), 3);
    }
}
