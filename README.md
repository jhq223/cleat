# cleat

[中文](README_zh.md) · [API guide](docs/en/API.md) · [Build guide](docs/en/BUILD.md) · [Modification history](CHANGELOG.md)

cleat is a Rust toolkit for Android Unity IL2CPP mods. Read and change managed
fields and properties, call methods, replace text and fonts, hook native methods,
and run work over multiple frames.

Hooks use Frida Gum on Android ARM, ARM64, x86 and x86_64.

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
buildable library.

## Everyday operations

```rust
use cleat::prelude::*;
fn update_player(player: Object) -> Result<()> {
    let hp: i32 = player.get("health")?;
    player.set("health", hp + 100)?;
    player.set("displayName", "Master")?;
    player.call::<()>("Refresh", ())?;

    let game = Class::find_in("Assembly-CSharp", "GameManager")?;
    let manager: Object = game.get("instance")?;
    manager.call::<()>("Save", ())?;
    Ok(())
}
```

`get` and `set` accept fields or properties. Calls use argument tuples; overloads
with the same argument count can be selected by parameter type. See the
[API guide](docs/en/API.md) for overloads, collections, coroutines and fonts.

## Capabilities

| Area | APIs |
|---|---|
| Reflection | Assemblies, classes, nested classes, fields, properties, methods, interfaces and hierarchy |
| Managed operations | Construction, boxing/unboxing, generic specialization, virtual dispatch, delegates and event accessors |
| Collections | Arrays, lists and dictionaries; checked writes and runtime-backed dictionary enumeration |
| Translation/resources | Rust-string inputs, UTF-16 access, text hooks, AssetBundle/font loading |
| Hooks | Frida Gum replacement, original calls, installation/uninstallation |
| Coroutines | Rust frame scheduler and existing managed IEnumerator support |
| Class registration | New classes, fields, methods, interfaces, virtual overrides and Rust IEnumerator |

Choose the game's Unity version using the [feature table](docs/en/BUILD.md#unity-version).

## Runtime requirements

`#[cleat::main]` runs after IL2CPP initialization on an attached worker thread.
Install hooks and load ordinary Rust data there. Use a Unity main-thread callback
for scene objects, UI, AssetBundles and coroutine startup.

Keep `object.root()?` alongside objects stored across callbacks. The root keeps
the managed object alive until it is dropped. See [object lifetimes](docs/en/API.md#object-lifetimes).

## Development

```sh
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy -p cleat -p cleat-macros --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example injection --features unity-2022-3
```

Select one Unity feature per build. See the [build guide](docs/en/BUILD.md) for
environment setup and offline builds.

## License

MIT. See [third-party notices](THIRD_PARTY_NOTICES.md) for dependencies.
