//! IL2CPP ABI descriptions. Keep version-dependent storage here, not in builders.
//! Offsets are checked against exported metadata before enabling native registration.
use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Type {
    pub data: usize,
    pub bits: u32,
}
impl Type {
    pub fn reference(class: *mut Class, by_ref: bool) -> Self {
        Self {
            data: class as usize,
            bits: (0x12 << 16)
                | ((by_ref as u32)
                    << if cfg!(any(feature = "unity-2019-4", feature = "unity-2020-3")) {
                        30
                    } else {
                        29
                    }),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtualEntry {
    pub function: *mut c_void,
    pub method: *mut Method,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InterfaceOffset {
    pub class: *mut Class,
    pub offset: i32,
}
#[repr(C)]
pub struct Field {
    pub name: *const c_char,
    pub ty: *mut Type,
    pub parent: *mut Class,
    pub offset: i32,
    pub token: u32,
}
#[repr(C)]
#[cfg(any(feature = "unity-2019-4", feature = "unity-2020-3"))]
pub struct Parameter {
    pub name: *const c_char,
    pub position: i32,
    pub token: u32,
    pub ty: *mut Type,
}
#[repr(C)]
pub struct Method {
    pub function: *mut c_void,
    #[cfg(not(any(feature = "unity-2019-4", feature = "unity-2020-3")))]
    pub virtual_function: *mut c_void,
    pub invoker: *mut c_void,
    pub name: *const c_char,
    pub class: *mut Class,
    pub return_type: *mut Type,
    pub parameters: *mut c_void,
    pub context: *mut c_void,
    pub generic: *mut c_void,
    pub token: u32,
    pub flags: u16,
    pub implementation_flags: u16,
    pub slot: u16,
    pub parameter_count: u8,
    pub traits: u8,
}

#[repr(C)]
pub struct Class {
    pub image: *mut c_void,
    pub gc_descriptor: *mut c_void,
    pub name: *const c_char,
    pub namespace: *const c_char,
    pub value_type: Type,
    pub reference_type: Type,
    pub element: *mut Class,
    pub cast: *mut Class,
    pub declaring: *mut Class,
    pub parent: *mut Class,
    pub generic: *mut c_void,
    pub definition: *mut c_void,
    pub interop: *mut c_void,
    pub self_class: *mut Class,
    pub fields: *mut Field,
    pub events: *mut c_void,
    pub properties: *mut c_void,
    pub methods: *mut *mut Method,
    pub nested: *mut *mut Class,
    pub interfaces: *mut *mut Class,
    pub interface_offsets: *mut InterfaceOffset,
    pub static_fields: *mut c_void,
    pub generic_context: *mut c_void,
    pub hierarchy: *mut *mut Class,
    pub unity_data: *mut c_void,
    #[cfg(feature = "unity-2023-1")]
    pub initialization_exception: usize,
    #[cfg(not(feature = "unity-2023-1"))]
    pub initialization_exception: u32,
    pub constructor_started: u32,
    pub constructor_finished: u32,
    pub tail: ClassTail,
}

// C applies 8-byte alignment to cctor_thread, including on 32-bit Android.
// Putting the entire suffix here preserves the following field's offset.
#[repr(C, align(8))]
pub struct ClassTail {
    pub constructor_thread: usize,
    #[cfg(feature = "unity-2019-4")]
    pub generic_container: i32,
    #[cfg(not(feature = "unity-2019-4"))]
    pub generic_container: usize,
    pub instance_size: u32,
    #[cfg(any(feature = "unity-2022-3", feature = "unity-2023-1"))]
    pub stack_slot_size: u32,
    pub actual_size: u32,
    pub element_size: u32,
    pub native_size: i32,
    pub static_size: u32,
    pub thread_static_size: u32,
    pub thread_static_offset: i32,
    pub flags: u32,
    pub token: u32,
    pub method_count: u16,
    pub property_count: u16,
    pub field_count: u16,
    pub event_count: u16,
    pub nested_count: u16,
    pub vtable_count: u16,
    pub interface_count: u16,
    pub interface_offset_count: u16,
    pub hierarchy_depth: u8,
    pub recursion_depth: u8,
    pub rank: u8,
    pub alignment: u8,
    #[cfg(not(any(feature = "unity-2022-3", feature = "unity-2023-1")))]
    pub natural_alignment: u8,
    pub packing: u8,
    pub state: [u8; 2],
    pub vtable: [VirtualEntry; 0],
}
impl Class {
    pub const VTABLE_OFFSET: usize =
        std::mem::offset_of!(Self, tail) + std::mem::offset_of!(ClassTail, vtable);
    pub fn prepare_state(&mut self, finalizer: bool) {
        #[cfg(feature = "unity-2019-4")]
        {
            self.tail.state = [0x20 | 0x80, 0x10 | u8::from(finalizer)];
        }
        #[cfg(not(feature = "unity-2019-4"))]
        {
            self.tail.state = [0x20, 0x01 | 0x20 | (u8::from(finalizer) << 1)];
        }
    }
    pub unsafe fn vtable(&self) -> &[VirtualEntry] {
        unsafe {
            std::slice::from_raw_parts(self.tail.vtable.as_ptr(), self.tail.vtable_count as usize)
        }
    }
}

// ABI facts checked against the reference headers for each Android pointer width.
// These are compile-time assertions, so every cross-target build checks them too.
const _: () = {
    let wide = usize::BITS == 64;
    let old = cfg!(feature = "unity-2019-4");
    let recent = cfg!(any(feature = "unity-2022-3", feature = "unity-2023-1"));
    assert!(
        size_of::<Class>()
            == if wide {
                if old { 304 } else { 312 }
            } else {
                192
            }
    );
    assert!(
        Class::VTABLE_OFFSET
            == if wide {
                if old { 304 } else { 312 }
            } else if recent {
                192
            } else {
                188
            }
    );
    assert!(
        std::mem::offset_of!(Class, tail) + std::mem::offset_of!(ClassTail, instance_size)
            == if wide {
                if old { 244 } else { 248 }
            } else {
                128
            }
    );
    assert!(
        std::mem::offset_of!(Class, tail) + std::mem::offset_of!(ClassTail, method_count)
            == if wide {
                if old {
                    280
                } else if recent {
                    288
                } else {
                    284
                }
            } else if recent {
                168
            } else {
                164
            }
    );
    assert!(
        size_of::<Method>()
            == if cfg!(any(feature = "unity-2019-4", feature = "unity-2020-3")) {
                if wide { 80 } else { 44 }
            } else if wide {
                88
            } else {
                48
            }
    );
    assert!(size_of::<Type>() == if wide { 16 } else { 8 });
};
