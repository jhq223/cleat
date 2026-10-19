# API

[中文](../zh/API.md) · [Building](BUILD.md) · [Examples](../../examples)

Replace names such as `Game.Player` and `health` with types and members from your game. Run `cargo doc --no-deps --open` for full signatures.

## Read and change objects

```rust
use cleat::prelude::*;

fn update_player(player: Object) -> Result<()> {
    let hp: i32 = player.get("health")?;
    player.set("health", hp + 100)?;
    player.set("displayName", "Master")?;
    player.call::<()>("Refresh", ())?;
    Ok(())
}
```

`get` and `set` accept fields and properties. Pass Rust strings directly to `set` and `call`. Use Rust return types such as `i32`, `f32`, `bool`, `Il2CppString` or `Object`; C# `void` corresponds to `()`.

Access static members through `Class`:

```rust
use cleat::prelude::*;

fn manager() -> Result<Object> {
    Class::find_in("Assembly-CSharp", "Game.GameManager")?.get("instance")
}
```

`Class::find("Game.GameManager")` searches all assemblies. Use `find_in` to select an assembly when a name is ambiguous.

## Call methods

Arguments go in tuples: `()` for none, `(value,)` for one. Select overloads by their C# parameter types:

```rust
use cleat::prelude::*;

fn set_title(label: Object) -> Result<()> {
    label.call::<()>(("SetText", ["System.String"]), ("Start Game",))
}
```

For repeated calls, retain the `MethodInfo` from `object.method(...)` and call `invoke`. `method.virtual_method()` resolves the bound object's virtual implementation.

Use `by_ref` and `out` for reference parameters:

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

## Construction and generics

`construct` allocates an object and calls its constructor. Use `construct_with` to select a constructor overload. For example, create `List<string>`:

```rust
use cleat::prelude::*;

fn string_list() -> Result<Object> {
    let string = Class::find("System.String")?;
    let list = Class::find("System.Collections.Generic.List`1")?.generic(&[&string])?;
    list.construct(())
}
```

Generic methods use `method.generic(&[&class])`. Use `Class::box_value` and `Object::unbox` for boxing and unboxing value types.

## Replace text

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

The macro takes an assembly, type and method name. The assembly is optional: `#[cleat::hook("Game.Player", "Start")]`. `text::original` calls the original method, and `text::install` installs the hook. Call `text::uninstall()` after calls to the target have stopped.

Match the target's parameter and return types. The macro accepts ordinary parameters, static methods and reference-type instance methods. Receive the instance as `this: &Object`.

## Collections

Read arrays, lists and dictionaries from managed fields or method results:

```rust
use cleat::prelude::*;

fn read_items(inventory: Object) -> Result<Vec<(i32, i32)>> {
    let counts: Il2CppDictionary<i32, i32> = inventory.get("itemCounts")?;
    let count = counts.get(1001)?.unwrap_or(0);
    counts.set(1001, count + 1)?;
    counts.iter()?.collect()
}
```

| Type | Common operations |
|---|---|
| `Il2CppArray<T>` | `len`, `get`, `set`, `iter`, `from_slice` |
| `Il2CppList<T>` | `len`, `get`, `set`, `push`, `remove`, `clear`, `snapshot` |
| `Il2CppDictionary<K, V>` | `len`, `get`, `set`, `remove`, `iter` |

Dictionary `get` returns `Result<Option<V>>`, and each iterator entry is a `Result`. Use a list's `snapshot()` for traversal across frames; its `RootedArray<T>` keeps the array and its elements alive.

## Object lifetimes

When storing an object across callbacks, retain its GC root and access the object through `root.object()`:

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

Dropping the root ends its hold on the object. Use `GcHandle::pinned` when native code retains an object's address; `object.root()` already creates a pinned root.

## Coroutines

From a Unity main-thread callback, install a `CoroutineRunner` on a MonoBehaviour with an `Update` method:

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

Retain the runner to execute tasks after that object's `Update`. Dropping it cancels tasks and uninstalls the hook; drop it after the object has stopped updating. If you already have an Update hook, use `CoroutineScheduler` and call `tick()` once per frame.

| Wait for | Usage |
|---|---|
| Next frame | `next_frame().await` |
| A number of frames | `wait_frames(10).await` |
| Game time | `wait_seconds(0.5).await?` |
| Real time | `wait_realtime(Duration::from_secs(1)).await` |
| A condition | `wait_until(|| object.get("ready")).await?` |
| An asynchronous operation | `wait_for_operation(operation).await?` |

The task handle returned by `spawn` provides `cancel()` and `status()`. Start an existing managed IEnumerator with `ManagedCoroutine::start`, or call an IEnumerator-returning method with `start_method`. Call `stop()` to stop it.

## Load a font

Load an AssetBundle in a main-thread coroutine, then assign the font to a text object:

```rust
use cleat::prelude::*;

async fn replace_font(label: GcHandle, bytes: Vec<u8>) -> Result<()> {
    let bundle = AssetBundle::load_from_memory_async(&bytes).await?;
    let font = bundle.load_asset_async("MyFont", "UnityEngine.Font").await?;
    label.object().set("font", font)?;
    bundle.unload(false)
}
```

Run this task with `runner.spawn(...)` or `scheduler.spawn(...)`. `unload(false)` releases the bundle container while retaining loaded assets. Synchronous loading uses `load_from_file`, `load_from_memory` and `load_asset`.

## Register classes

Choose a feature with class registration from the [version table](BUILD.md#unity-version). Call `unsafe { InjectionRuntime::initialize()? }` on Unity's main thread after initialization, then register types with `ClassBuilder`:

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

Define managed methods with `#[cleat::managed_method("Awake")]` and add them with `.method(function::definition()?)`. See [injection.rs](../../examples/injection.rs) for component registration and `UnityCoroutine`. Its yield operations include `end_of_frame`, `fixed_update`, `seconds` and existing managed yield objects.

`InjectionRuntime::initialize` requires the game's IL2CPP ABI to match the selected feature. Keep the module loaded while its registered types are in use.

## Entry point and threads

`#[cleat::main]` runs after IL2CPP initialization on an attached worker thread. Install hooks and read configuration there. Run scene, UI, AssetBundle and Unity coroutine operations in a main-thread callback.

On threads you create, retain the guard from `ThreadGuard::attach()?` for the duration of managed calls. For Android Java calls, use `cleat::android::with_java`; its closure provides access to the re-exported `jni` API.

## More APIs

| Operation | API |
|---|---|
| Assemblies and type relationships | `Assembly`, `Class::parent`, `nested`, `interfaces` |
| Properties and indexers | `property`, `get_with`, `set_with` |
| Delegates | `Delegate::new`, `invoke`, `combine`, `remove` |
| Events | `event`, `add`, `remove` |
| Unity math types | `Vector2`, `Vector3`, `Quaternion`, `Color`, `Rect` and others |
| Custom value types | `#[repr(C)]` and `#[cleat::value_type("Game.Type")]` |