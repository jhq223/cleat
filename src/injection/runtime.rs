use super::{
    decoder::{self, Architecture},
    layout,
};
use crate::{Class, Error, HookSlot, MethodPtr, Result, bridge::api};
use std::{
    collections::HashMap,
    ffi::{CStr, c_char, c_void},
    marker::PhantomData,
    rc::Rc,
    sync::{Mutex, OnceLock, RwLock},
};

static START: Mutex<()> = Mutex::new(());
static READY: OnceLock<InternalSymbols> = OnceLock::new();
static TYPE_HOOK: HookSlot = HookSlot::new();
static ELEMENT_HOOK: HookSlot = HookSlot::new();
static NAME_HOOK: HookSlot = HookSlot::new();
pub(super) static REGISTER: Mutex<()> = Mutex::new(());
#[derive(Default)]
struct Registry {
    types: HashMap<usize, usize>,
    names: HashMap<(usize, String, String), usize>,
}
static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
fn registry() -> &'static RwLock<Registry> {
    REGISTRY.get_or_init(Default::default)
}

/// Overrides for builds with a different native wrapper/call pattern.
/// Addresses refer to IL2CPP's internal functions, not its exported wrappers.
#[derive(Clone, Copy)]
pub struct InternalSymbols {
    pub class_init: usize,
    pub class_from_type: usize,
    pub class_or_element: usize,
    pub class_from_name: usize,
}

/// Version-specific class registration on Unity's main thread.
/// The native hooks and registered metadata remain installed for process life.
pub struct InjectionRuntime {
    _thread: PhantomData<Rc<()>>,
}
impl InjectionRuntime {
    /// Check the Unity version/layout, resolve internal calls, and enable registration.
    ///
    /// # Safety
    /// Call on Unity's main thread after initialization. The game must use an
    /// unmodified IL2CPP ABI matching the selected version feature. Keep the mod loaded.
    pub unsafe fn initialize() -> Result<Self> {
        unsafe { Self::initialize_with(None) }
    }

    /// Use known internal addresses if the game's compiled wrappers differ.
    ///
    /// # Safety
    /// In addition to initialize's requirements, every supplied address must
    /// implement the corresponding IL2CPP internal function's ABI.
    pub unsafe fn initialize_with(symbols: Option<InternalSymbols>) -> Result<Self> {
        if !cfg!(target_os = "android") {
            return Err(Error::UnsupportedPlatform(
                "class injection requires Android",
            ));
        }
        let _lock = START.lock().unwrap_or_else(|e| e.into_inner());
        if READY.get().is_some() {
            return Ok(Self {
                _thread: PhantomData,
            });
        }
        check_version()?;
        let api =
            api::functions().ok_or_else(|| Error::Bridge("IL2CPP is not initialized".into()))?;
        for name in ["System.Object", "System.String", "System.Type"] {
            unsafe {
                check_layout(&Class::find(name)?)?;
            }
        }
        let symbols = match symbols {
            Some(symbols) => symbols,
            None => InternalSymbols {
                class_init: follow(follow(api.array_new_specific as usize)?)?,
                class_from_type: follow(api.class_from_type as usize)?,
                class_or_element: follow(api.type_get_class_or_element_class as usize)?,
                class_from_name: follow(follow(api.class_from_name as usize)?)?,
            },
        };
        if [
            symbols.class_init,
            symbols.class_from_type,
            symbols.class_or_element,
            symbols.class_from_name,
        ]
        .contains(&0)
        {
            return Err(Error::Bridge(
                "internal IL2CPP addresses must be non-null".into(),
            ));
        }
        let install = |slot: &HookSlot, function: usize, replacement: *const c_void| unsafe {
            slot.install(
                MethodPtr {
                    function: function as *mut c_void,
                    method_info: std::ptr::null_mut(),
                },
                replacement,
            )
        };
        let result = (|| {
            install(
                &TYPE_HOOK,
                symbols.class_from_type,
                from_type as *const c_void,
            )?;
            install(
                &ELEMENT_HOOK,
                symbols.class_or_element,
                element as *const c_void,
            )?;
            install(
                &NAME_HOOK,
                symbols.class_from_name,
                from_name as *const c_void,
            )
        })();
        if let Err(error) = result {
            NAME_HOOK.uninstall()?;
            ELEMENT_HOOK.uninstall()?;
            TYPE_HOOK.uninstall()?;
            return Err(error);
        }
        let _ = READY.set(symbols);
        Ok(Self {
            _thread: PhantomData,
        })
    }
}

