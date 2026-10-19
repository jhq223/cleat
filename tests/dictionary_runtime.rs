//! Small IL2CPP API double: both private dictionary layouts expose identical
//! managed methods. This exercises invocation, boxing, enumeration and out args.
use cleat::{
    bridge::{
        api,
        structs::{Arg, Class, Method, Type},
    },
    *,
};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void},
    ptr,
    sync::{Mutex, OnceLock},
};

const INT: usize = 1;
const INT_REF: usize = 9;
const BOOL: usize = 2;
const VOID: usize = 3;
const LEGACY: usize = 4;
const MODERN: usize = 5;
const ENUM: usize = 6;
const PAIR: usize = 7;
const EXCEPTION: usize = 8;
const GET_COUNT: usize = 100;
const GET: usize = 101;
const SET: usize = 102;
const REMOVE: usize = 103;
const CLEAR: usize = 104;
const GET_ENUM: usize = 105;
const MOVE_NEXT: usize = 106;
const CURRENT: usize = 107;
const KEY: usize = 108;
const VALUE: usize = 109;
const DISPOSE: usize = 110;

#[repr(C)]
struct Header {
    class: usize,
    monitor: usize,
}
#[repr(C)]
struct Boxed<T> {
    header: Header,
    value: T,
}
#[repr(C)]
struct Legacy {
    header: Header,
    table: usize,
    links: usize,
    keys: usize,
    values: usize,
    backing: Backing,
}
#[repr(C)]
struct Modern {
    header: Header,
    buckets: usize,
    entries: usize,
    count: i32,
    version: i32,
    free_count: i32,
    backing: Backing,
}
struct Backing {
    slots: Vec<Option<(i32, i32)>>,
    version: usize,
}
#[repr(C)]
struct Enumerator {
    dictionary: *mut c_void,
    version: usize,
    index: usize,
}
#[repr(C)]
struct Pair {
    key: i32,
    value: i32,
}

fn boxed<T>(class: usize, value: T) -> *mut c_void {
    Box::into_raw(Box::new(Boxed {
        header: Header { class, monitor: 0 },
        value,
    }))
    .cast()
}
unsafe fn backing<'a>(object: *mut c_void) -> &'a mut Backing {
    unsafe {
        match (*object.cast::<Header>()).class {
            LEGACY => &mut (*object.cast::<Legacy>()).backing,
            MODERN => &mut (*object.cast::<Modern>()).backing,
            _ => panic!("not a dictionary"),
        }
    }
}
unsafe extern "C" fn object_class(object: *mut c_void) -> *mut c_void {
    unsafe { (*object.cast::<Header>()).class as *mut c_void }
}
unsafe extern "C" fn identity(class: *mut c_void) -> *mut c_void {
    if class as usize == INT_REF {
        INT as *mut c_void
    } else {
        class
    }
}
unsafe extern "C" fn is_value(class: *mut c_void) -> bool {
    matches!(class as usize, INT | BOOL | VOID | ENUM | PAIR)
}
unsafe extern "C" fn value_size(class: *mut c_void, _align: *mut u32) -> i32 {
    match class as usize {
        INT => 4,
        BOOL => 1,
        VOID => 0,
        ENUM => size_of::<Enumerator>() as i32,
        PAIR => 8,
        _ => 0,
    }
}
unsafe extern "C" fn assignable(expected: *mut c_void, actual: *mut c_void) -> bool {
    expected == actual
}
unsafe extern "C" fn byref(ty: *mut c_void) -> bool {
    ty as usize == INT_REF
}
unsafe extern "C" fn unbox(object: *mut c_void) -> *mut c_void {
    unsafe { object.cast::<u8>().add(size_of::<Header>()).cast() }
}
static HANDLES: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
unsafe extern "C" fn root(object: *mut c_void, _pin: bool) -> api::GcHandleId {
    let mut handles = HANDLES.get_or_init(Default::default).lock().unwrap();
    let id = handles.keys().max().copied().unwrap_or(0) + 1;
    handles.insert(id, object as usize);
    id as api::GcHandleId
}
unsafe extern "C" fn target(id: api::GcHandleId) -> *mut c_void {
    HANDLES.get().unwrap().lock().unwrap()[&(id as usize)] as *mut c_void
}
unsafe extern "C" fn free(id: api::GcHandleId) {
    HANDLES
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .remove(&(id as usize));
}
unsafe extern "C" fn method_name(_method: *mut c_void) -> *const c_char {
    c"dictionary operation".as_ptr()
}
unsafe extern "C" fn no_method(
    _class: *mut c_void,
    _name: *const c_char,
    _count: i32,
) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn unavailable() {
    eprintln!(
        "unexpected API call: {}",
        std::backtrace::Backtrace::force_capture()
    );
    std::process::abort();
}

