# cleat API Reference

---

## Entry Point

### `#[cleat::main]`

Marks the mod entry function. Generates a `JNI_OnLoad` symbol that Android calls via `System.loadLibrary`.

```rust
#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    cleat::set_app_data("/data/data/com.example.game/files");
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

Initializes android_logger and il2cpp_bridge_rs. Called automatically by `#[cleat::main]`.

### `cleat::set_app_data(path: impl AsRef<Path>)`

Sets the app's private data directory (usually `/data/data/<package>/files`). Only succeeds on the first call.

```rust
cleat::set_app_data("/data/data/com.example.game/files");
```

### `cleat::app_data() -> Result<&'static Path>`

Returns the path set by `set_app_data`. Returns `Err(Error::NotInitialized)` if not set.

```rust
let config = cleat::app_data()?.join("config.json");
```

---

## Class Metadata — `Il2CppClass`

`#[derive(Clone)]`, `Send + Sync`.

### `Il2CppClass::find(name: &str) -> Result<Self>`

Finds a class by fully-qualified name including namespace.

```rust
let klass = Il2CppClass::find("UnityEngine.GameObject")?;
let player = Il2CppClass::find("Player")?;
```

### `Il2CppClass::find_with(name: &str, assembly_name: &str) -> Result<Self>`

Finds a class within a specific assembly.

```rust
let klass = Il2CppClass::find_with("MyPlugin.MyType", "MyPlugin")?;
```

### `class.static_field_value<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

Reads a static field.

```rust
let width: i32 = Il2CppClass::find("UnityEngine.Screen")?
    .static_field_value("width")?;
```

### `class.parent(&self) -> Option<Self>`

Returns the parent class, or `None` if there is none.

### `class.new_object(&self) -> Result<Il2CppObject>`

Creates an instance without calling the constructor (like `FormatterServices.GetUninitializedObject`).

### `class.create_instance(&self) -> Result<Il2CppObject>`

Creates an instance via the default constructor (like `Activator.CreateInstance`).

### `class.find_objects(&self, include_inactive: bool) -> Vec<Il2CppObject>`

Returns all objects of this type in the current scene.

### `class.is_subclass_of(&self, other: &Self) -> bool`

Returns `true` if `self` is a subclass of `other`.

### `class.init(&self)`

Runs the static class constructor (equivalent to `RuntimeHelpers.RunClassConstructor`).

### `class.invoke_static<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

Calls a static method that returns a value.

```rust
let result: i32 = klass.invoke_static("Calculate", (10, 20))?;
```

### `class.invoke_static_void(&self, name: &str, args: impl Args) -> Result<()>`

Calls a static void method.

### `class.method_ptr(&self, name: &str) -> Result<bridge::structs::Method>`

Returns the raw bridge method pointer. Used internally by `#[cleat::hook]`.

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
let weapon: Il2CppObject = obj.load("equippedWeapon")?;
```

#### `obj.store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()>`

Writes an instance field.

```rust
obj.store("currentHealth", 999)?;
obj.store("playerName", Il2CppString::new("Hacker"))?;
```

### Method Calls

#### `obj.invoke<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

Calls a parameterless instance method that returns a value.

```rust
let name: Il2CppString = obj.invoke("get_Name")?;
let hp: i32 = obj.invoke("get_HP")?;
```

#### `obj.invoke_void(&self, name: &str) -> Result<()>`

Calls a parameterless void instance method.

#### `obj.invoke_with<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

Calls an instance method with parameters (returns a value).

```rust
let result: i32 = obj.invoke_with("Calculate", (10, 20))?;
```

#### `obj.invoke_with_void(&self, name: &str, args: impl Args) -> Result<()>`

Calls an instance method with parameters (void return).

#### `obj.invoke_raw_void(&self, name: &str, args: &[*mut c_void]) -> Result<()>`

Calls a void method with raw C pointers. Use when the `Args` trait would pass the wrong pointer — for example, when passing `Il2CppObject` values whose stack address would be sent instead of the GC object pointer.

#### `obj.method(&self, name: &str) -> Result<MethodInfo>`

Gets a method handle for generic method specialization.

```rust
let data: Il2CppObject = obj
    .method("GetMasterData")?
    .inflate(&[&some_class])?
    .invoke(())?;
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
let dict_ptr = script.raw_ptr();
let entries = unsafe { script.read_ptr_at(0x18)? }; // Dictionary.entries
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

### Internal (Hook macro)

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
// Decode a string from a Dictionary entry's value slot:
let s = unsafe { Il2CppString::from_raw_ptr(val_ptr) };
let text = s.to_string_lossy();
```

> **Safety**: `ptr` must be null or a valid `System.String*`. Passing a
> non-string pointer produces garbage or Undefined Behaviour.

### `Il2CppArray<T>`

Wraps a managed `T[]` array. `T: Copy + 'static`.

