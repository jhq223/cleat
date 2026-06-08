# cleat API 参考

---

## 入口宏

### `#[cleat::main]`

标记 Mod 入口函数。生成 `JNI_OnLoad` 符号供 Android 通过 `System.loadLibrary` 调用。

```rust
#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    let klass = Il2CppClass::find("UnityEngine.Time")?;
    let ts: f32 = klass.static_field_value("timeScale")?;
    log::info!("timeScale = {ts}");
    Ok(())
}
```

生成的代码会依次：
1. 调用 `android_logger::init_once(...)` — 日志输出到 logcat
2. 调用 `il2cpp_bridge_rs::init("libil2cpp.so", ...)`
3. IL2CPP 元数据就绪后，执行你的函数

---

## 初始化

### `cleat::init(target_image: &str, on_ready: impl FnOnce() + Send + 'static)`

初始化 android_logger 和 il2cpp_bridge_rs。由 `#[cleat::main]` 自动调用。

---

## 类元数据 — `Il2CppClass`

`#[derive(Clone)]`，`Send + Sync`。

### 查找

#### `Il2CppClass::find(name: &str) -> Result<Self>`

按全限定类名查找（含命名空间）。

```rust
let klass = Il2CppClass::find("UnityEngine.GameObject")?;
let player = Il2CppClass::find("Player")?;
```

#### `Il2CppClass::find_with(name: &str, assembly_name: &str) -> Result<Self>`

在指定程序集中查找类。

```rust
let klass = Il2CppClass::find_with("MyPlugin.MyType", "MyPlugin")?;
```

### 静态字段

#### `class.static_field_value<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

读取静态字段。

```rust
let width: i32 = Il2CppClass::find("UnityEngine.Screen")?
    .static_field_value("width")?;
```

#### `class.set_static_field_value<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()>`

写入静态字段。

```rust
Il2CppClass::find("UnityEngine.Time")?
    .set_static_field_value("timeScale", 2.0f32)?;
```

### 继承链

#### `class.parent(&self) -> Option<Self>`

获取父类，无父类返回 `None`。

#### `class.is_subclass_of(&self, other: &Self, check_interfaces: bool) -> bool`

判断 `self` 是否是 `other` 的子类。`check_interfaces = true` 时同时检查接口继承。

### 实例创建

#### `class.new_object(&self) -> Result<Il2CppObject>`

创建未初始化实例（等效于 `FormatterServices.GetUninitializedObject`）。

#### `class.create_instance(&self) -> Result<Il2CppObject>`

通过默认构造函数创建实例（等效于 `Activator.CreateInstance`）。

### 场景遍历

#### `class.find_objects(&self, include_inactive: bool) -> Vec<Il2CppObject>`

返回当前场景中所有该类型的对象。

### 静态方法调用

#### `class.invoke_static<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

调用有返回值的静态方法。`args` 为 `()` 时表示无参数。

```rust
let result: i32 = klass.invoke_static("Calculate", (10, 20))?;
let time: f32 = klass.invoke_static("GetTime", ())?;
```

#### `class.invoke_static_void(&self, name: &str, args: impl Args) -> Result<()>`

调用无返回值的静态方法。

```rust
klass.invoke_static_void("Init", ())?;
klass.invoke_static_void("SetValues", (1, 2.0f32))?;
```

### 方法句柄

#### `class.method(&self, name: &str) -> Result<MethodInfo>`

