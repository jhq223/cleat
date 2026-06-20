# cleat

Android IL2CPP 游戏 Modding 工具包。

[English](README.md) | [API](docs/zh/API.md) | [编译](docs/zh/BUILD.md)

## 这是什么

IL2CPP 的 Rust 绑定。提供**类型安全**的字段读写、方法调用、inline hook API，
同时提供 `List<T>` 和 `Dictionary<K,V>` 的**类型化集合封装**，不再需要手动算偏移量。

```rust
use cleat::prelude::*;

#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    // 类型安全的字段访问 — 无需 unsafe
    let klass = Il2CppClass::find("Player")?;
    let hp: i32 = klass.static_field_value("maxHealth")?;

    Ok(())
}

#[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
    god_mode::original(this, 0);
    Ok(())
}

// 类型化 Dictionary 访问 (0.2+) — 不再需要手动算偏移量
fn read_lookup(obj: &Il2CppObject) {
    let dict: Il2CppDictionary<*mut std::ffi::c_void, *mut std::ffi::c_void> =
        obj.load("lookup").unwrap();
    for (key, val) in dict.iter() {
        // ...
    }
}
```

## 开始

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

## 文档

| | |
|---|---|
| [API 参考](docs/zh/API.md) | 完整 API |
| [编译指南](docs/zh/BUILD.md) | 工具链安装和编译 |
| [API Reference](docs/en/API.md) | API (English) |
| [Build Guide](docs/en/BUILD.md) | Build (English) |

## 依赖

- Rust nightly
- Android NDK r26+
- cargo-ndk ≥ 3.0
- libshadowhook.so（运行时，随 APK 打包）

## License

MIT