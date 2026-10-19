# cleat API

本文对应仓库内的 0.3 API。可编译的完整示例位于 [examples](../../examples)，具体签名可用 `cargo doc --no-deps --open` 查看。

## 入口与线程

`#[cleat::main]` 生成 `JNI_OnLoad`，保存 JavaVM，并在游戏完成 IL2CPP 初始化后运行入口。入口位于已附加的工作线程。安装 Hook、加载普通 Rust 配置可以在入口执行；场景、UI、AssetBundle 和 Unity 协程必须在主线程执行，例如目标游戏的 `Update` Hook。

自行创建的线程调用托管 API 前，持有 `ThreadGuard::attach()?`。Android Java 调用使用 `cleat::android::with_java`，它负责线程附加、本地引用帧及异常处理。原始 JNI 功能由重新导出的 `jni` crate 提供。

## 查找、读取与调用

```rust
use cleat::prelude::*;

fn update(player: Object) -> Result<()> {
    let health: i32 = player.get("health")?;
    player.set("health", health + 100)?;
    player.set("displayName", "御主")?;
    player.call::<()>("Refresh", ())?;
    let class = Class::find_in("Assembly-CSharp", "GameManager")?;
    let _manager: Object = class.get("instance")?;
    Ok(())
}
```

`Class::find` 跨程序集查找，`find_in` 指定程序集；同名不明确时返回错误。`Object` 是 `Il2CppObject` 的别名，`Class` 是 `Il2CppClass` 的别名。`get` / `set` 支持字段及属性，`call` 接受参数元组，无参数为 `()`，单参数为 `(value,)`。

指定重载时使用 `object.call::<()>(("SetValue", ["System.String"]), ("文本",))?`。可通过 `method` 缓存句柄，再调用 `invoke`。同参数数量但类型不同的方法不会被随意选中。泛型类与泛型方法使用 `generic`；实例方法可用 `virtual_method` 解析实际虚方法。

`construct` 调用构造函数，`construct_with` 指定构造参数类型。`new_object` 只分配对象，不调用构造函数。`box_value` / `unbox` 用于值类型；`by_ref(&mut value)` 和 `out(&mut value)` 用于调用 ref/out 参数。原始指针和自定义值类型的布局仍由调用者保证。

程序集、继承与接口可通过 `Assembly`、`parent`、`nested`、`interfaces` 访问。`property` 支持索引器；`event` 通过 add/remove 访问器操作；`Delegate` 保持委托对象的 GC 根。

## Hook

```rust
use cleat::prelude::*;

#[cleat::hook("UnityEngine.UI", "UnityEngine.UI.Text", "set_text")]
fn text(this: &Object, value: Il2CppString) -> Result<()> {
    let value = if value.to_string_lossy() == "Start" {
        Il2CppString::try_new("开始")?
    } else { value };
    text::original(this, value)
}
```

在入口调用 `text::install()?`。可省略程序集参数，使用 `#[cleat::hook("Namespace.Type", "Method")]`。原函数返回 `Result`；尚未安装时调用会返回错误。宏保留 IL2CPP 隐藏的 MethodInfo 参数，并校验签名。Hook 内的 panic/错误不会跨越原生 ABI 边界。

`uninstall` 应在目标调用停止后执行；线程已经进入原生替换入口、尚未进入 Rust 回调时仍存在交接窗口。不要在目标方法持续并发调用期间依赖卸载无缝切换。宏暂不支持 ref/out 参数及值类型实例方法的 Hook；这些参数可用于反射调用。

## 集合与对象寿命

`Il2CppArray<T>` 支持带类型检查、边界检查和 GC 写屏障的读取与写入。`Il2CppList<T>` 提供 `get/set/push/remove/clear`；`snapshot()` 返回保留 GC 根的 `RootedArray<T>`，适合跨帧处理。`to_vec()` 中复制的托管对象包装本身不提供根。

`Il2CppDictionary<K,V>` 的 `get/set/contains_key/remove/clear/iter` 调用运行时集合方法，不读取私有 entries/buckets/count 偏移。`get` 返回 `Result<Option<V>>`；迭代元素也是 `Result`，删除过的槽位和遍历时修改均由托管枚举器处理。AOT 裁剪导致相关方法缺失时会明确返回错误。

`Object`、字符串和数组等包装只是托管指针，不阻止 GC。跨回调保存时持有 `object.root()?`，使用根的 `object()` 取回对象。GC 根并不提供线程同步；不要让未固定的对象在被回收后继续使用。

## 跨帧执行与资源

`CoroutineScheduler::spawn` 接受返回 `Result<()>` 的 future；在主线程每帧调用 `tick`。任务内可等待 `coroutine::next_frame()`、`wait_frames`、`wait_seconds`、`wait_realtime`、`wait_until` 和 `wait_for_operation`。保留任务句柄可取消任务、检查状态。调度器会隔离任务失败并防止重入；所有 tick 必须来自同一线程。

`ManagedCoroutine::start` 启动已有的托管 IEnumerator；`start_method` 从方法创建枚举器。需显式调用 `stop` 停止 Unity 协程，丢弃句柄只释放根。

`AssetBundle::load_from_memory_async` 和 `load_asset_async` 通过实际的异步请求等待 `isDone`，期间保留请求及资源所需的根。它们必须由主线程调度器驱动。同步 `load_from_file/load_from_memory/load_asset` 也可用，但部分游戏裁剪掉同步重载时应使用异步 API。

## 可选 Unity 内部结构

一般反射、文本 Hook、运行时集合和 Rust 帧调度不依赖私有类布局。Unity 6 使用 `unity-6000` 选择指针宽度的 GC 句柄 ABI，暂不支持类注入。

新类、接口和虚方法注册，以及 Rust 实现的 Unity IEnumerator，需要选择一个互斥的布局 feature：

| Feature | 布局范围 |
|---|---|
| `unity-2019-4` | Unity 2019.4 |
| `unity-2020-3` | Unity 2020.3，补丁 20 及以上 |
| `unity-2021-3` | Unity 2021.3 |
| `unity-2022-3` | Unity 2022.3 |
| `unity-2023-1` | Unity 2023.1 |

在安全的初始化时机调用 `unsafe { InjectionRuntime::initialize()? }`，通过 `ClassBuilder` 注册类型；示例见 [injection.rs](../../examples/injection.rs)。实现会检查版本和关键布局；编译成功不能代替目标游戏验证。

BNM 作为功能与 ABI 研究参考，未复制或编译其实现。现有类布局修改、新泛型类注册、Unity 6 类注入尚未提供；泛型共享/AOT 裁剪及游戏定制运行时仍需按目标验证。