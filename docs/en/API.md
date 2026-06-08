# cleat API Reference

---

## Entry Point

### `#[cleat::main]`

Marks the mod entry function. Generates a `JNI_OnLoad` symbol that Android
calls via `System.loadLibrary`.

```rust
#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    let klass = Il2CppClass::find("UnityEngine.Time")?;
    let ts: f32 = klass.static_field_value("timeScale")?;
    log::info!("timeScale = {ts}");
    Ok(())
}
```

Generated code:
1. Calls `android_logger::init_once(...)` — log output to logcat
2. Calls `il2cpp_bridge_rs::init("libil2cpp.so", ...)`
3. Runs your function when IL2CPP metadata is ready

---

## Initialization

### `cleat::init(target_image: &str, on_ready: impl FnOnce() + Send + 'static)`

Initializes android_logger and il2cpp_bridge_rs. Called automatically by
`#[cleat::main]`.

---

## Class Metadata — `Il2CppClass`

`#[derive(Clone)]`, `Send + Sync`.

### Lookup

#### `Il2CppClass::find(name: &str) -> Result<Self>`

Finds a class by fully-qualified name including namespace.

```rust
let klass = Il2CppClass::find("UnityEngine.GameObject")?;
let player = Il2CppClass::find("Player")?;
```

#### `Il2CppClass::find_with(name: &str, assembly_name: &str) -> Result<Self>`

Finds a class within a specific assembly.

```rust
let klass = Il2CppClass::find_with("MyPlugin.MyType", "MyPlugin")?;
```

### Static Fields

#### `class.static_field_value<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

Reads a static field.

```rust
let width: i32 = Il2CppClass::find("UnityEngine.Screen")?
    .static_field_value("width")?;
```

#### `class.set_static_field_value<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()>`

Writes a static field.

```rust
Il2CppClass::find("UnityEngine.Time")?
    .set_static_field_value("timeScale", 2.0f32)?;
```

### Inheritance

#### `class.parent(&self) -> Option<Self>`

Returns the parent class, or `None` if there is none.

#### `class.is_subclass_of(&self, other: &Self, check_interfaces: bool) -> bool`

Returns `true` if `self` is a subclass of `other`. Set `check_interfaces`
to also consider implemented interfaces.

### Instance Creation

#### `class.new_object(&self) -> Result<Il2CppObject>`

Creates an instance without calling the constructor (like
`FormatterServices.GetUninitializedObject`).

#### `class.create_instance(&self) -> Result<Il2CppObject>`

Creates an instance via the default constructor (like
`Activator.CreateInstance`).

### Scene Traversal

#### `class.find_objects(&self, include_inactive: bool) -> Vec<Il2CppObject>`

Returns all objects of this type in the current scene.

### Static Method Calls

#### `class.invoke_static<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

Calls a static method that returns a value. Use `()` for no arguments.

```rust
let result: i32 = klass.invoke_static("Calculate", (10, 20))?;
let time: f32 = klass.invoke_static("GetTime", ())?;
```

#### `class.invoke_static_void(&self, name: &str, args: impl Args) -> Result<()>`

Calls a static void method.

```rust
klass.invoke_static_void("Init", ())?;
klass.invoke_static_void("SetValues", (1, 2.0f32))?;
```

### Method Handle

#### `class.method(&self, name: &str) -> Result<MethodInfo>`

