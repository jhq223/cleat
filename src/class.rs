use std::fmt::{self, Debug};

use il2cpp_bridge_rs as bridge;

use crate::{Args, Error, Il2CppObject, Il2CppValueType, Result};

/// A handle to an IL2CPP class (type metadata, not an instance).
///
/// Safe to share across threads — class metadata is read-only and doesn't
/// move around.
#[derive(Clone)]
pub struct Il2CppClass {
    pub(crate) inner: bridge::structs::Class,
}

impl Il2CppClass {
    /// Look up a class by its fully-qualified C# name, including namespace.
    ///
    /// Searches every loaded assembly. If you know the assembly, prefer
    /// [`find_with`](Self::find_with) — it's cheaper.
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
    /// Use this when you have namespaced types from non-CSharp assemblies
    /// (DLL mods, third-party plugins, etc.).
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

    /// Read a static field on this class.
    ///
    /// ```ignore
    /// let screen_width: i32 = Il2CppClass::find("UnityEngine.Screen")?
    ///     .static_field_value("width")?;
    /// ```
    pub fn static_field_value<T: Il2CppValueType>(&self, name: &str) -> Result<T> {
        let field = self
            .inner
            .field(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))?;

        unsafe { T::load_field(&field) }
    }

    /// Walk up the inheritance chain. Returns `None` when you hit
    /// `System.Object` (or a root type with no parent).
    pub fn parent(&self) -> Option<Self> {
        let ptr = unsafe { bridge::api::class_get_parent(self.inner.address) };
        if ptr.is_null() {
            return None;
        }
        let class = bridge::api::cache::class_from_ptr(ptr)?;
        Some(Self { inner: class })
    }

    /// Allocate an object without running its constructor.
    ///
    /// This is equivalent to C#'s `FormatterServices.GetUninitializedObject`.
    /// Fields will be zero / null — you'll usually follow up with field
    /// writes via [`Il2CppObject::store`].
    pub fn new_object(&self) -> Result<Il2CppObject> {
        self.inner
            .new_object()
            .map(|object| Il2CppObject { inner: object })
            .map_err(Error::Bridge)
    }

    /// Allocate an object and run the default constructor.
    ///
    /// Equivalent to `Activator.CreateInstance` in C#. Fails if there is no
    /// parameterless constructor.
    pub fn create_instance(&self) -> Result<Il2CppObject> {
        self.inner
            .create_instance()
            .map(|object| Il2CppObject { inner: object })
            .map_err(Error::Bridge)
    }

    /// Walk the scene and return every live object of this type.
    ///
    /// Pass `include_inactive = true` to include disabled GameObjects.
    /// Be careful — this can return thousands of entries in a large scene.
    pub fn find_objects(&self, include_inactive: bool) -> Vec<Il2CppObject> {
        self.inner
            .find_objects_of_type(include_inactive)
            .iter()
            .map(|&object| Il2CppObject { inner: object })
            .collect::<Vec<_>>()
    }

    /// Returns `true` when this class inherits from `other` (directly or
    /// transitively).
    pub fn is_subclass_of(&self, other: &Self) -> bool {
        unsafe { bridge::api::class_is_subclass_of(self.inner.address, other.inner.address, false) }
    }

    /// Force the static constructor to run.
    ///
    /// The CLR lazily initialises static fields the first time a class is
    /// touched. Call this early to trigger that initialisation on your
    /// schedule instead of waiting for the game to hit it.
    pub fn init(&self) {
        unsafe {
            bridge::api::runtime_class_init(self.inner.address);
        }
    }

    /// Call a static method that returns a value.
    ///
    /// `args` is a tuple — `()`, `(42,)`, `(1, 2.0)`, etc. — converted to
    /// FFI pointers through the [`Args`](crate::Args) trait.
    ///
    /// ```ignore
    /// let result: i32 = klass.invoke_static("Calculate", (10, 20))?;
    /// ```
    pub fn invoke_static<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T> {
        let method = self.method_ptr(name)?;
        let arg_ptrs = args.to_arg_ptrs();
        unsafe { T::invoke_result(&method, &arg_ptrs) }
    }

    /// Call a static `void` method.
    pub fn invoke_static_void(&self, name: &str, args: impl Args) -> Result<()> {
        let method = self.method_ptr(name)?;
        let arg_ptrs = args.to_arg_ptrs();
        unsafe { method.call::<()>(&arg_ptrs).map_err(Error::Bridge) }
    }

    /// Get the raw bridge method pointer.
    ///
    /// Mostly used internally by `#[cleat::hook]`. If you're calling
    /// methods manually, use [`invoke_static`](Self::invoke_static) or
    /// [`invoke_static_void`](Self::invoke_static_void) instead.
    pub fn method_ptr(&self, name: &str) -> Result<bridge::structs::Method> {
        self.inner
            .method(name)
            .ok_or_else(|| Error::MethodNotFound(name.to_string()))
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
