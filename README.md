# cleat

Android IL2CPP game modding toolkit.

[中文](README_zh.md) | [API](docs/en/API.md) | [Build](docs/en/BUILD.md)

## What

Rust bindings over IL2CPP. You get a **safe** API for field access, method
calls, and inline hooks, plus **typed collection wrappers** for `List<T>` and
`Dictionary<K,V>` so you never need to hand-compute memory offsets.

```rust
use cleat::prelude::*;

#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    // Type-safe field access — no unsafe needed.
    let klass = Il2CppClass::find("Player")?;
    let hp: i32 = klass.static_field_value("maxHealth")?;

    Ok(())
}

#[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
    god_mode::original(this, 0);
    Ok(())
}

// Typed Dictionary access (0.2+) — no more manual offset math.
fn read_lookup(obj: &Il2CppObject) {
    let dict: Il2CppDictionary<*mut std::ffi::c_void, *mut std::ffi::c_void> =
        obj.load("lookup").unwrap();
    for (key, val) in dict.iter() {
        // ...
    }
}
```

## Setup

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
cleat = "0.2"
```

```rust
// src/lib.rs
use cleat::prelude::*;

#[cleat::main]
fn main() -> cleat::Result<()> {
    let klass = Il2CppClass::find("UnityEngine.Time")?;
    let time: f32 = klass.static_field_value("timeScale")?;
    log::info!("timeScale = {}", time);

    Ok(())
}
```

```bash
cargo ndk -t arm64-v8a -o ./target/jniLibs build --release
```

## Docs

| | |
|---|---|
| [API Reference](docs/en/API.md) | Full API |
| [Build Guide](docs/en/BUILD.md) | Toolchain setup and compilation |
| [API 参考](docs/zh/API.md) | API (Chinese) |
| [编译指南](docs/zh/BUILD.md) | Build (Chinese) |

## Requires

- Rust nightly
- Android NDK r26+
- cargo-ndk ≥ 3.0
- libshadowhook.so (runtime, packed in APK)

## License

MIT