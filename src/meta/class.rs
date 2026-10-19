//! IL2CPP class metadata and reflection.

use std::fmt::{self, Debug};

use il2cpp_bridge_rs as bridge;

use crate::{Error, Il2CppObject, Il2CppValueType, MethodInfo, MethodSelector, Result};

/// A handle to an IL2CPP class (type metadata, not an instance).
///
/// Cheap to clone — class metadata is read-only and doesn't move.
#[derive(Clone)]
pub struct Il2CppClass {
    pub(crate) inner: bridge::structs::Class,
}

impl Il2CppClass {
    /// Box a Rust value matching this managed value type.
    pub fn box_value<T: Il2CppValueType>(&self, value: T) -> Result<Il2CppObject> {
        if T::IS_REFERENCE || !self.is_valuetype() {
            return Err(Error::Bridge("boxing requires a value type".into()));
        }
        T::validate_type(&bridge::structs::Type {
            address: self.inner.ty,
            name: format!("{}.{}", self.namespace(), self.name()),
            size: 0,
        })?;
        let ptr = unsafe {
            bridge::api::value_box(
                self.inner.address,
                &value as *const T as *mut std::ffi::c_void,
            )
        };
        if ptr.is_null() {
            return Err(Error::NullObject);
        }
        Ok(unsafe { Il2CppObject::from_raw(ptr) })
    }
    /// Read a static field or property (fields take precedence).
    pub fn get<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        if self.inner.field(name).is_some() {
            self.field(name)?.get()
        } else {
            self.property(name)?.get()
        }
    }
    /// Write a static field or property. Accepts Rust strings directly.
    pub fn set(&self, name: &str, value: impl crate::IntoIl2Cpp) -> Result<()> {
        let value = value.into_il2cpp()?;
        if self.inner.field(name).is_some() {
            self.field(name)?.set(value)
        } else {
            self.property(name)?.set(value)
        }
    }
    pub fn call<T: Il2CppValueType>(
        &self,
        selector: impl MethodSelector,
        args: impl crate::Args,
    ) -> Result<T> {
        self.invoke_static(selector, args)
    }
    /// The managed System.Type, without allocating a dummy instance.
    pub fn type_object(&self) -> Result<Il2CppObject> {
        if self.inner.object.is_null() {
            return Err(Error::NullObject);
        }
        Ok(unsafe { Il2CppObject::from_raw(self.inner.object) })
    }
    // ── lookup ────────────────────────────────────────────────────────────

    /// Look up a class by its fully-qualified C# name.
    ///
    /// ```ignore
    /// let klass = Il2CppClass::find("UnityEngine.GameObject")?;
    /// ```
    pub fn find(name: &str) -> Result<Self> {
        let assemblies = bridge::api::cache::CACHE
            .assemblies
            .iter()
            .map(|entry| entry.value().clone())
            .collect::<Vec<_>>();
        let mut matches = assemblies
            .iter()
            .filter_map(|assembly| assembly.class(name))
            .collect::<Vec<_>>();
        matches.sort_by_key(|class| class.address as usize);
        matches.dedup_by_key(|class| class.address as usize);
        match matches.len() {
            0 => Err(Error::ClassNotFound(name.into())),
            1 => Ok(Self {
                inner: matches.remove(0),
            }),
            _ => Err(Error::Ambiguous(format!(
                "class {name}; use Class::find_in(assembly, name)"
            ))),
        }
    }

    /// Find a class in an assembly, with or without the `.dll` suffix.
    pub fn find_in(assembly: &str, name: &str) -> Result<Self> {
        Self::find_with(name, assembly)
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }
    pub fn namespace(&self) -> &str {
        &self.inner.namespace
    }
    pub fn assembly_name(&self) -> &str {
        &self.inner.assembly_name
    }

    /// Methods declared on this class; instance methods can be bound with bind.
    pub fn methods(&self) -> Vec<MethodInfo> {
        self.inner
            .methods
            .iter()
            .cloned()
            .map(|inner| MethodInfo { inner })
            .collect()
    }
    /// Fields declared on this class. Bind instance fields before reading them.
    pub fn fields(&self) -> Vec<crate::FieldHandle> {
        self.inner
            .fields
            .iter()
            .cloned()
            .map(|field| crate::FieldHandle { field })
            .collect()
    }

    /// Allocate and invoke the constructor selected by argument count.
    /// Use `construct_with` to disambiguate constructors with equal arity.
    pub fn construct(&self, args: impl crate::Args) -> Result<Il2CppObject> {
        self.construct_with((".ctor", args.len()), args)
    }

    pub fn construct_with(
        &self,
        selector: impl MethodSelector,
        args: impl crate::Args,
    ) -> Result<Il2CppObject> {
        let method = self.method(selector)?;
        if method.name() != ".ctor" || method.is_static() {
            return Err(Error::Bridge(
                "constructor selector must resolve an instance .ctor".into(),
            ));
        }
        let object = self.new_object()?;
        method.bind(&object)?.invoke_void(args)?;
        Ok(object)
    }

    /// Cached static field handle.
    pub fn field(&self, name: &str) -> Result<crate::FieldHandle> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.into()))?;
        Ok(crate::FieldHandle { field })
    }

    pub fn property(&self, name: &str) -> Result<crate::Property> {
        crate::Property::find(self, name, None)
    }

    pub fn event(&self, name: &str) -> Result<crate::Event> {
        crate::Event::find(self, name, None)
    }

    pub fn nested(&self, name: &str) -> Result<Self> {
        self.nested_types()
            .into_iter()
            .find(|class| class.name() == name)
            .ok_or_else(|| Error::ClassNotFound(name.into()))
    }

    pub fn nested_types(&self) -> Vec<Self> {
        self.inner
            .nested_types
            .iter()
            .filter_map(|&ptr| bridge::api::cache::class_from_ptr(ptr))
            .map(|inner| Self { inner })
            .collect()
    }

    pub fn interfaces(&self) -> Vec<Self> {
        self.inner
            .interfaces
            .iter()
            .filter_map(|&ptr| bridge::api::cache::class_from_ptr(ptr))
            .map(|inner| Self { inner })
            .collect()
    }

    pub fn is_assignable_from(&self, other: &Self) -> bool {
        self.inner.is_assignable_from(&other.inner)
    }

    /// Look up a class scoped to a specific assembly.
    ///
    /// ```ignore
    /// let klass = Il2CppClass::find_with("MyPlugin.SpecialType", "MyPlugin")?;
    /// ```
    pub fn find_with(name: &str, assembly_name: &str) -> Result<Self> {
        let assembly = bridge::api::cache::assembly(assembly_name)
            .ok_or_else(|| Error::AssemblyNotFound(assembly_name.to_string()))?;
        let class = assembly
            .class(name)
            .ok_or_else(|| Error::ClassNotFound(name.to_string()))?;
        Ok(Self { inner: class })
    }

    // ── static fields ─────────────────────────────────────────────────────

    /// Read a static field on this class.
    ///
    /// ```ignore
    /// let width: i32 = Il2CppClass::find("UnityEngine.Screen")?
    ///     .static_field_value("width")?;
    /// ```
    pub fn static_field_value<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        T::validate_type(&field.type_info)?;
        unsafe { T::load_field(&field) }
    }

    /// Write a static field on this class.
    ///
    /// ```ignore
    /// Il2CppClass::find("UnityEngine.Time")?
    ///     .set_static_field_value("timeScale", 2.0f32)?;
    /// ```
    pub fn set_static_field_value<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;
        val.validate_value(&field.type_info)?;
        unsafe { T::store_field(&field, val) }
    }

    // ── hierarchy ─────────────────────────────────────────────────────────

    /// Walk up the inheritance chain. Returns `None` at `System.Object`.
    pub fn parent(&self) -> Option<Self> {
        let ptr = unsafe { bridge::api::class_get_parent(self.inner.address) };
        if ptr.is_null() {
            return None;
        }
        let class = bridge::api::cache::class_from_ptr(ptr)?;
        Some(Self { inner: class })
    }

    /// Returns `true` when this class inherits from `other`.
    ///
    /// Set `check_interfaces` to also consider implemented interfaces.
    pub fn is_subclass_of(&self, other: &Self, check_interfaces: bool) -> bool {
        unsafe {
            bridge::api::class_is_subclass_of(
                self.inner.address,
                other.inner.address,
                check_interfaces,
            )
        }
    }

    // ── instance creation ─────────────────────────────────────────────────

    /// Allocate an object without running its constructor.
    ///
    /// Equivalent to C#'s `FormatterServices.GetUninitializedObject`.
    pub fn new_object(&self) -> Result<Il2CppObject> {
        self.inner
            .new_object()
            .map(|object| Il2CppObject { inner: object })
            .map_err(Error::Bridge)
    }

    /// Allocate an object and run the default constructor.
    ///
    /// Equivalent to `Activator.CreateInstance` in C#.
    pub fn create_instance(&self) -> Result<Il2CppObject> {
        self.inner
            .create_instance()
            .map(|object| Il2CppObject { inner: object })
            .map_err(Error::Bridge)
    }

    // ── scene traversal ───────────────────────────────────────────────────

    /// Walk the scene and return every live object of this type.
    ///
    /// Pass `include_inactive = true` to include disabled GameObjects.
    pub fn find_objects(&self, include_inactive: bool) -> Result<Vec<Il2CppObject>> {
        let unity = Self::find("UnityEngine.Object")?;
        let array: crate::Il2CppArray<Il2CppObject> =
            match unity.method(("FindObjectsOfType", ["System.Type", "System.Boolean"])) {
                Ok(method) => method.invoke((self.type_object()?, include_inactive))?,
                Err(Error::MethodNotFound(_)) if !include_inactive => unity.call(
                    ("FindObjectsOfType", ["System.Type"]),
                    (self.type_object()?,),
                )?,
                Err(error) => return Err(error),
            };
        let _root = crate::GcHandle::pinned(unsafe { Il2CppObject::from_raw(array.as_ptr()) })?;
        Ok(array.to_vec())
    }
    /// Find one live scene object. Call from Unity's main thread.
    pub fn find_object(&self, include_inactive: bool) -> Result<Option<Il2CppObject>> {
        Ok(self.find_objects(include_inactive)?.into_iter().next())
    }

    /// Specialize a generic type (alias of inflate).
    pub fn generic(&self, type_args: &[&Self]) -> Result<Self> {
        self.inflate(type_args)
    }

    /// Create a ScriptableObject through Unity's factory, on the main thread.
    pub fn create_scriptable_instance(&self) -> Result<Il2CppObject> {
        Self::find("UnityEngine.ScriptableObject")?
            .call(("CreateInstance", ["System.Type"]), (self.type_object()?,))
    }

    // ── static methods ────────────────────────────────────────────────────

    /// Call a static method.
    ///
    /// `args` is a tuple — `()`, `(42,)`, `(1, 2.0)`, etc.
    ///
    /// ```ignore
    /// let result: i32 = klass.invoke_static("Calculate", (10, 20))?;
    /// ```
    pub fn invoke_static<T: Il2CppValueType>(
        &self,
        name: impl MethodSelector,
        args: impl crate::Args,
    ) -> Result<T> {
        let method = self.select_method(name, Some(args.len()))?;
        method.invoke(args)
    }

    /// Call a static `void` method.
    pub fn invoke_static_void(
        &self,
        name: impl MethodSelector,
        args: impl crate::Args,
    ) -> Result<()> {
        let method = self.select_method(name, Some(args.len()))?;
        method.invoke_void(args)
    }

    // ── init ──────────────────────────────────────────────────────────────

    /// Force the static constructor to run.
    ///
    /// Call this early to trigger class initialisation on your schedule.
    pub fn init(&self) {
        unsafe {
            bridge::api::runtime_class_init(self.inner.address);
        }
    }

    // ── generics ─────────────────────────────────────────────────────────

    /// Create a concrete generic type from a generic type definition.
    ///
    /// ```ignore
    /// let list_t = Il2CppClass::find("System.Collections.Generic.List`1")?;
    /// let int32 = Il2CppClass::find("System.Int32")?;
    /// let list_int = list_t.inflate(&[&int32])?;
    /// ```
    pub fn inflate(&self, type_args: &[&Il2CppClass]) -> Result<Self> {
        let classes: Vec<&bridge::structs::Class> = type_args.iter().map(|c| &c.inner).collect();
        let inflated = self.inner.inflate(&classes).map_err(Error::Bridge)?;
        // Bridge returns Arc<Class>, unwrap to owned.
        let inner = bridge::structs::Class::clone(&inflated);
        Ok(Self { inner })
    }

    // ── type predicates ──────────────────────────────────────────────────

    /// True if this is a value type (`struct` in C#).
    pub fn is_valuetype(&self) -> bool {
        self.inner.is_valuetype
    }

    /// True if this is an enum.
    pub fn is_enum(&self) -> bool {
        self.inner.is_enum
    }

    /// True if this is a generic type definition (e.g. `List<T>`).
    pub fn is_generic(&self) -> bool {
        self.inner.is_generic
    }

    /// True if this is an interface.
    pub fn is_interface(&self) -> bool {
        self.inner.is_interface
    }

    /// True if this is an abstract class.
    pub fn is_abstract(&self) -> bool {
        self.inner.is_abstract
    }

    // ── method handle ─────────────────────────────────────────────────────

    /// Get a [`MethodInfo`] handle for a static method.
    ///
    /// Prefer [`invoke_static`](Self::invoke_static) for simple calls.
    /// Use this when you need to specialise a generic method before
    /// calling it.
    pub fn method(&self, selector: impl MethodSelector) -> Result<MethodInfo> {
        self.select_method(selector, None)
    }

    // ── internal ──────────────────────────────────────────────────────────

    /// Return the raw bridge method pointer.
    ///
    /// Used internally by the `#[cleat::hook]` macro. Prefer
    /// [`method`](Self::method) or [`invoke_static`](Self::invoke_static)
    /// for regular use.
    #[doc(hidden)]
    pub fn method_ptr(&self, name: impl MethodSelector) -> Result<bridge::structs::Method> {
        Ok(self.method(name)?.inner)
    }

    pub(crate) fn select_method(
        &self,
        selector: impl MethodSelector,
        arity: Option<usize>,
    ) -> Result<MethodInfo> {
        let mut class = self.clone();
        loop {
            let mut matches = class.inner.methods.iter().filter(|method| {
                selector.matches(method) && arity.is_none_or(|len| method.args.len() == len)
            });
            if let Some(method) = matches.next() {
                if matches.next().is_some() {
                    return Err(Error::Ambiguous(format!(
                        "{}.{}; specify parameter types",
                        class.name(),
                        method.name
                    )));
                }
                return Ok(MethodInfo {
                    inner: method.clone(),
                });
            }
            match class.parent() {
                Some(parent) => class = parent,
                None => break,
            }
        }
        Err(Error::MethodNotFound(format!(
            "matching method on {}",
            self.name()
        )))
    }
}

// ── traits ────────────────────────────────────────────────────────────────

impl Debug for Il2CppClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.inner.namespace, self.inner.name)
    }
}

impl PartialEq for Il2CppClass {
    fn eq(&self, other: &Self) -> bool {
        self.inner.address == other.inner.address
    }
}

impl Eq for Il2CppClass {}

// Class metadata is immutable — safe to share.
unsafe impl Send for Il2CppClass {}
unsafe impl Sync for Il2CppClass {}
