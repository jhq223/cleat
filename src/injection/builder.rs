use super::{InjectionRuntime, layout, runtime};
use crate::{Class, Error, Il2CppValueType, Result, bridge};
use bumpalo::Bump;
use std::{
    alloc::Layout,
    ffi::{CString, c_char, c_void},
    mem::{align_of, size_of},
    ptr,
};

/// A Rust callback exposed as a managed method. Obtain using #[cleat::managed_method].
pub struct ManagedMethod {
    name: String,
    result: Class,
    parameters: Vec<Class>,
    is_static: bool,
    native: *mut c_void,
    invoker: *mut c_void,
}
impl ManagedMethod {
    /// # Safety
    /// Pointers must implement the declared method ABI and the old/new IL2CPP invoker ABIs.
    #[doc(hidden)]
    pub unsafe fn from_parts(
        name: &str,
        result: Class,
        parameters: Vec<Class>,
        is_static: bool,
        native: *mut c_void,
        old_invoker: *mut c_void,
        new_invoker: *mut c_void,
    ) -> Self {
        let invoker = if cfg!(any(feature = "unity-2019-4", feature = "unity-2020-3")) {
            old_invoker
        } else {
            new_invoker
        };
        Self {
            name: name.into(),
            result,
            parameters,
            is_static,
            native,
            invoker,
        }
    }
}
struct Field {
    name: String,
    class: Class,
    size: usize,
    alignment: usize,
}