unsafe extern "C" fn invoke(
    method: *mut c_void,
    object: *mut c_void,
    args: *const *mut c_void,
    exception: *mut *mut c_void,
) -> *mut c_void {
    unsafe {
        *exception = ptr::null_mut();
        match method as usize {
            GET_COUNT => boxed(INT, backing(object).slots.iter().flatten().count() as i32),
            GET => {
                let key = *(*args).cast::<i32>();
                let pair = backing(object)
                    .slots
                    .iter()
                    .flatten()
                    .find(|(k, _)| *k == key)
                    .copied();
                if let Some((_, value)) = pair {
                    *(*args.add(1)).cast::<i32>() = value;
                }
                boxed(BOOL, pair.is_some())
            }
            SET => {
                let key = *(*args).cast::<i32>();
                let value = *(*args.add(1)).cast::<i32>();
                let data = backing(object);
                if let Some(pair) = data.slots.iter_mut().flatten().find(|(k, _)| *k == key) {
                    pair.1 = value;
                } else {
                    data.slots.push(Some((key, value)));
                }
                data.version += 1;
                ptr::null_mut()
            }
            REMOVE => {
                let key = *(*args).cast::<i32>();
                let data = backing(object);
                let position = data
                    .slots
                    .iter()
                    .position(|pair| pair.is_some_and(|(k, _)| k == key));
                if let Some(position) = position {
                    data.slots[position] = None;
                    data.version += 1;
                }
                boxed(BOOL, position.is_some())
            }
            CLEAR => {
                let data = backing(object);
                data.slots.clear();
                data.version += 1;
                ptr::null_mut()
            }
            GET_ENUM => boxed(
                ENUM,
                Enumerator {
                    dictionary: object,
                    version: backing(object).version,
                    index: 0,
                },
            ),
            MOVE_NEXT => {
                // The runtime must receive the unboxed value-type this.
                let enumerator = &mut *object.cast::<Enumerator>();
                let data = backing(enumerator.dictionary);
                if enumerator.version != data.version {
                    *exception = boxed(EXCEPTION, ());
                    return ptr::null_mut();
                }
                while enumerator.index < data.slots.len() {
                    let pair = data.slots[enumerator.index];
                    enumerator.index += 1;
                    if pair.is_some() {
                        return boxed(BOOL, true);
                    }
                }
                boxed(BOOL, false)
            }
            CURRENT => {
                let enumerator = &*object.cast::<Enumerator>();
                let (key, value) =
                    backing(enumerator.dictionary).slots[enumerator.index - 1].unwrap();
                boxed(PAIR, Pair { key, value })
            }
            KEY => boxed(INT, (*object.cast::<Pair>()).key),
            VALUE => boxed(INT, (*object.cast::<Pair>()).value),
            DISPOSE => ptr::null_mut(),
            _ => std::process::abort(),
        }
    }
}