```rust
let arr = Il2CppArray::<i32>::new(&element_class, 10)?;

arr.set(0, 42);                         // &mut self
let val = arr.get(0);                   // no bounds check
arr.at(5);                              // → Option<T> (bounds-checked)
arr.len();                              // → usize
arr.is_empty();                         // → bool
arr.to_vec();                           // → Vec<T>
arr.as_ptr();                           // → *mut c_void (array object ptr)
arr.data_ptr();                         // → *const T  (first element ptr)
```

#### `arr.data_ptr() -> *const T`

Raw pointer to the first element, skipping the `Il2CppArray` header.
Use together with [`len()`](#arrlen---usize) when you need to walk an
array whose element type is a managed struct that Rust doesn't know
about (e.g. `Dictionary.Entry`).

```rust
let arr = unsafe { Il2CppArray::<u8>::from_raw(entries_ptr) };
let count = arr.len();
let data = arr.data_ptr();
for i in 0..count {
    let entry = unsafe { data.add(i * 24) }; // 24 = sizeof(Entry)
    // ...
}
```

### `Il2CppList<T>`

`#[repr(C)]` memory mapping of .NET `List<T>`. `T: Copy + 'static`. `Il2CppValueType`.

```rust
let list: Il2CppList<i32> = obj.load("scores")?;

list.len();                             // → usize
list.is_empty();                        // → bool
list.get(0);                            // → Option<T> (bounds-checked)
list.to_vec();                          // → Vec<T>
for item in list.iter() { /* ... */ }   // Iterator
```

> Mutation (`Add`/`Remove`) must be done through C# methods. Using `store_field` returns an error.

---

## Math Types

Re-exported from `il2cpp_bridge_rs::structs` (backed by [glam](https://crates.io/crates/glam)). All are `Il2CppValueType`.

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
// All glam operations available: dot, cross, normalize, lerp, slerp, etc.
```

---

## iCall API

For calling IL2CPP internal calls (`[NativeHeader]` / `extern` methods) that aren't reachable through managed method lookup.

### `cleat::resolve_icall(name: &str) -> Result<*mut c_void>`

Resolves an iCall by its fully-qualified name (format: `"Assembly.Class::Method(args)"`).

```rust
let ptr = cleat::resolve_icall(
    "UnityEngine.AssetBundle::LoadFromFile(string)"
)?;
```

### `unsafe cleat::invoke_icall_0(fn_ptr: *mut c_void) -> *mut c_void`

Call a resolved iCall with 0 arguments.

### `unsafe cleat::invoke_icall_1(fn_ptr: *mut c_void, a: *mut c_void) -> *mut c_void`

Call a resolved iCall with 1 argument.

### `unsafe cleat::invoke_icall_2(fn_ptr: *mut c_void, a: *mut c_void, b: *mut c_void) -> *mut c_void`

Call a resolved iCall with 2 arguments.

### `unsafe cleat::invoke_icall_3(fn_ptr: *mut c_void, a: *mut c_void, b: *mut c_void, c: *mut c_void) -> *mut c_void`

Call a resolved iCall with 3 arguments.

> **Safety**: `fn_ptr` must be a valid function pointer from `resolve_icall` with a matching signature.

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

### `Il2CppValueType`  *(unsafe trait)*

Marker trait for types ABI-compatible with IL2CPP. Requires `Copy + 'static`. Implemented for:

| Category | Types |
|----------|-------|
| Primitives | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool` |
| Math | `Vector2`, `Vector3`, `Vector4`, `Quaternion` |
| Managed | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>` |
| Custom | Via `#[cleat::value_type]` |

### `Args`  *(unsafe trait)*

Converts tuple arguments to FFI pointer arrays. Used internally by `invoke_with`/`invoke_static`. The returned pointers are only valid for the duration of the call — do not store them.

---

## Error Types

### `Error`  *(thiserror)*

| Variant | Display |
|---------|---------|
| `ClassNotFound(String)` | `class not found: {0}` |
| `AssemblyNotFound(String)` | `assembly not found: {0}` |
| `FieldNotFound(String)` | `field not found: {0}` |
| `MethodNotFound(String)` | `method not found: {0}` |
| `NotInitialized` | `cleat not initialized: call set_app_data() first` |
| `Bridge(String)` | `bridge error: {0}` |
| `Hook(String)` | `hook error: {0}` |

### `Result<T> = std::result::Result<T, Error>`

---

## Prelude

```rust
use cleat::prelude::*;
```

Imports: `Il2CppClass`, `Il2CppObject`, `MethodInfo`, `Il2CppString`, `Il2CppArray`, `Il2CppList`, `Vector2`, `Vector3`, `Vector4`, `Quaternion`, `Error`, `Result`, `Args`, `Il2CppValueType`, `app_data`, `set_app_data`.