Gets a [`MethodInfo`](#method-handle--methodinfo) handle for a static
method, useful for generic method specialization.

### Generics

#### `class.inflate(&self, type_args: &[&Il2CppClass]) -> Result<Self>`

Creates a concrete generic type from a generic type definition.

```rust
let list_t = Il2CppClass::find("System.Collections.Generic.List`1")?;
let int32 = Il2CppClass::find("System.Int32")?;
let list_int = list_t.inflate(&[&int32])?;
```

### Type Predicates

```rust
class.is_valuetype()    // → bool  value type (struct)
class.is_enum()         // → bool  enum
class.is_generic()      // → bool  generic type definition
class.is_interface()    // → bool  interface
class.is_abstract()     // → bool  abstract class
```

### Init

#### `class.init(&self)`

Runs the static class constructor (equivalent to
`RuntimeHelpers.RunClassConstructor`).

---

## Managed Object — `Il2CppObject`

`#[derive(Clone, Copy)]`, `Send + Sync`.

### Field Access

#### `obj.load<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

Reads an instance field.

```rust
let hp: i32 = obj.load("currentHealth")?;
let name: Il2CppString = obj.load("playerName")?;
let pos: Vector3 = obj.load("position")?;
```

#### `obj.store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()>`

Writes an instance field.

```rust
obj.store("currentHealth", 999)?;
obj.store("playerName", Il2CppString::new("Hacker"))?;
```

#### `obj.field(&self, name: &str) -> Result<FieldHandle>`

Returns a cached field handle. The handle remembers the field offset so
subsequent `get`/`set` calls skip the name lookup.

```rust
let hp_field = obj.field("currentHealth")?;
let hp: i32 = hp_field.get()?;
hp_field.set(999)?;
```

### Method Calls

#### `obj.invoke<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

Calls an instance method that returns a value. Use `()` for no arguments.

```rust
let hp: i32 = obj.invoke("get_HP", ())?;
let result: i32 = obj.invoke("Calculate", (10, 20))?;
```

#### `obj.invoke_void(&self, name: &str, args: impl Args) -> Result<()>`

Calls a void instance method.

```rust
obj.invoke_void("Update", ())?;
obj.invoke_void("SetHP", (999,))?;
```

#### `obj.method(&self, name: &str) -> Result<MethodInfo>`

Gets a method handle for generic method specialization.

```rust
let data: Il2CppObject = obj
    .method("GetMasterData")?
    .inflate(&[&some_class])?
    .invoke(())?;
```

### Runtime Type

#### `obj.class(&self) -> Result<Il2CppClass>`

Returns the object's runtime type.

```rust
let klass = obj.class()?;
if klass.is_subclass_of(&player_class, false) {
    // ...
}
```

### Low-level Field Access (by Offset)

These escape hatches let you read and write fields whose type isn't
registered in cleat — private fields, fields inside managed structs like
`Dictionary<K,V>`, or any data at a known byte offset.

All methods in this section are `unsafe` — the caller must guarantee the
offset is within the object's allocated memory.

#### `unsafe obj.read_ptr_at(offset: isize) -> Option<*mut c_void>`

Reads a pointer-sized value at a byte offset. Returns `None` when the
object or the stored pointer field is null.

```rust
let dict_ptr = obj.raw_ptr();
let entries = unsafe { obj.read_ptr_at(0x18)? }; // Dictionary.entries
```

#### `unsafe obj.write_ptr_at(offset: isize, val: *mut c_void)`

Writes a pointer-sized value at a byte offset.

```rust
unsafe { obj.write_ptr_at(0x18, new_array.as_ptr()) };
```

#### `unsafe obj.read_i32_at(offset: isize) -> i32`

Reads a 32-bit signed integer at a byte offset.

```rust
let count = unsafe { dict.read_i32_at(0x20) }; // Dictionary.count
```

#### `unsafe obj.read_usize_at(offset: isize) -> usize`

Reads a pointer-sized unsigned integer at a byte offset.

```rust
let len = unsafe { arr.read_usize_at(0x18) }; // Il2CppArray.max_length
```

### Internal

#### `obj.raw_ptr(&self) -> *mut c_void`

Returns the raw IL2CPP object pointer. Useful together with
[`from_raw`](#unsafe-il2cppobjectfrom_rawptr-mut-c_void---self) when
you need to mix typed and offset-based access.

#### `unsafe Il2CppObject::from_raw(ptr: *mut c_void) -> Self`

Constructs from a raw IL2CPP object pointer. The pointer must be a
valid GC-tracked object or null.

```rust
let obj = unsafe { Il2CppObject::from_raw(some_ptr) };
let field = unsafe { obj.read_ptr_at(0x10) };
```

---

## Field Handle — `FieldHandle`

Obtained via [`Il2CppObject::field`](#objfieldself-name-str---resultfieldhandle).
Caches the field offset so repeated reads and writes skip the name lookup.

```rust
let hp_field = obj.field("currentHealth")?;

// Read
let hp: i32 = hp_field.get()?;

// Write
hp_field.set(999)?;
```

---

## Method Handle — `MethodInfo`

`#[derive(Clone)]`, `Send + Sync`.

### `method.inflate(&self, type_args: &[&Il2CppClass]) -> Result<Self>`

Injects generic type arguments. Instance binding is automatically inherited.

```rust
let inflated = method.inflate(&[&int_class, &string_class])?;
```

### `method.invoke<T: Il2CppValueType>(&self, args: impl Args) -> Result<T>`

Calls the method (with return value).

### `method.invoke_void(&self, args: impl Args) -> Result<()>`

Calls the method (void return).

---

## Managed Types

### `Il2CppString`

`#[derive(Clone, Copy)]`, wraps a `System.String` pointer.

```rust
// Impls: Display, Debug, From<&str>, TryFrom<&Il2CppString>

let s = Il2CppString::new("Hello");
let s2: Il2CppString = "Hello".into();

s.to_string_lossy();                    // → String (always succeeds)
s.len();                                // → usize (UTF-16 code units)
s.is_empty();                           // → bool
s.as_ptr();                             // → *mut c_void (for iCalls)

// Fallible conversion
let rust: String = String::try_from(&s)?;
```

#### `unsafe Il2CppString::from_raw_ptr(ptr: *mut c_void) -> Self`

Wraps a raw IL2CPP string pointer found outside the typed field API —
for example from a `Dictionary<K,V>` entry or a `List<T>` item slot.

```rust
let s = unsafe { Il2CppString::from_raw_ptr(val_ptr) };
let text = s.to_string_lossy();
```

> **Safety**: `ptr` must be null or a valid `System.String*`.

### `Il2CppArray<T>`

Wraps a managed `T[]` array. `T: Copy + 'static`.

```rust
let arr = Il2CppArray::<i32>::new(&element_class, 10)?;

// Safe access
let val = arr.get(0);                   // → Option<T> (bounds-checked)
arr.iter();                             // Iterator

// Fast access (unchecked, unsafe)
unsafe { arr.get_unchecked(0) };        // → T
unsafe { arr.set_unchecked(0, 42) };

arr.len();                              // → usize
arr.is_empty();                         // → bool
arr.to_vec();                           // → Vec<T>
arr.as_ptr();                           // → *mut c_void (array object ptr)
arr.data_ptr();                         // → *const T  (first element ptr)
unsafe { arr.as_slice() };              // → &[T] (valid until next GC)
```

#### `arr.data_ptr() -> *const T`

Raw pointer to the first element, skipping the `Il2CppArray` header.
Use together with [`len()`](#arrlen---usize) when you need to walk an
array whose element type Rust doesn't know about.

```rust
let arr = unsafe { Il2CppArray::<u8>::from_raw(entries_ptr) };
let count = arr.len();
let data = arr.data_ptr();
for i in 0..count {
    let entry = unsafe { data.add(i * 24) };
    // ...
}
```

### `Il2CppList<T>`

`#[repr(C)]` memory mapping of .NET `List<T>`. `T: Copy + 'static`.
Implements `Il2CppValueType`.

```rust
let list: Il2CppList<i32> = obj.load("scores")?;

list.len();                             // → usize
list.is_empty();                        // → bool
list.get(0);                            // → Option<T> (bounds-checked)
list.to_vec();                          // → Vec<T>
list.iter();                            // Iterator
```

> Mutation (`Add`/`Remove`) must be done through C# methods. Using
> `store_field` returns an error.

---

## Math Types

Re-exported from `il2cpp_bridge_rs::structs` (backed by
[glam](https://crates.io/crates/glam)). All are `Il2CppValueType`.

| Type | C# Equivalent |
|------|--------------|
| `Vector2` | `UnityEngine.Vector2` |
| `Vector3` | `UnityEngine.Vector3` |
| `Vector4` | `UnityEngine.Vector4` |
| `Quaternion` | `UnityEngine.Quaternion` |

```rust
let pos: Vector3 = obj.load("position")?;
let dist = pos.distance(Vector3::ZERO);
let q = Quaternion::IDENTITY;
// All glam operations: dot, cross, normalize, lerp, slerp, etc.
```

---

## iCall API

For calling IL2CPP internal calls (`[NativeHeader]` / `extern` methods)
that aren't reachable through managed method lookup.

### `cleat::resolve_icall(name: &str) -> Result<*mut c_void>`

Resolves an iCall by its fully-qualified name
(format: `"Assembly.Class::Method(args)"`).

```rust
let ptr = cleat::resolve_icall(
    "UnityEngine.AssetBundle::LoadFromFile(string)"
)?;
```

### Invocation

`invoke_icall_0` through `invoke_icall_15`, selected by argument count:

```rust
let result = unsafe { cleat::invoke_icall_0(ptr) };
let result = unsafe { cleat::invoke_icall_2(ptr, arg0, arg1) };
```

> **Safety**: `fn_ptr` must be a valid function pointer from
> `resolve_icall` with a matching signature.

---

## Hooks

### `#[cleat::hook("AssemblyName", "ClassName", "MethodName")]`

Inline hook via ShadowHook. Generates a module named after the function.

```rust
#[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
    god_mode::original(this, 0); // nullify damage
    Ok(())
}
```

Generated module:

| Function | Signature |
|----------|-----------|
| `fn_name::install()` | `-> cleat::Result<()>` — install the hook (idempotent) |
| `fn_name::uninstall()` | `-> cleat::Result<()>` — remove the hook |
| `fn_name::original(...)` | mirrored user signature — call the original function |

Supported signatures:
- Instance methods: first param is `&Il2CppObject` or `&mut Il2CppObject`
- Static methods: no `this` parameter
- Returns: `Result<()>` or `Result<T>`
- Additional params: up to 15

---

## Value Type Mapping

### `#[cleat::value_type]`

Implements `Il2CppValueType` for a custom `#[repr(C)]` struct.

```rust
#[cleat::value_type]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MyVec3 {
    x: f32,
    y: f32,
    z: f32,
}

let v: MyVec3 = obj.load("position")?;
obj.store("position", v)?;
```

---

## Traits

### `Il2CppValueType` *(unsafe trait)*

Marker trait for types ABI-compatible with IL2CPP. Requires
`Copy + 'static`. Implemented for:

| Category | Types |
|----------|-------|
| Primitives | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool` |
| Math | `Vector2`, `Vector3`, `Vector4`, `Quaternion` |
| Managed | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>` |
| Custom | Via `#[cleat::value_type]` |

### `Args` *(unsafe trait)*

Converts tuple arguments to FFI pointer arrays. Used internally by
`invoke`/`invoke_static`. The returned pointers are only valid for the
duration of the call — do not store them. Supports tuples of 0 to 16
elements.

---

## Error Types

### `Error` *(thiserror)*

| Variant | Display |
|---------|---------|
| `ClassNotFound(String)` | `class not found: {0}` |
| `AssemblyNotFound(String)` | `assembly not found: {0}` |
| `FieldNotFound(String)` | `field not found: {0}` |
| `MethodNotFound(String)` | `method not found: {0}` |
| `Bridge(String)` | `bridge error: {0}` |
| `Hook(String)` | `hook error: {0}` |

### `Result<T> = std::result::Result<T, Error>`

---

## Prelude

```rust
use cleat::prelude::*;
```

Imports: `Il2CppClass`, `Il2CppObject`, `FieldHandle`, `MethodInfo`,
`Il2CppString`, `Il2CppArray`, `Il2CppList`, `Vector2`, `Vector3`,
`Vector4`, `Quaternion`, `Error`, `Result`, `Args`, `Il2CppValueType`,
`resolve_icall`.

---

## Advanced Use

`cleat::bridge` re-exports the full `il2cpp_bridge_rs` crate for direct
access to the low-level API:

```rust
use cleat::bridge;

// Manually trigger GC
unsafe { bridge::api::gc_collect(-1) };

// Look up a type in a non-CSharp assembly
let assembly = bridge::api::cache::assembly("MyPlugin").unwrap();
let class = assembly.class("MyType").unwrap();
```