fn check_version() -> Result<()> {
    let version: crate::Il2CppString =
        Class::find("UnityEngine.Application")?.get("unityVersion")?;
    let version = version.to_string();
    let expected = if cfg!(feature = "unity-2019-4") {
        "2019.4."
    } else if cfg!(feature = "unity-2020-3") {
        "2020.3."
    } else if cfg!(feature = "unity-2021-3") {
        "2021.3."
    } else if cfg!(feature = "unity-2022-3") {
        "2022.3."
    } else {
        "2023.1."
    };
    let old_patch = cfg!(feature = "unity-2020-3")
        && version
            .split('.')
            .nth(2)
            .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|s| s.parse::<u32>().ok())
            .is_none_or(|patch| patch < 20);
    if !version.starts_with(expected) || old_patch {
        return Err(Error::Bridge(format!(
            "Unity {version} does not match the selected {expected} feature (2020.3 requires patch 20+)"
        )));
    }
    Ok(())
}

pub(super) unsafe fn check_layout(class: &Class) -> Result<()> {
    let native = unsafe { &*class.inner.address.cast::<layout::Class>() };
    if native.name != unsafe { api::class_get_name(class.inner.address) }
        || std::ptr::addr_of!(native.value_type).cast::<c_void>() != class.inner.ty
        || native.tail.instance_size as i32 != class.inner.instance_size
        || native.tail.method_count as usize != class.inner.methods.len()
    {
        return Err(Error::Bridge(format!(
            "{} has an incompatible IL2CPP class layout",
            class.name()
        )));
    }
    for method in &class.inner.methods {
        let ptr = unsafe { &*method.address.cast::<layout::Method>() };
        if ptr.function != method.function
            || ptr.name != unsafe { api::method_get_name(method.address) }
            || ptr.parameter_count as usize != method.args.len()
        {
            return Err(Error::Bridge("incompatible IL2CPP method layout".into()));
        }
    }
    Ok(())
}

pub(super) unsafe fn initialize_class(class: *mut layout::Class) -> Result<()> {
    let address = READY
        .get()
        .ok_or_else(|| Error::Bridge("initialize InjectionRuntime first".into()))?
        .class_init;
    let call: unsafe extern "C" fn(*mut layout::Class) = unsafe { std::mem::transmute(address) };
    unsafe { call(class) };
    let initialized_bit = if cfg!(any(feature = "unity-2019-4", feature = "unity-2020-3")) {
        4
    } else {
        2
    };
    if unsafe { (*class).tail.state[0] } & initialized_bit == 0 {
        return Err(Error::Bridge(
            "IL2CPP did not initialize the registered class".into(),
        ));
    }
    Ok(())
}