/// Define a new class in an existing assembly. Registered metadata lives until process exit.
pub struct ClassBuilder {
    assembly: String,
    name: String,
    parent: Class,
    interfaces: Vec<Class>,
    fields: Vec<Field>,
    methods: Vec<ManagedMethod>,
}
impl ClassBuilder {
    pub fn new(assembly: impl Into<String>, name: impl Into<String>, parent: &Class) -> Self {
        Self {
            assembly: assembly.into(),
            name: name.into(),
            parent: parent.clone(),
            interfaces: vec![],
            fields: vec![],
            methods: vec![],
        }
    }
    pub fn implements(mut self, interface: &Class) -> Self {
        self.interfaces.push(interface.clone());
        self
    }
    /// Add a field whose runtime type is inferred from T's managed name.
    pub fn field<T: Il2CppValueType>(mut self, name: &str) -> Result<Self> {
        let class = super::type_of::<T>()?;
        T::validate_type(&bridge::structs::Type {
            address: class.inner.ty,
            name: format!("{}.{}", class.namespace(), class.name()),
            size: 0,
        })?;
        self.fields.push(Field {
            name: name.into(),
            class,
            size: size_of::<T>(),
            alignment: align_of::<T>(),
        });
        Ok(self)
    }
    pub fn method(mut self, method: ManagedMethod) -> Self {
        self.methods.push(method);
        self
    }
    /// Register on the thread that initialized the runtime. Existing classes are rejected.
    pub fn register(self, _runtime: &InjectionRuntime) -> Result<Class> {
        let _lock = runtime::REGISTER.lock().unwrap_or_else(|e| e.into_inner());
        self.validate()?;
        unsafe { self.build() }
    }
    fn validate(&self) -> Result<()> {
        if self.name.is_empty()
            || self.name.ends_with('.')
            || self.name.contains('`')
            || self.name.contains('/')
        {
            return Err(Error::Bridge(
                "injected classes need a non-generic, non-nested C# name".into(),
            ));
        }
        if self.parent.is_valuetype()
            || self.parent.is_interface()
            || self.parent.is_generic()
            || self.parent.inner.flags & 0x100 != 0
        {
            return Err(Error::Bridge(
                "parent must be a non-sealed reference class".into(),
            ));
        }
        match Class::find_in(&self.assembly, &self.name) {
            Ok(_) => return Err(Error::Bridge("class already exists".into())),
            Err(Error::ClassNotFound(_)) => {}
            Err(error) => return Err(error),
        }
        let mut fields = std::collections::HashSet::new();
        for field in &self.fields {
            if field.name.is_empty() || field.size == 0 || !fields.insert(&field.name) {
                return Err(Error::Bridge("invalid or duplicate field".into()));
            }
            CString::new(field.name.as_str())
                .map_err(|_| Error::Bridge("field name contains NUL".into()))?;
        }
        let mut methods = std::collections::HashSet::new();
        for method in &self.methods {
            let signature = (
                &method.name,
                method
                    .parameters
                    .iter()
                    .map(|ty| ty.inner.address as usize)
                    .collect::<Vec<_>>(),
            );
            if method.name.is_empty()
                || method.parameters.len() > u8::MAX as usize
                || !methods.insert(signature)
            {
                return Err(Error::Bridge(
                    "invalid or duplicate method signature".into(),
                ));
            }
            CString::new(method.name.as_str())
                .map_err(|_| Error::Bridge("method name contains NUL".into()))?;
        }
        for interface in &self.interfaces {
            if !interface.is_interface() {
                return Err(Error::Bridge("implements requires an interface".into()));
            }
        }
        if self.fields.len() > u16::MAX as usize || self.methods.len() > u16::MAX as usize {
            return Err(Error::Bridge("too many members".into()));
        }
        Ok(())
    }
    unsafe fn build(self) -> Result<Class> {
        self.parent.init();
        unsafe {
            runtime::check_layout(&self.parent)?;
        }
        let parent_ptr = self.parent.inner.address.cast::<layout::Class>();
        let parent = unsafe { &*parent_ptr };
        let assembly = bridge::api::cache::assembly(&self.assembly)
            .ok_or_else(|| Error::AssemblyNotFound(self.assembly.clone()))?;
        let (namespace, name) = self.name.rsplit_once('.').unwrap_or(("", &self.name));
        let arena = Bump::new();
        let mut interfaces = Vec::new();
        fn collect(class: &Class, into: &mut Vec<Class>, parent: &Class) {
            if into.contains(class) || class.is_assignable_from(parent) {
                return;
            }
            for inherited in class.interfaces() {
                collect(&inherited, into, parent);
            }
            into.push(class.clone());
        }
        for interface in &self.interfaces {
            collect(interface, &mut interfaces, &self.parent);
        }
        let mut vtable = unsafe { parent.vtable() }.to_vec();
        let mut offsets = if parent.tail.interface_offset_count == 0 {
            vec![]
        } else {
            unsafe {
                std::slice::from_raw_parts(
                    parent.interface_offsets,
                    parent.tail.interface_offset_count as usize,
                )
            }
            .to_vec()
        };
        for interface in &interfaces {
            interface.init();
            offsets.push(layout::InterfaceOffset {
                class: interface.inner.address.cast(),
                offset: vtable.len() as i32,
            });
            for method in interface.methods() {
                vtable.push(layout::VirtualEntry {
                    function: ptr::null_mut(),
                    method: method.inner.address.cast(),
                });
            }
        }
        if vtable.len() > u16::MAX as usize
            || offsets.len() > u16::MAX as usize
            || parent.tail.hierarchy_depth == u8::MAX
        {
            return Err(Error::Bridge(
                "class hierarchy or virtual table is too large".into(),
            ));
        }
        let total = size_of::<layout::Class>()
            .max(layout::Class::VTABLE_OFFSET + vtable.len() * size_of::<layout::VirtualEntry>());
        let allocation = arena.alloc_layout(
            Layout::from_size_align(total, align_of::<layout::Class>())
                .map_err(|e| Error::Bridge(e.to_string()))?,
        );
        unsafe { ptr::write_bytes(allocation.as_ptr(), 0, total) };
        let class_ptr = allocation.as_ptr().cast::<layout::Class>();
        let class = unsafe { &mut *class_ptr };
        class.image = assembly.image.address;
        class.name = name_in(&arena, name)?;
        class.namespace = name_in(&arena, namespace)?;
        class.value_type = layout::Type::reference(class_ptr, false);
        class.reference_type = layout::Type::reference(class_ptr, true);
        class.parent = parent_ptr;
        class.element = class_ptr;
        class.cast = class_ptr;
        class.self_class = class_ptr;
        class.constructor_finished = 1;
        #[cfg(feature = "unity-2019-4")]
        {
            class.tail.generic_container = -1;
        }
        #[cfg(any(feature = "unity-2022-3", feature = "unity-2023-1"))]
        {
            class.tail.stack_slot_size = size_of::<usize>() as u32;
        }
        class.tail.native_size = -1;
        class.tail.flags = (parent.tail.flags & !(0x80 | 0x100 | 0x20 | 0x7)) | 1;
        let mut instance_size = parent.tail.instance_size as usize;
        let mut alignment = (parent.tail.alignment as usize).max(align_of::<usize>());
        let mut native_fields = Vec::new();
        for field in &self.fields {
            alignment = alignment.max(field.alignment);
            instance_size = instance_size.next_multiple_of(field.alignment);
            let mut ty = unsafe { *field.class.inner.ty.cast::<layout::Type>() };
            ty.bits = (ty.bits & !0xffff) | 6; // public instance field
            native_fields.push(layout::Field {
                name: name_in(&arena, &field.name)?,
                ty: arena.alloc(ty),
                parent: class_ptr,
                offset: i32::try_from(instance_size)
                    .map_err(|_| Error::Bridge("class is too large".into()))?,
                token: 0,
            });
            instance_size = instance_size
                .checked_add(field.size)
                .ok_or_else(|| Error::Bridge("class is too large".into()))?;
        }
        instance_size = instance_size.next_multiple_of(alignment);
        class.tail.instance_size =
            u32::try_from(instance_size).map_err(|_| Error::Bridge("class is too large".into()))?;
        class.tail.actual_size = class.tail.instance_size;
        class.tail.alignment = alignment as u8;
        #[cfg(not(any(feature = "unity-2022-3", feature = "unity-2023-1")))]
        {
            class.tail.natural_alignment = alignment as u8;
        }
        class.tail.field_count = native_fields.len() as u16;
        class.fields = arena.alloc_slice_fill_iter(native_fields).as_mut_ptr();
        let mut native_methods = Vec::new();
        let mut finalizer =
            parent.tail.state[1] & if cfg!(feature = "unity-2019-4") { 1 } else { 2 } != 0;
        for method in &self.methods {
            let native = arena.alloc(unsafe { std::mem::zeroed::<layout::Method>() });
            native.function = method.native;
            native.invoker = method.invoker;
            #[cfg(not(any(feature = "unity-2019-4", feature = "unity-2020-3")))]
            {
                native.virtual_function = method.native;
            }
            native.name = name_in(&arena, &method.name)?;
            native.class = class_ptr;
            native.return_type = method.result.inner.ty.cast();
            native.parameter_count = method.parameters.len() as u8;
            native.flags = 6 | 0x80 | if method.is_static { 0x10 } else { 0 };
            if method.name == ".ctor" {
                native.flags |= 0x1800;
            }
            native.slot = u16::MAX;
            #[cfg(any(feature = "unity-2019-4", feature = "unity-2020-3"))]
            {
                native.parameters = arena
                    .alloc_slice_fill_iter(method.parameters.iter().enumerate().map(
                        |(index, ty)| layout::Parameter {
                            name: c"argument".as_ptr(),
                            position: index as i32,
                            token: 0,
                            ty: ty.inner.ty.cast(),
                        },
                    ))
                    .as_mut_ptr()
                    .cast();
            }
            #[cfg(not(any(feature = "unity-2019-4", feature = "unity-2020-3")))]
            {
                native.parameters = arena
                    .alloc_slice_fill_iter(
                        method
                            .parameters
                            .iter()
                            .map(|ty| ty.inner.ty.cast::<layout::Type>()),
                    )
                    .as_mut_ptr()
                    .cast();
            }
            if !method.is_static {
                for (index, slot) in vtable.iter_mut().enumerate() {
                    let original = unsafe { &*slot.method };
                    if unsafe { std::ffi::CStr::from_ptr(original.name) }.to_bytes()
                        != method.name.as_bytes()
                        || original.parameter_count as usize != method.parameters.len()
                    {
                        continue;
                    }
                    let matches = method.parameters.iter().enumerate().all(|(i, ty)| unsafe {
                        bridge::api::class_from_type(bridge::api::method_get_param(
                            slot.method.cast(),
                            i as u32,
                        )) == ty.inner.address
                    });
                    if !matches {
                        continue;
                    }
                    if unsafe { bridge::api::class_from_type(original.return_type.cast()) }
                        != method.result.inner.address
                    {
                        return Err(Error::Bridge(format!(
                            "{} override has incompatible return type",
                            method.name
                        )));
                    }
                    native.flags |= 0x40;
                    native.slot = index as u16;
                    slot.function = native.function;
                    slot.method = native;
                    if method.name == "Finalize" {
                        finalizer = true;
                    }
                }
            }
            native_methods.push(native as *mut layout::Method);
        }
        if vtable.iter().any(|entry| entry.function.is_null()) {
            return Err(Error::Bridge(
                "all abstract/interface methods need implementations".into(),
            ));
        }
        class.methods = arena.alloc_slice_copy(&native_methods).as_mut_ptr();
        class.tail.method_count = native_methods.len() as u16;
        class.interfaces = arena
            .alloc_slice_fill_iter(
                interfaces
                    .iter()
                    .map(|ty| ty.inner.address.cast::<layout::Class>()),
            )
            .as_mut_ptr();
        class.tail.interface_count = interfaces.len() as u16;
        class.interface_offsets = arena.alloc_slice_copy(&offsets).as_mut_ptr();
        class.tail.interface_offset_count = offsets.len() as u16;
        let mut hierarchy = if parent.tail.hierarchy_depth == 0 {
            vec![]
        } else {
            unsafe {
                std::slice::from_raw_parts(parent.hierarchy, parent.tail.hierarchy_depth as usize)
            }
            .to_vec()
        };
        hierarchy.push(class_ptr);
        class.tail.hierarchy_depth = hierarchy.len() as u8;
        class.hierarchy = arena.alloc_slice_copy(&hierarchy).as_mut_ptr();
        class.tail.vtable_count = vtable.len() as u16;
        unsafe {
            ptr::copy_nonoverlapping(
                vtable.as_ptr(),
                class.tail.vtable.as_mut_ptr(),
                vtable.len(),
            )
        };
        class.prepare_state(finalizer);
        runtime::publish(class_ptr, class.image as usize, namespace, name);
        // Native code may now retain these addresses, even if initialization fails.
        // IL2CPP metadata is process-lifetime data and must never be unloaded.
        Box::leak(Box::new(arena));
        let initialized = unsafe { runtime::initialize_class(class_ptr) }.and_then(|()| {
            bridge::api::cache::class_from_ptr(class_ptr.cast())
                .map(|inner| Class { inner })
                .ok_or_else(|| Error::Bridge("could not cache registered class".into()))
        });
        if initialized.is_err() {
            runtime::hide_failed_class(class.image as usize, namespace, name);
        }
        initialized
    }
}
fn name_in(arena: &Bump, name: &str) -> Result<*const c_char> {
    let name = CString::new(name).map_err(|_| Error::Bridge("name contains NUL".into()))?;
    Ok(arena
        .alloc_slice_copy(name.as_bytes_with_nul())
        .as_ptr()
        .cast())
}
