# Building

## Prerequisites

Install stable Rust, Android NDK and cargo-ndk:

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk --locked
```

Install the NDK through Android SDK Manager and set `ANDROID_NDK_HOME`. For example, in PowerShell:

```powershell
$env:ANDROID_NDK_HOME = 'D:\Android\Sdk\ndk\27.0.12077973'
```

## Create a module

Place the cleat repository next to your module:

```text
workspace/
├── cleat/
└── my-mod/
    ├── Cargo.toml
    └── src/lib.rs
```

In `my-mod/Cargo.toml`:

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

Save the [translation example from the README](../../README.md#start-with-a-translation-hook) as `src/lib.rs`, then build from the module directory:

```sh
cargo ndk -t arm64-v8a -o target/jniLibs build --release
```

Copy `target/jniLibs/arm64-v8a/libmy_mod.so` to `lib/arm64-v8a/` in the APK. Load it at application startup with `System.loadLibrary("my_mod")`.

Match the ABI of the game's native libraries, including when an emulator runs the game through ARM translation.

| Android ABI | Rust target |
|---|---|
| `arm64-v8a` | `aarch64-linux-android` |
| `armeabi-v7a` | `armv7-linux-androideabi` |
| `x86` | `i686-linux-android` |
| `x86_64` | `x86_64-linux-android` |

Install each target with `rustup target add <target>` before building it.

## Unity version

Select one feature matching the game's Unity version:

```toml
cleat = { path = "../cleat", features = ["unity-2022-3"] }
```

| Feature | Unity version | APIs |
|---|---|---|
| Default | Unity 2019–2022 | Reflection, hooks, collections, Rust frame scheduling and managed coroutines |
| `unity-2019-4` | 2019.4 | Default APIs, class registration and Rust IEnumerator |
| `unity-2020-3` | 2020.3.20 and later patches | Same as above |
| `unity-2021-3` | 2021.3 | Same as above |
| `unity-2022-3` | 2022.3 | Same as above |
| `unity-2023-1` | 2023.1 | Same as above, with the corresponding GC handle ABI |
| `unity-6000` | Unity 6 | Reflection, hooks, collections, Rust frame scheduling and managed coroutines |

See [injection.rs](../../examples/injection.rs) for class registration.

## Offline builds

The first Android build downloads and statically links Frida Gum 17.24.3. To build offline, extract the official devkit for your ABI and set `CLEAT_GUM_DEVKIT` to the directory containing `frida-gum.h` and `libfrida-gum.a`:

```powershell
$env:CLEAT_GUM_DEVKIT = 'D:\devkits\frida-17.24.3-android-arm64'
cargo fetch
cargo ndk -t arm64-v8a build --release --offline
```

Change the devkit directory when switching ABIs.

## Development checks

Run these commands in the cleat repository:

```sh
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy --workspace --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example translation
```