fn ty(id: usize) -> Type {
    Type {
        address: id as *mut c_void,
        name: match id {
            INT => "System.Int32",
            INT_REF => "System.Int32&",
            BOOL => "System.Boolean",
            VOID => "System.Void",
            ENUM => "Enumerator",
            PAIR => "Pair",
            _ => "Dictionary",
        }
        .into(),
        size: 0,
    }
}
fn method(class: usize, id: usize, name: &str, ret: usize, args: &[usize]) -> Method {
    Method {
        address: id as *mut c_void,
        token: 0,
        name: name.into(),
        class: None,
        return_type: ty(ret),
        flags: 0,
        is_static: false,
        function: ptr::null_mut(),
        rva: 0,
        va: 0,
        args: args
            .iter()
            .map(|&id| Arg {
                name: String::new(),
                type_info: ty(id),
            })
            .collect(),
        is_generic: false,
        is_inflated: false,
        is_instance: true,
        param_count: args.len() as u8,
        declaring_type: class as *mut c_void,
        instance: None,
    }
}
fn class(id: usize, methods: Vec<Method>) -> Class {
    Class {
        address: id as *mut c_void,
        image: ptr::null_mut(),
        token: 0,
        name: ty(id).name,
        parent: None,
        namespace: String::new(),
        is_enum: false,
        is_generic: false,
        is_inflated: false,
        is_interface: false,
        is_abstract: false,
        is_blittable: true,
        is_valuetype: unsafe { is_value(id as *mut c_void) },
        flags: 0,
        rank: 0,
        instance_size: 0,
        array_element_size: 0,
        num_fields_count: 0,
        enum_basetype: ptr::null_mut(),
        static_field_data: ptr::null_mut(),
        assembly_name: String::new(),
        assembly: None,
        fields: vec![],
        methods,
        properties: vec![],
        interfaces: vec![],
        nested_types: vec![],
        element_class: ptr::null_mut(),
        declaring_type: ptr::null_mut(),
        ty: id as *mut c_void,
        object: ptr::null_mut(),
    }
}
fn setup() {
    api::load(|name| {
        (match name {
            "il2cpp_object_get_class" => object_class as *const (),
            "il2cpp_class_from_type" => identity as *const (),
            "il2cpp_class_is_valuetype" => is_value as *const (),
            "il2cpp_class_value_size" => value_size as *const (),
            "il2cpp_class_is_assignable_from" => assignable as *const (),
            "il2cpp_type_is_byref" => byref as *const (),
            "il2cpp_object_unbox" => unbox as *const (),
            "il2cpp_gchandle_new" => root as *const (),
            "il2cpp_gchandle_get_target" => target as *const (),
            "il2cpp_gchandle_free" => free as *const (),
            "il2cpp_runtime_invoke" => invoke as *const (),
            "il2cpp_method_get_name" => method_name as *const (),
            "il2cpp_class_get_method_from_name" => no_method as *const (),
            _ => unavailable as *const (),
        })
        .cast_mut()
        .cast()
    })
    .unwrap();
    for id in [INT, BOOL, VOID, LEGACY, MODERN, ENUM, PAIR, EXCEPTION] {
        let methods = match id {
            LEGACY | MODERN => vec![
                method(id, GET_COUNT, "get_Count", INT, &[]),
                method(id, GET, "TryGetValue", BOOL, &[INT, INT_REF]),
                method(id, SET, "set_Item", VOID, &[INT, INT]),
                method(id, REMOVE, "Remove", BOOL, &[INT]),
                method(id, CLEAR, "Clear", VOID, &[]),
                method(id, GET_ENUM, "GetEnumerator", ENUM, &[]),
            ],
            ENUM => vec![
                method(id, MOVE_NEXT, "MoveNext", BOOL, &[]),
                method(id, CURRENT, "get_Current", PAIR, &[]),
                method(id, DISPOSE, "Dispose", VOID, &[]),
            ],
            PAIR => vec![
                method(id, KEY, "get_Key", INT, &[]),
                method(id, VALUE, "get_Value", INT, &[]),
            ],
            _ => vec![],
        };
        api::cache::CACHE
            .classes
            .insert(id.to_string(), Box::new(class(id, methods)));
        api::cache::CACHE.classes_by_ptr.insert(id, id.to_string());
    }
}

#[test]
fn dictionaries_work_across_private_layouts_and_detect_mutation() {
    setup();
    let data = || Backing {
        slots: vec![Some((1, 10)), Some((2, 20)), Some((3, 30))],
        version: 0,
    };
    let mut old = Legacy {
        header: Header {
            class: LEGACY,
            monitor: 0,
        },
        table: 0,
        links: 0,
        keys: 0,
        values: 0,
        backing: data(),
    };
    let mut new = Modern {
        header: Header {
            class: MODERN,
            monitor: 0,
        },
        buckets: 0,
        entries: 0,
        count: 0,
        version: 0,
        free_count: 0,
        backing: data(),
    };
    for ptr in [
        (&mut old as *mut Legacy).cast(),
        (&mut new as *mut Modern).cast(),
    ] {
        let dictionary = unsafe { Il2CppDictionary::<i32, i32>::from_raw(ptr) };
        assert_eq!(dictionary.len().unwrap(), 3);
        assert_eq!(dictionary.get(2).unwrap(), Some(20));
        let mut output = 0i32;
        assert!(
            dictionary
                .as_object()
                .call::<bool>("TryGetValue", (2, cleat::out(&mut output)))
                .unwrap()
        );
        assert_eq!(output, 20);
        assert!(
            dictionary
                .as_object()
                .call::<bool>("TryGetValue", (2, 0i32))
                .is_err()
        );
        assert!(
            dictionary
                .as_object()
                .call::<bool>("TryGetValue", (cleat::by_ref(&mut output), 0))
                .is_err()
        );

        assert_eq!(dictionary.get(99).unwrap(), None);
        assert!(dictionary.remove(1).unwrap());
        assert_eq!(dictionary.to_vec().unwrap(), [(2, 20), (3, 30)]);
        dictionary.set(3, 300).unwrap();
        dictionary.set(4, 400).unwrap();
        assert_eq!(dictionary.to_vec().unwrap(), [(2, 20), (3, 300), (4, 400)]);
        let mut iter = dictionary.iter().unwrap();
        assert_eq!(iter.next().unwrap().unwrap(), (2, 20));
        dictionary.remove(3).unwrap();
        assert!(iter.next().unwrap().is_err());
        assert!(iter.next().is_none());
        drop(iter);
        dictionary.clear().unwrap();
        assert!(dictionary.is_empty().unwrap());
        assert!(dictionary.to_vec().unwrap().is_empty());
        // Mismatched V must be rejected before TryGetValue writes its out slot.
        let wrong = unsafe { Il2CppDictionary::<i32, u8>::from_raw(ptr) };
        assert!(wrong.get(2).is_err());
    }
    assert!(HANDLES.get().unwrap().lock().unwrap().is_empty());
}