pub(super) fn publish(class: *mut layout::Class, image: usize, namespace: &str, name: &str) {
    let mut registry = registry().write().unwrap_or_else(|e| e.into_inner());
    registry.types.insert(class as usize, class as usize);
    registry
        .names
        .insert((image, namespace.into(), name.into()), class as usize);
}
pub(super) fn hide_failed_class(image: usize, namespace: &str, name: &str) {
    // Keep type-pointer resolution alive for any native references acquired
    // during initialization, but don't expose a failed registration by name.
    registry()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .names
        .remove(&(image, namespace.into(), name.into()));
}
unsafe fn lookup(ty: *const layout::Type) -> Option<*mut layout::Class> {
    let ty = unsafe { ty.as_ref()? };
    if (ty.bits >> 16) & 0xff != 0x12 {
        return None;
    }
    registry()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .types
        .get(&ty.data)
        .map(|&ptr| ptr as *mut layout::Class)
}
unsafe extern "C" fn from_type(
    ty: *const layout::Type,
    throw_on_error: bool,
) -> *mut layout::Class {
    std::panic::catch_unwind(|| {
        let _active = TYPE_HOOK.enter(std::ptr::null_mut()).ok()?;
        if let Some(class) = unsafe { lookup(ty) } {
            return Some(class);
        }
        TYPE_HOOK
            .with_original(|original| {
                let call: unsafe extern "C" fn(*const layout::Type, bool) -> *mut layout::Class =
                    unsafe { std::mem::transmute(original.function) };
                unsafe { call(ty, throw_on_error) }
            })
            .ok()
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}
unsafe extern "C" fn element(ty: *const layout::Type) -> *mut layout::Class {
    std::panic::catch_unwind(|| {
        let _active = ELEMENT_HOOK.enter(std::ptr::null_mut()).ok()?;
        if let Some(class) = unsafe { lookup(ty) } {
            return Some(class);
        }
        ELEMENT_HOOK
            .with_original(|original| {
                let call: unsafe extern "C" fn(*const layout::Type) -> *mut layout::Class =
                    unsafe { std::mem::transmute(original.function) };
                unsafe { call(ty) }
            })
            .ok()
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}
unsafe extern "C" fn from_name(
    image: *mut c_void,
    namespace: *const c_char,
    name: *const c_char,
) -> *mut layout::Class {
    std::panic::catch_unwind(|| {
        let _active = NAME_HOOK.enter(std::ptr::null_mut()).ok()?;
        if !namespace.is_null() && !name.is_null() {
            let key = (
                image as usize,
                unsafe { CStr::from_ptr(namespace) }
                    .to_string_lossy()
                    .into_owned(),
                unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned(),
            );
            if let Some(&class) = registry()
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .names
                .get(&key)
            {
                return Some(class as *mut layout::Class);
            }
        }
        NAME_HOOK
            .with_original(|original| {
                let call: unsafe extern "C" fn(
                    *mut c_void,
                    *const c_char,
                    *const c_char,
                ) -> *mut layout::Class = unsafe { std::mem::transmute(original.function) };
                unsafe { call(image, namespace, name) }
            })
            .ok()
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

fn follow(address: usize) -> Result<usize> {
    let bytes = read_code(
        address
            & if cfg!(target_arch = "arm") {
                !1
            } else {
                usize::MAX
            },
    )?;
    let arch = if cfg!(target_arch = "aarch64") {
        Architecture::Arm64
    } else if cfg!(target_arch = "arm") {
        if address & 1 != 0 {
            Architecture::Thumb
        } else {
            Architecture::Arm
        }
    } else {
        Architecture::X86(usize::BITS)
    };
    let mut targets = decoder::branches(&bytes, address, arch)?;
    // 32-bit PIC wrappers may first call __x86.get_pc_thunk.*.
    if cfg!(target_arch = "x86") && targets.len() > 1 {
        let first = read_code(targets[0])?;
        if first.first() == Some(&0x8b) && first.get(3) == Some(&0xc3) {
            targets.remove(0);
        }
    }
    targets.first().copied().ok_or_else(|| {
        Error::Bridge(format!(
            "unsupported IL2CPP call pattern at {address:#x}; supply InternalSymbols"
        ))
    })
}
#[cfg(target_os = "android")]
fn read_code(address: usize) -> Result<Vec<u8>> {
    crate::hook::initialize();
    unsafe extern "C" {
        fn gum_memory_read(address: *const c_void, length: usize, actual: *mut usize) -> *mut u8;
        #[link_name = "_frida_g_free"]
        fn g_free(address: *mut c_void);
    }
    let mut length = 0;
    let data = unsafe { gum_memory_read(address as *const c_void, 512, &mut length) };
    if data.is_null() {
        return Err(Error::Bridge("cannot read IL2CPP code".into()));
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
    unsafe { g_free(data.cast()) };
    Ok(bytes)
}
#[cfg(not(target_os = "android"))]
fn read_code(_: usize) -> Result<Vec<u8>> {
    Err(Error::UnsupportedPlatform(
        "internal code resolution requires Android",
    ))
}