获取静态方法的 [`MethodInfo`](#方法句柄--methodinfo) 句柄，用于泛型方法特化。

### 泛型

#### `class.inflate(&self, type_args: &[&Il2CppClass]) -> Result<Self>`

从泛型定义创建具体类型。

```rust
let list_t = Il2CppClass::find("System.Collections.Generic.List`1")?;
let int32 = Il2CppClass::find("System.Int32")?;
let list_int = list_t.inflate(&[&int32])?;
```

### 类型谓词

```rust
class.is_valuetype()    // → bool  值类型 (struct)
class.is_enum()         // → bool  枚举
class.is_generic()      // → bool  泛型定义
class.is_interface()    // → bool  接口
class.is_abstract()     // → bool  抽象类
```

### 初始化

#### `class.init(&self)`

运行类的静态构造函数（等效于 `RuntimeHelpers.RunClassConstructor`）。

---

## 托管对象 — `Il2CppObject`

`#[derive(Clone, Copy)]`，`Send + Sync`。

### 字段读写

#### `obj.load<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

读取实例字段。

```rust
let hp: i32 = obj.load("currentHealth")?;
let name: Il2CppString = obj.load("playerName")?;
let pos: Vector3 = obj.load("position")?;
```

#### `obj.store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()>`

写入实例字段。

```rust
obj.store("currentHealth", 999)?;
obj.store("playerName", Il2CppString::new("Hacker"))?;
```

#### `obj.field(&self, name: &str) -> Result<FieldHandle>`

获取缓存的字段句柄。句柄记住字段偏移量，后续 `get`/`set` 跳过名称查找。

```rust
let hp_field = obj.field("currentHealth")?;
let hp: i32 = hp_field.get()?;
hp_field.set(999)?;
```

### 方法调用

#### `obj.invoke<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

调用有返回值的实例方法。`args` 为 `()` 时表示无参数。

```rust
let hp: i32 = obj.invoke("get_HP", ())?;
let result: i32 = obj.invoke("Calculate", (10, 20))?;
```

#### `obj.invoke_void(&self, name: &str, args: impl Args) -> Result<()>`

调用无返回值的实例方法。

```rust
obj.invoke_void("Update", ())?;
obj.invoke_void("SetHP", (999,))?;
```

#### `obj.method(&self, name: &str) -> Result<MethodInfo>`

获取方法句柄，用于泛型方法特化。

```rust
let data: Il2CppObject = obj
    .method("GetMasterData")?
    .inflate(&[&some_class])?
    .invoke(())?;
```

### 运行时类型

#### `obj.class(&self) -> Result<Il2CppClass>`

获取对象的运行时类型。

```rust
let klass = obj.class()?;
if klass.is_subclass_of(&player_class, false) {
    // ...
}
```

### 底层字段访问（按偏移量）

当字段类型未在 cleat 注册时（私有字段、`Dictionary<K,V>` 等托管结构体内部的字段），
以下 escape hatches 允许通过已知的字节偏移量直接读写内存。

本节的**所有方法都是 `unsafe` 的**——调用者必须保证偏移量在对象分配的内存范围内。

#### `unsafe obj.read_ptr_at(offset: isize) -> Option<*mut c_void>`

读取指定偏移量处的一个指针值。对象指针或字段自身为 null 时返回 `None`。

```rust
let dict_ptr = obj.raw_ptr();
let entries = unsafe { obj.read_ptr_at(0x18)? }; // Dictionary.entries
```

#### `unsafe obj.write_ptr_at(offset: isize, val: *mut c_void)`

将指针值写入指定偏移量。

```rust
unsafe { obj.write_ptr_at(0x18, new_array.as_ptr()) };
```

#### `unsafe obj.read_i32_at(offset: isize) -> i32`

读取偏移量处的一个 32 位有符号整数。

```rust
let count = unsafe { dict.read_i32_at(0x20) }; // Dictionary.count
```

#### `unsafe obj.read_usize_at(offset: isize) -> usize`

读取偏移量处的一个指针宽度无符号整数。

```rust
let len = unsafe { arr.read_usize_at(0x18) }; // Il2CppArray.max_length
```

### 内部方法

#### `obj.raw_ptr(&self) -> *mut c_void`

获取原始 IL2CPP 对象指针。配合 [`from_raw`](#unsafe-il2cppobjectfrom_rawptr-mut_c_void---self) 可在类型化访问和偏移量访问之间自由切换。

#### `unsafe Il2CppObject::from_raw(ptr: *mut c_void) -> Self`

从原始指针构造。指针必须是一个有效的 GC 托管对象或 null。

```rust
let obj = unsafe { Il2CppObject::from_raw(some_ptr) };
let field = unsafe { obj.read_ptr_at(0x10) };
```

---

## 字段句柄 — `FieldHandle`

通过 [`Il2CppObject::field`](#objfieldself-name-str---resultfieldhandle) 获取，
缓存字段偏移量，重复读写无需名称查找。

```rust
let hp_field = obj.field("currentHealth")?;

// 读取
let hp: i32 = hp_field.get()?;

// 写入
hp_field.set(999)?;
```

---

## 方法句柄 — `MethodInfo`

`#[derive(Clone)]`，`Send + Sync`。

### `method.inflate(&self, type_args: &[&Il2CppClass]) -> Result<Self>`

注入泛型类型参数。实例绑定自动继承。

```rust
let inflated = method.inflate(&[&int_class, &string_class])?;
```

### `method.invoke<T: Il2CppValueType>(&self, args: impl Args) -> Result<T>`

调用方法（有返回值）。

### `method.invoke_void(&self, args: impl Args) -> Result<()>`

调用方法（无返回值）。

---

## 托管类型

### `Il2CppString`

`#[derive(Clone, Copy)]`，封装 `System.String` 指针。

```rust
// 已实现：Display, Debug, From<&str>, TryFrom<&Il2CppString>

let s = Il2CppString::new("你好");
let s2: Il2CppString = "你好".into();

s.to_string_lossy();                    // → String（总是成功）
s.len();                                // → usize（UTF-16 码元数）
s.is_empty();                           // → bool
s.as_ptr();                             // → *mut c_void（iCall 用）

// 可失败转换
let rust: String = String::try_from(&s)?;
```

#### `unsafe Il2CppString::from_raw_ptr(ptr: *mut c_void) -> Self`

从在类型化字段 API 之外获取的裸指针构造——例如从 `Dictionary<K,V>` entry 或 `List<T>` 元素中取出的指针。

```rust
let s = unsafe { Il2CppString::from_raw_ptr(val_ptr) };
let text = s.to_string_lossy();
```

> **安全要求**：`ptr` 必须为 null 或一个有效的 `System.String*`。

### `Il2CppArray<T>`

封装托管 `T[]` 数组。`T: Copy + 'static`。

```rust
let arr = Il2CppArray::<i32>::new(&element_class, 10)?;

// 安全读写
let val = arr.get(0);                   // → Option<T>（有边界检查）
arr.iter();                             // 安全迭代器

// 高性能（无边界检查，unsafe）
unsafe { arr.get_unchecked(0) };        // → T
unsafe { arr.set_unchecked(0, 42) };

arr.len();                              // → usize
arr.is_empty();                         // → bool
arr.to_vec();                           // → Vec<T>
arr.as_ptr();                           // → *mut c_void（数组对象指针）
arr.data_ptr();                         // → *const T  （首元素指针）
unsafe { arr.as_slice() };              // → &[T]（直到下次 GC）
```

#### `arr.data_ptr() -> *const T`

跳过 Il2CppArray 头部的首元素裸指针。配合 [`len()`](#arrlen---usize) 遍历 Rust 侧未定义类型的托管结构体数组。

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

.NET `List<T>` 的 `#[repr(C)]` 内存映射。`T: Copy + 'static`。实现了 `Il2CppValueType`。

```rust
let list: Il2CppList<i32> = obj.load("scores")?;

list.len();                             // → usize
list.is_empty();                        // → bool
list.get(0);                            // → Option<T>（有越界检查）
list.to_vec();                          // → Vec<T>
list.iter();                            // 迭代器
```

> 修改（`Add`/`Remove`）需通过 C# 方法完成。`store_field` 会返回错误。

---

## 数学类型

从 `il2cpp_bridge_rs::structs` 重导出（底层基于 [glam](https://crates.io/crates/glam)）。全部实现了 `Il2CppValueType`。

| 类型 | C# 对应 |
|------|--------|
| `Vector2` | `UnityEngine.Vector2` |
| `Vector3` | `UnityEngine.Vector3` |
| `Vector4` | `UnityEngine.Vector4` |
| `Quaternion` | `UnityEngine.Quaternion` |

```rust
let pos: Vector3 = obj.load("position")?;
let dist = pos.distance(Vector3::ZERO);
let q = Quaternion::IDENTITY;
// 完整 glam 数学运算：dot, cross, normalize, lerp, slerp 等
```

---

## iCall API

用于调用通过托管方法查找无法到达的 IL2CPP 内部调用（`[NativeHeader]` / `extern` 方法）。

### `cleat::resolve_icall(name: &str) -> Result<*mut c_void>`

按全限定名解析 iCall（格式：`"Assembly.Class::Method(args)"`）。

```rust
let ptr = cleat::resolve_icall(
    "UnityEngine.AssetBundle::LoadFromFile(string)"
)?;
```

### 调用函数

`invoke_icall_0` 到 `invoke_icall_15`，按参数个数选择：

```rust
let result = unsafe { cleat::invoke_icall_0(ptr) };
let result = unsafe { cleat::invoke_icall_2(ptr, arg0, arg1) };
```

> **安全性**：`fn_ptr` 必须是通过 `resolve_icall` 获取的、签名匹配的有效函数指针。

---

## Hook

### `#[cleat::hook("程序集名", "类名", "方法名")]`

基于 ShadowHook 的 inline hook。生成以函数名命名的模块。

```rust
#[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
    god_mode::original(this, 0); // 伤害清零
    Ok(())
}
```

生成的模块：

| 函数 | 签名 |
|------|------|
| `函数名::install()` | `-> cleat::Result<()>` — 安装 hook（幂等） |
| `函数名::uninstall()` | `-> cleat::Result<()>` — 卸载 hook |
| `函数名::original(...)` | 与用户签名一致 — 调用原函数 |

支持的签名：
- 实例方法：第一个参数为 `&Il2CppObject` 或 `&mut Il2CppObject`
- 静态方法：无 `this` 参数
- 返回：`Result<()>` 或 `Result<T>`
- 额外参数：最多 15 个

---

## 值类型映射

### `#[cleat::value_type]`

为自定义 `#[repr(C)]` 结构体实现 `Il2CppValueType`。

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

## Trait

### `Il2CppValueType` *(unsafe trait)*

标记与 IL2CPP ABI 兼容的类型。要求 `Copy + 'static`。已实现：

| 分类 | 类型 |
|------|------|
| 基本类型 | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool` |
| 数学 | `Vector2`, `Vector3`, `Vector4`, `Quaternion` |
| 托管类型 | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>` |
| 自定义 | 通过 `#[cleat::value_type]` |

### `Args` *(unsafe trait)*

将元组参数转为 FFI 裸指针数组，由 `invoke`/`invoke_static` 内部使用。返回的指针仅在调用期间有效，**不可保存**。支持 0 到 16 个元素的元组。

---

## 错误类型

### `Error` *(thiserror)*

| 变体 | 显示信息 |
|------|---------|
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

一键导入：`Il2CppClass`, `Il2CppObject`, `FieldHandle`, `MethodInfo`, `Il2CppString`, `Il2CppArray`, `Il2CppList`, `Vector2`, `Vector3`, `Vector4`, `Quaternion`, `Error`, `Result`, `Args`, `Il2CppValueType`, `resolve_icall`。

---

## 高级用途

`cleat::bridge` 完整 re-export `il2cpp_bridge_rs`，可直接调用底层 API：

```rust
use cleat::bridge;

// 手动触发 GC
unsafe { bridge::api::gc_collect(-1) };

// 在非 CSharp 程序集中查找类型
let assembly = bridge::api::cache::assembly("MyPlugin").unwrap();
let class = assembly.class("MyType").unwrap();
```
