# cleat

[中文](README_zh.md) · [API guide](docs/en/API.md) · [Build guide](docs/en/BUILD.md) · [Modification history](CHANGELOG.md)

cleat is a Rust toolkit for Android Unity IL2CPP mods. Read and change managed
fields and properties, call methods, replace text and fonts, hook native methods,
and run work over multiple frames.

The project implementation is Rust. Android hooks link the official Frida Gum
native library for ARM, ARM64, x86 and x86_64. BNM is a reference for capabilities
and ABI research; its code is not bundled.

## Start with a translation hook

Check out this repository next to your mod, then use a local dependency:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
cleat = { path = "../cleat" }
```

```rust
use cleat::prelude::*;

#[cleat::main]
fn start() -> Result<()> {
    translate::install()
}

#[cleat::hook("UnityEngine.UI", "UnityEngine.UI.Text", "set_text")]
fn translate(this: &Object, text: Il2CppString) -> Result<()> {
    let translated = match text.to_string_lossy().as_str() {
        "Start Game" => Il2CppString::try_new("开始游戏")?,
        "Settings" => Il2CppString::try_new("设置")?,
        _ => text,
    };
    translate::original(this, translated)
}
```

Build with `cargo ndk -t arm64-v8a build --release`, then load the resulting
library through Android's `System.loadLibrary`. Choose the ABI of the game's
native libraries, including when the game runs through an emulator's ARM
translation layer. The [translation example](examples/translation.rs) is a
buildable library. This repository's 0.3 API is not yet a published crate.

## Everyday operations

```rust
use cleat::prelude::*;
fn example(player: Object) -> Result<()> {
let hp: i32 = player.get("health")?;
player.set("health", hp + 100)?;
player.set("displayName", "御主")?;
player.call::<()>("Refresh", ())?;

let game = Class::find_in("Assembly-CSharp", "GameManager")?;
let manager: Object = game.get("instance")?;
let _retained = manager.root()?; // keep this root while storing the object
Ok(())
}
```

`get` and `set` accept fields or properties. Calls use argument tuples; overloads
with the same argument count require explicit parameter types. Failures return
`Result` rather than silently choosing an overload.

## Capabilities

| Area | APIs |
|---|---|
| Reflection | Assemblies, classes, nested classes, fields, properties, methods, interfaces and hierarchy |
| Managed operations | Construction, boxing/unboxing, generic specialization, virtual dispatch, delegates and event accessors |
| Collections | Arrays, lists and dictionaries; checked writes and runtime-backed dictionary enumeration |
| Translation/resources | Rust-string inputs, UTF-16 access, text hooks, AssetBundle/font loading |
| Hooks | Frida Gum replacement, original calls, installation/uninstallation |
| Coroutines | Rust frame scheduler and existing managed IEnumerator support |
| Optional Unity internals | Registered classes/fields/methods/interfaces, virtual overrides, Rust IEnumerator and Unity yield phases |

The [API guide](docs/en/API.md) describes remaining differences from BNM.
Class registration and native IEnumerator support require exactly one Unity
feature: `unity-2019-4`, `unity-2020-3` (patch 20+), `unity-2021-3`,
`unity-2022-3` or `unity-2023-1`. These are internal ABI profiles, not a promise
that every game build has the same native call patterns. Unity 2023 also needs
its profile for the changed GC handle ABI. Unity 6 uses `unity-6000` for public
APIs and GC handles; class injection is not yet available for that version.

## Runtime requirements

`#[cleat::main]` runs after IL2CPP initialization on an attached worker thread.
Install hooks and load ordinary Rust data there. Use a Unity main-thread callback
for scene objects, UI, AssetBundles and coroutine startup.

Managed wrappers are unrooted pointers. Keep `GcHandle`/an object's `root()`
when retaining an object across callbacks or allocations without another managed
owner. Rooting does not make engine APIs thread-safe. Custom value layouts and
raw pointers remain the caller's responsibility.

Gum smoke tests have passed on MuMu Android 15 in native x86_64 and ARM64 via
Houdini, including concurrent calls and 20 uninstall/reinstall cycles. This
checks the hook backend; version-specific class injection needs separate
in-game verification.

## Development

```sh
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy -p cleat -p cleat-macros --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example injection --features unity-2022-3
```

Do not use `--all-features`: Unity profiles are mutually exclusive. The bridge is
maintained in [crates/il2cpp-bridge-rs](crates/il2cpp-bridge-rs/UPSTREAM.md); builds
do not depend on an upstream Git branch. See [third-party notices](THIRD_PARTY_NOTICES.md).
