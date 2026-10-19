# Build a cleat module

Use stable Rust with Edition 2024, Android NDK and cargo-ndk. The current validation environment uses Rust 1.98.1, cargo-ndk 4.1.2 and NDK 27.0.12077973. Install the NDK through Android SDK Manager and set `ANDROID_NDK_HOME`.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
cargo ndk -t arm64-v8a -o target/jniLibs build --release
```

PowerShell example:

```powershell
$env:ANDROID_NDK_HOME = 'D:\Android\Sdk\ndk\27.0.12077973'
cargo ndk -t arm64-v8a build --release
```

Match the game's native ABI. A MuMu instance running an ARM64 game through Houdini needs the ARM64 module, while a native x86_64 game needs x86_64.

## Frida Gum

The build downloads the official Frida Gum 17.24.3 Android devkit, verifies a pinned SHA-256 and links it statically. No ShadowHook dependency or separate libshadowhook.so/libfrida-gum.so is needed in the APK.

For offline builds, extract the matching official devkit and set `CLEAT_GUM_DEVKIT` to the directory directly containing `frida-gum.h` and `libfrida-gum.a`. This override must contain trusted files of the correct version and ABI; it does not download or verify an archive hash.

```powershell
$env:CLEAT_GUM_DEVKIT = 'D:\devkits\frida-17.24.3-android-arm64'
cargo ndk -t arm64-v8a build --release --offline
```

The override describes one ABI at a time. Use automatic downloads for multi-ABI builds or change the override between separate builds. Gum is a native dependency; the project's integration and layout code are Rust.

## Modules and checks

Use `cleat = { path = "../cleat" }` and `crate-type = ["cdylib"]`. Unity 6 needs `features = ["unity-6000"]`. See the [API guide](API.md) for optional internal profiles. Unity features are mutually exclusive; do not use `--all-features`.

```sh
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy --workspace --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example translation
```

Localized MSVC linker progress messages may be reported by Rust as linker_messages warnings. Host checks can use Rust's bundled LLD without suppressing lints:

```powershell
$sysroot = rustc --print sysroot
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $sysroot 'lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe'
cargo test --workspace
```

Host tests cover argument ABI, collections, scheduling and failures. Cross-compilation checks Android linking. Hooks, asset loading and registration still need testing in the matching Unity runtime. Load the packaged module through `System.loadLibrary`; the entry macro waits for IL2CPP readiness.

The private bridge at `crates/il2cpp-bridge-rs` is maintained in this repository. Builds do not track an upstream Git branch. After fetching Cargo dependencies, use offline mode as needed.