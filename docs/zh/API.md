# API

[English](../en/API.md) · [构建](BUILD.md) · [完整示例](../../examples)

示例中的 `Game.Player`、`health` 等名称需要替换为目标游戏的类型和成员名。函数签名可用 `cargo doc --no-deps --open` 查看。

## 读取和修改对象

```rust
use cleat::prelude::*;

fn update_player(player: Object) -> Result<()> {
    let hp: i32 = player.get("health")?;
    player.set("health", hp + 100)?;
    player.set("displayName", "御主")?;
    player.call::<()>("Refresh", ())?;
    Ok(())
}
```

`get`、`set` 支持字段和属性。Rust 字符串可以直接传入 `set` 和 `call`。返回值使用 Rust 类型标注，例如 `i32`、`f32`、`bool`、`Il2CppString` 或 `Object`；C# 的 `void` 对应 `()`。

静态成员通过 `Class` 访问：

```rust
use cleat::prelude::*;

fn manager() -> Result<Object> {
    Class::find_in("Assembly-CSharp", "Game.GameManager")?.get("instance")
}
```

`Class::find("Game.GameManager")` 会跨程序集查找。遇到同名类型时，用 `find_in` 指定程序集。

## 调用方法

参数放在元组里：无参数用 `()`，一个参数用 `(value,)`。同名重载可通过 C# 参数类型选择：

```rust
use cleat::prelude::*;

fn set_title(label: Object) -> Result<()> {
    label.call::<()>(("SetText", ["System.String"]), ("开始游戏",))
}
```

重复调用时，可以保存 `object.method(...)` 返回的 `MethodInfo`，再用 `invoke` 调用。`method.virtual_method()` 取得绑定对象的实际虚方法。

`ref` 和 `out` 参数使用 `by_ref`、`out`：

```rust
use cleat::prelude::*;

fn parse_number(text: &str) -> Result<Option<i32>> {
    let mut number = 0i32;
    let parsed: bool = Class::find("System.Int32")?.call(
        ("TryParse", ["System.String", "System.Int32&"]),
        (text, out(&mut number)),
    )?;
    Ok(parsed.then_some(number))
}
```

## 创建对象和泛型

`construct` 分配对象并调用构造函数。指定构造重载时使用 `construct_with`。例如创建 `List<string>`：

```rust
use cleat::prelude::*;

fn string_list() -> Result<Object> {
    let string = Class::find("System.String")?;
    let list = Class::find("System.Collections.Generic.List`1")?.generic(&[&string])?;
    list.construct(())
}
```

泛型方法同样使用 `method.generic(&[&class])`。`Class::box_value` 和 `Object::unbox` 用于值类型装箱与拆箱。

## 替换文本

```rust
use cleat::prelude::*;

#[cleat::main]
fn start() -> Result<()> {
    text::install()
}

#[cleat::hook("UnityEngine.UI", "UnityEngine.UI.Text", "set_text")]
fn text(this: &Object, value: Il2CppString) -> Result<()> {
    let value = match value.to_string_lossy().as_str() {
        "Start Game" => Il2CppString::try_new("开始游戏")?,
        "Settings" => Il2CppString::try_new("设置")?,
        _ => value,
    };
    text::original(this, value)
}
```

宏参数依次为程序集、类型名和方法名。程序集可省略，例如 `#[cleat::hook("Game.Player", "Start")]`。`text::original` 调用原方法，`text::install` 安装 Hook；在目标方法停止调用后，使用 `text::uninstall()` 卸载。

Hook 的 Rust 参数及返回类型应与目标方法一致。宏支持普通参数、静态方法和引用类型实例方法，实例以 `this: &Object` 接收。

## 集合

数组、列表和字典可以直接从托管字段或方法返回值读取：

```rust
use cleat::prelude::*;

fn read_items(inventory: Object) -> Result<Vec<(i32, i32)>> {
    let counts: Il2CppDictionary<i32, i32> = inventory.get("itemCounts")?;
    let count = counts.get(1001)?.unwrap_or(0);
    counts.set(1001, count + 1)?;
    counts.iter()?.collect()
}
```

| 类型 | 常用操作 |
|---|---|
| `Il2CppArray<T>` | `len`、`get`、`set`、`iter`、`from_slice` |
| `Il2CppList<T>` | `len`、`get`、`set`、`push`、`remove`、`clear`、`snapshot` |
| `Il2CppDictionary<K, V>` | `len`、`get`、`set`、`remove`、`iter` |

字典 `get` 返回 `Result<Option<V>>`，`iter` 中的每一项也是 `Result`。跨帧遍历列表时使用 `snapshot()`，返回的 `RootedArray<T>` 会保持数组和元素存活。

## 对象寿命

跨回调保存对象时，保存它的 GC 根，并通过 `root.object()` 取回对象：

