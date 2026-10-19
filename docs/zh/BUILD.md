# 构建 cleat 模块

使用稳定版 Rust（Edition 2024）、Android NDK 和 cargo-ndk。当前验证环境为 Rust 1.98.1、cargo-ndk 4.1.2、NDK 27.0.12077973。通过 Android SDK Manager 安装 NDK 后设置 `ANDROID_NDK_HOME`。

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
cargo ndk -t arm64-v8a -o target/jniLibs build --release
```

PowerShell 示例：

```powershell
$env:ANDROID_NDK_HOME = 'D:\Android\Sdk\ndk\27.0.12077973'
cargo ndk -t arm64-v8a build --release
```

ABI 选择游戏自身原生库的 ABI。MuMu 以 Houdini 运行 ARM64 游戏时，模块也构建为 ARM64；原生 x86_64 游戏使用 x86_64。

## Frida Gum

构建脚本下载 Frida Gum 17.24.3 官方 Android devkit，核对固定 SHA-256 后解压并静态链接。项目没有 ShadowHook 依赖，也不需要在 APK 中额外放入 `libshadowhook.so` 或 `libfrida-gum.so`。

离线构建可先下载对应 ABI 的官方 devkit，解压后设置 `CLEAT_GUM_DEVKIT`，目录应直接包含 `frida-gum.h` 与 `libfrida-gum.a`。手动提供的文件必须来自可信来源，且版本与 ABI 匹配；该覆盖路径不会下载或核对归档散列。

```powershell
$env:CLEAT_GUM_DEVKIT = 'D:\devkits\frida-17.24.3-android-arm64'
cargo ndk -t arm64-v8a build --release --offline
```

该变量一次只对应一个 ABI。多 ABI 构建应使用自动下载，或逐个设置后分别构建。Gum 是官方原生依赖，cleat 的接入与版本布局代码使用 Rust。

## 使用与检查

Mod 项目将 `cleat = { path = "../cleat" }` 作为依赖，并设置 `crate-type = ["cdylib"]`。Unity 6 使用 `features = ["unity-6000"]`。其他版本的可选内部功能参见 [API 文档](API.md)。禁止同时启用多个 Unity feature，也不要使用 `--all-features`。

```sh
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy --workspace --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example translation
```

Windows 的本地化 MSVC 链接器可能把“正在创建库”作为输出，Rust 将其展示为 linker_messages 警告。可为主机检查使用 Rust 随附的 LLD，不需要压制 lint：

```powershell
$sysroot = rustc --print sysroot
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $sysroot 'lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe'
cargo test --workspace
```

主机测试检查参数 ABI、集合语义、调度器与错误路径。Android 交叉编译检查目标链接；运行时 Hook、字体加载和类注册需要在对应 Unity 游戏中验证。将生成库打入 APK 后通过 `System.loadLibrary` 加载；`#[cleat::main]` 负责等待 IL2CPP 就绪。

内部 bridge 位于 `crates/il2cpp-bridge-rs`，随本仓库维护。构建不拉取上游或某个分支；首次依赖下载后可使用 Cargo 离线模式。