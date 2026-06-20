# cleat API 参考

---

## 入口宏

### `#[cleat::main]`

标记 Mod 入口函数。生成 `JNI_OnLoad` 符号供 Android 调用。

```rust
#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    let klass = Il2CppClass::find("UnityEngine.Time")?;
    let ts: f32 = klass.static_field_value("timeScale")?;
    log::info!("timeScale = {ts}");
    Ok(())
}
```

---

## 类元数据 — `Il2CppClass`

### 查找

```rust
let klass = Il2CppClass::find("UnityEngine.GameObject")?;
let klass = Il2CppClass::find_with("MyType", "MyPlugin")?;
```

### 静态字段 / 方法 / 实例创建 / 泛型 / 类型谓词

见内联 rustdoc — 0.1 的所有方法保持不变。

---

## 托管对象 — `Il2CppObject`

### 字段读写

```rust
let hp: i32 = obj.load("currentHealth")?;
obj.store("currentHealth", 999)?;
let hp_field = obj.field("currentHealth")?;  // 缓存句柄
```

### 方法调用

```rust
let hp: i32 = obj.invoke("get_HP", ())?;
obj.invoke_void("SetHP", (999,))?;
let data: Il2CppObject = obj.method("GetMasterData")?.inflate(&[&klass])?.invoke(())?;
```

### 底层偏移量访问

```rust
let entries = unsafe { obj.read_ptr_at(0x18)? };    // Dictionary.entries
let count   = unsafe { obj.read_i32_at(0x20) };     // Dictionary.count
```

---

## 托管类型

### `Il2CppString`

```rust
let s = Il2CppString::new("你好");
s.to_string_lossy();            // → String
s.len();                        // → usize
s.as_ptr();                     // → *mut c_void
```

### `Il2CppArray<T>`

```rust
let arr = Il2CppArray::<i32>::new(&element_class, 10)?;
arr.get(0);                    // → Option<T>
arr.iter();                     // 迭代器
arr.to_vec();                   // → Vec<T>
arr.data_ptr();                 // → *const T
```

### `Il2CppList<T>`

```rust
let list: Il2CppList<i32> = obj.load("scores")?;
list.len();                     // → usize
list.get(0);                    // → Option<T>
list.to_vec();                  // → Vec<T>
```

### `Il2CppDictionary<K, V>` *(0.2 新增)*

`System.Collections.Generic.Dictionary<K, V>` 的 `#[repr(C)]` 内存映射。

```rust
let dict: Il2CppDictionary<*mut c_void, *mut c_void> = obj.load("lookup")?;

dict.len();                              // → usize
dict.is_empty();                         // → bool
dict.iter();                             // → DictionaryIter<(K, V)>

// 遍历所有条目（自动跳过未使用槽位 hashCode < 0）
for (key, val) in dict.iter() {
    // ...
}

// 底层 entries 数组访问
let entries = dict.entries_array()?;     // → Il2CppArray<u8>
```

Entry 布局（64位）：`hashCode(4) + next(4) + key(K) + value(V)`，按指针对齐填充。
对于 `Dictionary<string, object>`：每个 entry 24 字节。

---

## 方法指针解析

用于 Unity 引擎内部方法（`internal extern`、`[FreeFunction]`），这些方法
可能无法通过 `il2cpp_runtime_invoke` 调用。通过类方法表解析——对**所有**
绑定类型有效，包括 Unity 6 的 FreeFunction。

### `cleat::resolve_method(assembly, class, method) -> Result<MethodPtr>`

```rust
let m = cleat::resolve_method(
    "UnityEngine.AssetBundleModule.dll",
    "UnityEngine.AssetBundle",
    "LoadFromMemoryAsync_Internal",
)?;
```

### `MethodPtr`

携带 `methodPointer` + `MethodInfo*`。IL2CPP 调用约定尾部多一个 `MethodInfo*` 参数。

### 静态调用

```rust
let r = unsafe { cleat::invoke_method_0(m) };
let r = unsafe { cleat::invoke_method_2(m, a, b) };
```

### 实例调用

```rust
let r = unsafe { cleat::invoke_method_inst_0(m, this_ptr) };
let r = unsafe { cleat::invoke_method_inst_2(m, this_ptr, a, b) };
```

> 普通 C# 方法请用 `Il2CppObject::invoke` / `Il2CppClass::invoke_static`。
> `resolve_method` 仅用于 Unity 引擎内部方法。

---

## Unity 高级封装

### `AssetBundle` *(0.2 新增)*

`UnityEngine.AssetBundle` 加载的高级封装。

```rust
let font = AssetBundle::load_from_memory(&data)?
    .load_asset("FGO-Main-Font-Mod", "UnityEngine.Font", "UnityEngine.TextRenderingModule.dll")?;
```

### `cleat::load_asset_from_memory(data, asset_name, type_class, type_asm)`

便捷函数：从内存中的 AssetBundle 加载单个资源。

```rust
let font = cleat::load_asset_from_memory(
    &data,
    "MyFont",
    "UnityEngine.Font",
    "UnityEngine.TextRenderingModule.dll",
)?;
```

---

## Hook

### `#[cleat::hook("程序集名", "类名", "方法名")]`

```rust
#[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
    god_mode::original(this, 0);
    Ok(())
}
```

生成：`fn_name::install()`、`fn_name::uninstall()`、`fn_name::original(...)`。

---

## Trait

### `Il2CppValueType` *(unsafe trait)*

与 IL2CPP ABI 兼容的类型标记。要求 `Copy + 'static`。

| 分类 | 类型 |
|------|------|
| 基本类型 | `i8`–`u64`, `f32`, `f64`, `bool` |
| 数学 | `Vector2/3/4`, `Quaternion` |
| 托管 | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>`, `Il2CppDictionary<K,V>` |
| 自定义 | 通过 `#[cleat::value_type]` |

### `Args` *(unsafe trait)*

元组 → FFI 指针数组。支持 0–16 元素的元组。

---

## 错误类型

| 变体 | 显示信息 |
|------|---------|
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

一键导入：`Il2CppClass`, `Il2CppObject`, `FieldHandle`, `MethodInfo`,
`Il2CppString`, `Il2CppArray`, `Il2CppList`, `Il2CppDictionary`,
`AssetBundle`, `load_asset_from_memory`,
`Vector2/3/4`, `Quaternion`, `Error`, `Result`, `Args`, `Il2CppValueType`。