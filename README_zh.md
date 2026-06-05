# cleat

Android IL2CPP 游戏 Modding 工具包。

[English](README.md) | [API](docs/zh/API.md) | [编译](docs/zh/BUILD.md)

## 这是什么

IL2CPP 的 Rust 绑定。提供**类型安全**的字段读写、方法调用、inline hook API，
同时为需要访问私有字段 / 未暴露内部结构的场景提供**底层偏移量读写逃生舱**。
危险的指针运算被封装在命名清晰的函数中——你只需要提供偏移量。

```rust
use cleat::prelude::*;

#[cleat::main]
fn my_mod() -> cleat::Result<()> {
    cleat::set_app_data("/data/data/com.example.game/files");

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

// 底层偏移量访问 — 用于私有字段 / 未暴露类型
fn patch_dictionary(obj: &Il2CppObject) {
    let entries = unsafe { obj.read_ptr_at(0x18) }.unwrap();  // Dictionary.entries
    let count   = unsafe { obj.read_i32_at(0x20) };           // Dictionary.count
    // ...
}
```

## 开始

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
cleat = "0.1"
```

```rust
// src/lib.rs
use cleat::prelude::*;

#[cleat::main]
fn main() -> cleat::Result<()> {
    cleat::set_app_data("/data/data/com.example.game/files");

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
