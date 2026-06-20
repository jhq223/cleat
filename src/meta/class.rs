//! IL2CPP class metadata and reflection.

use std::fmt::{self, Debug};

use il2cpp_bridge_rs as bridge;

use crate::{Error, Il2CppObject, Il2CppValueType, MethodInfo, Result};

/// A handle to an IL2CPP class (type metadata, not an instance).
///
/// Cheap to clone — class metadata is read-only and doesn't move.
#[derive(Clone)]
pub struct Il2CppClass {
    pub(crate) inner: bridge::structs::Class,
}

impl Il2CppClass {
    // ── lookup ────────────────────────────────────────────────────────────

    /// Look up a class by its fully-qualified C# name.
    ///
    /// ```ignore
    /// let klass = Il2CppClass::find("UnityEngine.GameObject")?;
    /// ```
    pub fn find(name: &str) -> Result<Self> {
        let class = bridge::api::cache::csharp()
            .class(name)
            .ok_or_else(|| Error::ClassNotFound(name.to_string()))?;
        Ok(Self { inner: class })
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
    pub fn find_objects(&self, include_inactive: bool) -> Vec<Il2CppObject> {
        self.inner
            .find_objects_of_type(include_inactive)
            .iter()
            .map(|&object| Il2CppObject { inner: object })
            .collect::<Vec<_>>()
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
        name: &str,
        args: impl crate::Args,
    ) -> Result<T> {
        let method = self.get_method(name)?;
        method.invoke(args)
    }

    /// Call a static `void` method.
    pub fn invoke_static_void(&self, name: &str, args: impl crate::Args) -> Result<()> {
        let method = self.get_method(name)?;
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
    pub fn method(&self, name: &str) -> Result<MethodInfo> {
        self.get_method(name)
    }

    // ── internal ──────────────────────────────────────────────────────────

    /// Return the raw bridge method pointer.
    ///
    /// Used internally by the `#[cleat::hook]` macro. Prefer
    /// [`method`](Self::method) or [`invoke_static`](Self::invoke_static)
    /// for regular use.
    #[doc(hidden)]
    pub fn method_ptr(&self, name: &str) -> Result<bridge::structs::Method> {
        self.inner
            .method(name)
            .ok_or_else(|| Error::MethodNotFound(name.to_string()))
    }

    fn get_method(&self, name: &str) -> Result<MethodInfo> {
        let method = self
            .inner
            .method(name)
            .ok_or_else(|| Error::MethodNotFound(name.to_string()))?;
        Ok(MethodInfo { inner: method })
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
