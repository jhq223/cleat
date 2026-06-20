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

## Class Metadata — `Il2CppClass`

`#[derive(Clone)]`, `Send + Sync`.

### Lookup

#### `Il2CppClass::find(name: &str) -> Result<Self>`

Finds a class by fully-qualified name including namespace.

```rust
let klass = Il2CppClass::find("UnityEngine.GameObject")?;
```

#### `Il2CppClass::find_with(name: &str, assembly_name: &str) -> Result<Self>`

Finds a class within a specific assembly.

```rust
let klass = Il2CppClass::find_with("MyPlugin.MyType", "MyPlugin")?;
```

### Static Fields / Methods / Instance Creation / Generics / Type Predicates

See inline rustdoc — all methods from 0.1 are unchanged.

---

## Managed Object — `Il2CppObject`

### Field Access

```rust
let hp: i32 = obj.load("currentHealth")?;
obj.store("currentHealth", 999)?;
let hp_field = obj.field("currentHealth")?;  // cached handle
```

### Method Calls

```rust
let hp: i32 = obj.invoke("get_HP", ())?;
obj.invoke_void("SetHP", (999,))?;
let data: Il2CppObject = obj.method("GetMasterData")?.inflate(&[&klass])?.invoke(())?;
```

### Low-level Offset Access

```rust
let entries = unsafe { obj.read_ptr_at(0x18)? };    // Dictionary.entries
let count   = unsafe { obj.read_i32_at(0x20) };     // Dictionary.count
```

---

## Managed Types

### `Il2CppString`

```rust
let s = Il2CppString::new("Hello");
s.to_string_lossy();            // → String
s.len();                        // → usize
s.as_ptr();                     // → *mut c_void
let s2: Il2CppString = "Hello".into();
```

### `Il2CppArray<T>`

```rust
let arr = Il2CppArray::<i32>::new(&element_class, 10)?;
arr.get(0);                    // → Option<T>
arr.iter();                     // Iterator
arr.to_vec();                   // → Vec<T>
arr.data_ptr();                 // → *const T (first element)
unsafe { arr.as_slice() };     // → &[T]
```

### `Il2CppList<T>`

```rust
let list: Il2CppList<i32> = obj.load("scores")?;
list.len();                     // → usize
list.get(0);                    // → Option<T>
list.to_vec();                  // → Vec<T>
```

### `Il2CppDictionary<K, V>` *(new in 0.2)*

Direct `#[repr(C)]` memory mapping of `System.Collections.Generic.Dictionary<K, V>`.

```rust
let dict: Il2CppDictionary<*mut c_void, *mut c_void> = obj.load("lookup")?;

dict.len();                              // → usize
dict.is_empty();                         // → bool
dict.iter();                             // → DictionaryIter<(K, V)>

// Walk all entries (skips unused slots where hashCode < 0)
for (key, val) in dict.iter() {
    // ...
}

// Low-level entries array access
let entries = dict.entries_array()?;     // → Il2CppArray<u8>
```

Entry layout (64-bit): `hashCode(4) + next(4) + key(K) + value(V)`, padded to
pointer alignment. For `Dictionary<string, object>`: 24 bytes per entry.

---

## Method Pointer Resolution

For Unity engine internal methods (`internal extern`, `[FreeFunction]`) that
may not work through `il2cpp_runtime_invoke`. Goes through the class method
table — works for **all** binding types including Unity 6 FreeFunction.

### `cleat::resolve_method(assembly, class, method) -> Result<MethodPtr>`

```rust
let m = cleat::resolve_method(
    "UnityEngine.AssetBundleModule.dll",
    "UnityEngine.AssetBundle",
    "LoadFromMemoryAsync_Internal",
)?;
```

### `MethodPtr`

Carries the raw `methodPointer` + `MethodInfo*`. The IL2CPP calling
convention appends a trailing `MethodInfo*` parameter.

### Static invokers

```rust
let r = unsafe { cleat::invoke_method_0(m) };
let r = unsafe { cleat::invoke_method_2(m, a, b) };
```

### Instance invokers

```rust
let r = unsafe { cleat::invoke_method_inst_0(m, this_ptr) };
let r = unsafe { cleat::invoke_method_inst_2(m, this_ptr, a, b) };
```

> Prefer `Il2CppObject::invoke` / `Il2CppClass::invoke_static` for regular
> C# methods. Use `resolve_method` only for Unity engine internals.

---

## Unity Wrappers

### `AssetBundle` *(new in 0.2)*

High-level wrapper for `UnityEngine.AssetBundle` loading.

```rust
let font = AssetBundle::load_from_memory(&data)?
    .load_asset("FGO-Main-Font-Mod", "UnityEngine.Font", "UnityEngine.TextRenderingModule.dll")?;
```

### `cleat::load_asset_from_memory(data, asset_name, type_class, type_asm)`

Convenience: load a single asset from an in-memory bundle.

```rust
let font = cleat::load_asset_from_memory(
    &data,
    "MyFont",
    "UnityEngine.Font",
    "UnityEngine.TextRenderingModule.dll",
)?;
```

---

## Hooks

### `#[cleat::hook("AssemblyName", "ClassName", "MethodName")]`

```rust
#[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
    god_mode::original(this, 0);
    Ok(())
}
```

Generated: `fn_name::install()`, `fn_name::uninstall()`, `fn_name::original(...)`.

---

## Traits

### `Il2CppValueType` *(unsafe trait)*

Types ABI-compatible with IL2CPP. Requires `Copy + 'static`.

| Category | Types |
|----------|-------|
| Primitives | `i8`–`u64`, `f32`, `f64`, `bool` |
| Math | `Vector2/3/4`, `Quaternion` |
| Managed | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>`, `Il2CppDictionary<K,V>` |
| Custom | Via `#[cleat::value_type]` |

### `Args` *(unsafe trait)*

Tuple → FFI pointer arrays. Supports 0–16 element tuples.

---

## Error Types

| Variant | Display |
|---------|---------|
| `ClassNotFound(String)` | `class not found: {0}` |
| `AssemblyNotFound(String)` | `assembly not found: {0}` |
| `FieldNotFound(String)` | `field not found: {0}` |
| `MethodNotFound(String)` | `method not found: {0}` |
| `Bridge(String)` | `bridge error: {0}` |
| `Hook(String)` | `hook error: {0}` |

---

## Prelude

```rust
use cleat::prelude::*;
```

Imports: `Il2CppClass`, `Il2CppObject`, `FieldHandle`, `MethodInfo`,
`Il2CppString`, `Il2CppArray`, `Il2CppList`, `Il2CppDictionary`,
`AssetBundle`, `load_asset_from_memory`,
`Vector2/3/4`, `Quaternion`, `Error`, `Result`, `Args`, `Il2CppValueType`.