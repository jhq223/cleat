# cleat API

This guide describes the repository's 0.3 API. See [examples](../../examples) for complete programs and run `cargo doc --no-deps --open` for signatures.

## Entry point and threads

`#[cleat::main]` exports `JNI_OnLoad`, retains the JavaVM and waits for the game's IL2CPP initialization. The entry runs on an attached worker thread. Install hooks and read Rust configuration there. Access scenes, UI, AssetBundles and Unity coroutines from a Unity main-thread callback, such as a game's Update hook.

Threads created by your code must retain `ThreadGuard::attach()?` before managed calls. On Android, `cleat::android::with_java` manages attachment, local reference frames and exceptions using the re-exported jni crate.

## Reflection and calls

```rust
use cleat::prelude::*;

fn update(player: Object) -> Result<()> {
    let health: i32 = player.get("health")?;
    player.set("health", health + 100)?;
    player.set("displayName", "Master")?;
    player.call::<()>("Refresh", ())?;
    let game = Class::find_in("Assembly-CSharp", "GameManager")?;
    let _manager: Object = game.get("instance")?;
    Ok(())
}
```

`Class::find` searches assemblies; `find_in` selects an assembly. Ambiguous names return errors. `Class` and `Object` alias `Il2CppClass` and `Il2CppObject`. `get/set` accept fields or properties. Calls take tuples: `()` or `(value,)` for zero or one argument.

Select an overload with `object.call::<()>(("SetValue", ["System.String"]), ("text",))?`. Cache a method through `method`, then use `invoke`. Same-arity overloads are not arbitrarily selected. `generic` specializes classes or methods; `virtual_method` resolves an instance's virtual implementation.

`construct` calls a constructor; `construct_with` selects parameter types. `new_object` only allocates. Use `box_value/unbox` for values and `by_ref(&mut value)` / `out(&mut value)` for ref/out calls. Custom value layouts and raw pointers remain the caller's responsibility.

`Assembly`, `parent`, `nested` and `interfaces` expose metadata. `property` supports indexers; `event` uses add/remove accessors; `Delegate` retains the delegate's GC root.

## Hooks

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

Call `text::install()?` from the entry. The assembly argument is optional. `original` returns an error before installation. The macro validates signatures, preserves the hidden IL2CPP MethodInfo argument and contains panics/errors at the native boundary.

Uninstall only after calls to the target have stopped. A native-to-Rust entry window remains outside the invocation snapshot; do not rely on seamless concurrent uninstallation. The hook macro does not support ref/out parameters or value-type instance methods, although reflection calls support ref/out arguments.

## Collections and lifetimes

`Il2CppArray<T>` checks bounds and types and uses GC write barriers. `Il2CppList<T>` provides get/set/push/remove/clear. `snapshot()` returns `RootedArray<T>` for work spanning frames; managed handles copied into `to_vec()` are not independently rooted.

`Il2CppDictionary<K,V>` uses managed collection methods rather than private buckets/entries/count offsets. `get` returns `Result<Option<V>>`; `iter` yields fallible entries and honors managed enumerator behavior for deleted slots and concurrent modification. Missing AOT methods produce errors.

Object, string and array wrappers do not prevent collection. Retain `object.root()?` when storing objects across callbacks and retrieve them through the root's `object()`. GC roots do not synchronize object contents.

## Coroutines and assets

`CoroutineScheduler::spawn` accepts a future returning `Result<()>`. Call `tick` once per main-thread frame. Tasks can await `coroutine::next_frame`, `wait_frames`, `wait_seconds`, `wait_realtime`, `wait_until` and `wait_for_operation`. Handles expose cancellation and status. Task failures are isolated; reentrant ticks are rejected, and all ticks must use the same thread.

`ManagedCoroutine::start` runs an existing managed IEnumerator; `start_method` invokes a method to create one. Explicitly call `stop` to stop Unity's coroutine; dropping the handle only releases roots.

`AssetBundle::load_from_memory_async` and `load_asset_async` poll real requests until completion while retaining necessary roots. Drive them with the main-thread scheduler. Synchronous load_from_file/load_from_memory/load_asset methods are also available where the game has not stripped those overloads.

## Unity internals and BNM coverage

General reflection, text hooks, runtime collections and Rust scheduling do not depend on private class layouts. Unity 6 requires `unity-6000` for pointer-sized GC handles and currently has no class-injection profile.

New class/interface/virtual-method registration and Rust Unity IEnumerator generators require exactly one feature: `unity-2019-4`, `unity-2020-3` (patch 20+), `unity-2021-3`, `unity-2022-3` or `unity-2023-1`. Call `unsafe { InjectionRuntime::initialize()? }` at a suitable initialization point, then register through `ClassBuilder`; see [injection.rs](../../examples/injection.rs). Runtime version/layout checks do not replace testing in the target game.

BNM is a capability and ABI reference; its implementation is not copied or compiled. Existing-class layout mutation, new generic class registration and Unity 6 class injection are not provided. Generic sharing, AOT stripping and customized runtimes require target-specific verification.