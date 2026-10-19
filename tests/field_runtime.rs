//! Field ABI regression: reference setters receive an object, not &object.
use cleat::bridge::{
    api,
    structs::{Field, Type},
};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};

static RECEIVED: AtomicUsize = AtomicUsize::new(0);
unsafe extern "C" fn kind(ty: *mut c_void) -> i32 {
    ty as i32
}
unsafe extern "C" fn byref(_: *mut c_void) -> bool {
    false
}
unsafe extern "C" fn set(_: *mut c_void, field: *mut c_void, value: *mut c_void) {
    let value = if field as usize == 8 {
        unsafe { *value.cast::<i32>() as usize }
    } else {
        value as usize
    };
    RECEIVED.store(value, Ordering::Relaxed);
}
unsafe extern "C" fn set_static(field: *mut c_void, value: *mut c_void) {
    unsafe { set(std::ptr::null_mut(), field, value) }
}
unsafe extern "C" fn unavailable() {
    panic!("unexpected mock API call")
}

#[test]
fn stores_references_null_and_values_with_the_correct_indirection() {
    api::load(|name| {
        (match name {
            "il2cpp_type_get_type" => kind as *const (),
            "il2cpp_type_is_byref" => byref as *const (),
            "il2cpp_field_set_value" => set as *const (),
            "il2cpp_field_static_set_value" => set_static as *const (),
            _ => unavailable as *const (),
        })
        .cast_mut()
        .cast()
    })
    .unwrap();
    for is_static in [false, true] {
        let mut field = Field {
            address: 0x0e as *mut c_void,
            name: "text".into(),
            type_info: Type {
                address: 0x0e as *mut c_void,
                name: "System.String".into(),
                size: 0,
            },
            class: None,
            offset: 16,
            flags: 0,
            is_static,
            is_literal: false,
            is_readonly: false,
            is_not_serialized: false,
            is_special_name: false,
            is_pinvoke_impl: false,
            instance: Some(0x1000 as *mut c_void),
        };
        for pointer in [0usize, 0x12345678] {
            unsafe { field.set_value(pointer as *mut c_void) }.unwrap();
            assert_eq!(RECEIVED.load(Ordering::Relaxed), pointer);
        }
        field.address = 8 as *mut c_void;
        field.type_info.address = 8 as *mut c_void;
        unsafe { field.set_value(42i32) }.unwrap();
        assert_eq!(RECEIVED.load(Ordering::Relaxed), 42);
    }
}