```rust
use cleat::prelude::*;

fn refresh_later(player: Object, scheduler: &CoroutineScheduler) -> Result<CoroutineTask> {
    let player = player.root()?;
    Ok(scheduler.spawn(async move {
        coroutine::next_frame().await;
        player.object().call::<()>("Refresh", ())
    }))
}
```

根在释放时结束保活。将对象地址交给原生代码长期保存时，使用 `GcHandle::pinned`；`object.root()` 已采用固定地址的根。

## 协程

在 Unity 主线程回调中取得一个带 `Update` 方法的 MonoBehaviour，然后安装 `CoroutineRunner`：

```rust
use cleat::prelude::*;

fn start_tasks(owner: Object) -> Result<CoroutineRunner> {
    let runner = CoroutineRunner::install(owner)?;
    runner.spawn(async {
        coroutine::next_frame().await;
        coroutine::wait_seconds(0.5).await?;
        Ok(())
    });
    Ok(runner)
}
```

保存返回的 runner，让任务随该对象的 `Update` 执行。释放 runner 会取消任务并卸载 Hook，应在目标对象停止更新后释放。已有 Update Hook 时，可使用 `CoroutineScheduler` 并在每帧调用一次 `tick()`。

| 等待 | 用法 |
|---|---|
| 下一帧 | `next_frame().await` |
| 指定帧数 | `wait_frames(10).await` |
| 游戏时间 | `wait_seconds(0.5).await?` |
| 实际时间 | `wait_realtime(Duration::from_secs(1)).await` |
| 条件成立 | `wait_until(|| object.get("ready")).await?` |
| 异步操作完成 | `wait_for_operation(operation).await?` |

`spawn` 返回的任务句柄可用 `cancel()` 取消，用 `status()` 查询状态。已有托管 IEnumerator 可通过 `ManagedCoroutine::start` 启动，或用 `start_method` 调用返回 IEnumerator 的方法；调用 `stop()` 停止它。

## 加载字体

在主线程协程中异步加载 AssetBundle，再给文本对象设置字体：

```rust
use cleat::prelude::*;

async fn replace_font(label: GcHandle, bytes: Vec<u8>) -> Result<()> {
    let bundle = AssetBundle::load_from_memory_async(&bytes).await?;
    let font = bundle.load_asset_async("MyFont", "UnityEngine.Font").await?;
    label.object().set("font", font)?;
    bundle.unload(false)
}
```

通过 `runner.spawn(...)` 或 `scheduler.spawn(...)` 执行此任务。`unload(false)` 释放 AssetBundle 容器并保留已加载资源。同步加载可使用 `load_from_file`、`load_from_memory` 和 `load_asset`。

## 注册类

选择[版本表](BUILD.md#unity-版本)中支持类注册的 feature。在 Unity 初始化后的主线程回调中调用 `unsafe { InjectionRuntime::initialize()? }`，再使用 `ClassBuilder` 注册类型：

```rust
use cleat::injection::{ClassBuilder, InjectionRuntime};
use cleat::prelude::*;

fn register_controller(runtime: &InjectionRuntime) -> Result<Class> {
    ClassBuilder::new(
        "Assembly-CSharp",
        "MyMod.Controller",
        &Class::find("UnityEngine.MonoBehaviour")?,
    )
    .field::<f32>("speed")?
    .register(runtime)
}
```

通过 `#[cleat::managed_method("Awake")]` 定义托管方法，再用 `.method(function::definition()?)` 添加。完整的组件注册和 `UnityCoroutine` 用法见 [injection.rs](../../examples/injection.rs)。`UnityCoroutine` 支持 `end_of_frame`、`fixed_update`、`seconds` 和托管等待对象。

`InjectionRuntime::initialize` 要求游戏的 IL2CPP ABI 与所选 feature 一致，模块在注册类型使用期间保持加载。

## 线程和入口

`#[cleat::main]` 在 IL2CPP 初始化后运行，入口位于已附加到运行时的工作线程。Hook 安装和配置读取可以放在入口；场景、UI、AssetBundle 和 Unity 协程放到主线程回调中执行。

自行创建的线程调用托管 API 前，持有 `ThreadGuard::attach()?` 返回的 guard，直到调用完成。Android Java 调用使用 `cleat::android::with_java`，闭包中可使用重新导出的 `jni` API。

## 其他 API

| 操作 | API |
|---|---|
| 程序集与类型关系 | `Assembly`、`Class::parent`、`nested`、`interfaces` |
| 属性与索引器 | `property`、`get_with`、`set_with` |
| 委托 | `Delegate::new`、`invoke`、`combine`、`remove` |
| 事件 | `event`、`add`、`remove` |
| Unity 数学类型 | `Vector2`、`Vector3`、`Quaternion`、`Color`、`Rect` 等 |
| 自定义值类型 | `#[repr(C)]` 与 `#[cleat::value_type("Game.Type")]` |