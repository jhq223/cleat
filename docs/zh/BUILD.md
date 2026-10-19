# 构建

## 环境

安装稳定版 Rust、Android NDK 和 cargo-ndk：

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk --locked
```

通过 Android SDK Manager 安装 NDK，并设置 `ANDROID_NDK_HOME`。例如 PowerShell：

```powershell
$env:ANDROID_NDK_HOME = 'D:\Android\Sdk\ndk\27.0.12077973'
```

## 创建模块

将 cleat 仓库和模块放在同一目录下：

```text
workspace/
├── cleat/
└── my-mod/
    ├── Cargo.toml
    └── src/lib.rs
```

`my-mod/Cargo.toml`：

```toml
[package]
name = "my-mod"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
cleat = { path = "../cleat" }
```

将 [README 中的文本替换示例](../../README_zh.md#从文本替换开始)保存为 `src/lib.rs`，在模块目录执行：

```sh
cargo ndk -t arm64-v8a -o target/jniLibs build --release
```

生成文件为 `target/jniLibs/arm64-v8a/libmy_mod.so`。放入 APK 的 `lib/arm64-v8a/`，在应用启动时调用 `System.loadLibrary("my_mod")`。

ABI 与游戏原生库保持一致。模拟器通过 ARM 转译运行游戏时，也选择游戏的 ARM ABI。

| Android ABI | Rust target |
|---|---|
| `arm64-v8a` | `aarch64-linux-android` |
| `armeabi-v7a` | `armv7-linux-androideabi` |
| `x86` | `i686-linux-android` |
| `x86_64` | `x86_64-linux-android` |

构建其他 ABI 前，使用 `rustup target add <target>` 安装对应目标。

## Unity 版本

在依赖中选择游戏对应的 feature，每次启用一个：

```toml
cleat = { path = "../cleat", features = ["unity-2022-3"] }
```

| Feature | Unity 版本 | 功能 |
|---|---|---|
| 默认 | Unity 2019–2022 | 反射、Hook、集合、Rust 帧调度和托管协程 |
| `unity-2019-4` | 2019.4 | 默认功能、类注册、Rust IEnumerator |
| `unity-2020-3` | 2020.3.20 及后续补丁 | 同上 |
| `unity-2021-3` | 2021.3 | 同上 |
| `unity-2022-3` | 2022.3 | 同上 |
| `unity-2023-1` | 2023.1 | 同上，使用对应 GC 句柄 ABI |
| `unity-6000` | Unity 6 | 反射、Hook、集合、Rust 帧调度和托管协程 |

类注册示例见 [injection.rs](../../examples/injection.rs)。

## 离线构建

首次 Android 构建会下载并静态链接 Frida Gum 17.24.3。离线使用时，先准备对应 ABI 的官方 devkit，并让 `CLEAT_GUM_DEVKIT` 指向包含 `frida-gum.h` 和 `libfrida-gum.a` 的目录：

```powershell
$env:CLEAT_GUM_DEVKIT = 'D:\devkits\frida-17.24.3-android-arm64'
cargo fetch
cargo ndk -t arm64-v8a build --release --offline
```

切换 ABI 时，同时切换 devkit 目录。

## 开发检查

在 cleat 仓库执行：

```sh
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy --workspace --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example translation
```