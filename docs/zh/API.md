# cleat API 参考

---

## 入口宏

### `#[cleat::main]`

标记 Mod 入口函数。生成 `load()` 符号供 Android 通过 `System.loadLibrary` 调用。

```rust
#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    cleat::set_app_data("/data/data/com.example.game/files");
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

### `cleat::set_app_data(path: impl AsRef<Path>)`

设置 app 私有数据目录（通常为 `/data/data/<包名>/files`）。仅首次调用生效。

```rust
cleat::set_app_data("/data/data/com.example.game/files");
```

### `cleat::app_data() -> Result<&'static Path>`

获取 `set_app_data` 设置的路径。未设置时返回 `Err(Error::NotInitialized)`。

```rust
let config = cleat::app_data()?.join("config.json");
```

---

## 类元数据 — `Il2CppClass`

`#[derive(Clone)]`，`Send + Sync`。

### `Il2CppClass::find(name: &str) -> Result<Self>`

按全限定类名查找（含命名空间）。

```rust
let klass = Il2CppClass::find("UnityEngine.GameObject")?;
let player = Il2CppClass::find("Player")?;
```

### `Il2CppClass::find_with(name: &str, assembly_name: &str) -> Result<Self>`

在指定程序集中查找类。

```rust
let klass = Il2CppClass::find_with("MyPlugin.MyType", "MyPlugin")?;
```

### `class.static_field_value<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

读取静态字段。

```rust
let width: i32 = Il2CppClass::find("UnityEngine.Screen")?
    .static_field_value("width")?;
```

### `class.parent(&self) -> Option<Self>`

获取父类，无父类返回 `None`。

### `class.new_object(&self) -> Result<Il2CppObject>`

创建未初始化实例（等效于 `FormatterServices.GetUninitializedObject`）。

### `class.create_instance(&self) -> Result<Il2CppObject>`

通过默认构造函数创建实例（等效于 `Activator.CreateInstance`）。

### `class.find_objects(&self, include_inactive: bool) -> Vec<Il2CppObject>`

返回当前场景中所有该类型的对象。

### `class.is_subclass_of(&self, other: &Self) -> bool`

判断 `self` 是否是 `other` 的子类。

### `class.init(&self)`

运行类的静态构造函数（等效于 `RuntimeHelpers.RunClassConstructor`）。

### `class.invoke_static<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

调用有返回值的静态方法。

```rust
let result: i32 = klass.invoke_static("Calculate", (10, 20))?;
```

### `class.invoke_static_void(&self, name: &str, args: impl Args) -> Result<()>`

调用无返回值的静态方法。

### `class.method_ptr(&self, name: &str) -> Result<bridge::structs::Method>`

获取原生 bridge 方法指针。由 `#[cleat::hook]` 宏内部使用。

---

## 托管对象 — `Il2CppObject`

`#[derive(Clone, Copy)]`，`!Send + !Sync`。

### 字段读写

#### `obj.load<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

读取实例字段。

```rust
let hp: i32 = obj.load("currentHealth")?;
let name: Il2CppString = obj.load("playerName")?;
let pos: Vector3 = obj.load("position")?;
let weapon: Il2CppObject = obj.load("equippedWeapon")?;
```

#### `obj.store<T: Il2CppValueType>(&self, name: &str, val: T) -> Result<()>`

写入实例字段。

```rust
obj.store("currentHealth", 999)?;
obj.store("playerName", Il2CppString::new("Hacker"))?;
```

### 方法调用

#### `obj.invoke<T: Il2CppValueType>(&self, name: &str) -> Result<T>`

调用无参、有返回值的实例方法。

```rust
let name: Il2CppString = obj.invoke("get_Name")?;
let hp: i32 = obj.invoke("get_HP")?;
```

#### `obj.invoke_void(&self, name: &str) -> Result<()>`

调用无参、无返回值的实例方法。

#### `obj.invoke_with<T: Il2CppValueType>(&self, name: &str, args: impl Args) -> Result<T>`

调用有参、有返回值的实例方法。

```rust
let result: i32 = obj.invoke_with("Calculate", (10, 20))?;
```

#### `obj.invoke_with_void(&self, name: &str, args: impl Args) -> Result<()>`

调用有参、无返回值的实例方法。

#### `obj.method(&self, name: &str) -> Result<MethodInfo>`

获取方法句柄，用于泛型方法特化。

```rust
let data: Il2CppObject = obj
    .method("GetMasterData")?
    .inflate(&[&some_class])?
    .invoke(())?;
```

### 内部方法（Hook 宏使用）

#### `obj.raw_ptr(&self) -> *mut c_void`  *(doc hidden)*

获取原始 IL2CPP 对象指针。

#### `unsafe Il2CppObject::from_raw(ptr: *mut c_void) -> Self`  *(doc hidden)*

从原始指针构造。

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
s.len();                                // → usize（字符数）
s.is_empty();                           // → bool

// 可失败转换
let rust: String = String::try_from(&s)?;
```

### `Il2CppArray<T>`

封装托管 `T[]` 数组。`T: Copy + 'static`。

```rust
let arr = Il2CppArray::<i32>::new(&element_class, 10)?;

arr.set(0, 42);                         // &mut self
let val = arr.get(0);                   // 不做越界检查
arr.len();                              // → usize
arr.is_empty();                         // → bool
arr.to_vec();                           // → Vec<T>
```

### `Il2CppList<T>`

.NET `List<T>` 的 `#[repr(C)]` 内存映射。`T: Copy + 'static`。实现了 `Il2CppValueType`。

```rust
let list: Il2CppList<i32> = obj.load("scores")?;

list.len();                             // → usize
list.is_empty();                        // → bool
list.get(0);                            // → Option<T>（有越界检查）
list.to_vec();                          // → Vec<T>
for item in list.iter() { /* ... */ }   // 迭代器
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

### `Il2CppValueType`  *(unsafe trait)*

标记与 IL2CPP ABI 兼容的类型。要求 `Copy + 'static`。已实现：

| 分类 | 类型 |
|------|------|
| 基本类型 | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool` |
| 数学 | `Vector2`, `Vector3`, `Vector4`, `Quaternion` |
| 托管类型 | `Il2CppObject`, `Il2CppString`, `Il2CppList<T>` |
| 自定义 | 通过 `#[cleat::value_type]` |

### `Args`  *(unsafe trait)*

将元组参数转为 FFI 裸指针数组，由 `invoke_with`/`invoke_static` 内部使用。返回的指针仅在调用期间有效——**不可保存**。

---

## 错误类型

### `Error`  *(thiserror)*

| 变体 | 显示信息 |
|------|---------|
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

一键导入：`Il2CppClass`, `Il2CppObject`, `MethodInfo`, `Il2CppString`, `Il2CppArray`, `Il2CppList`, `Vector2`, `Vector3`, `Vector4`, `Quaternion`, `Error`, `Result`, `Args`, `Il2CppValueType`, `app_data`, `set_app_data`。
