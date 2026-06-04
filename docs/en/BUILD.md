# Build Guide

## Install Rust

```bash
rustup install nightly
rustup default nightly
rustup target add aarch64-linux-android
```

## Install Android NDK

| Platform | URL |
|----------|-----|
| Windows | https://dl.google.com/android/repository/android-ndk-r29-windows.zip |
| macOS | https://dl.google.com/android/repository/android-ndk-r29-darwin.dmg |
| Linux | https://dl.google.com/android/repository/android-ndk-r29-linux.zip |

```bash
# Linux / macOS
unzip android-ndk-r29-linux.zip -d ~/Android/Sdk/ndk/
export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/android-ndk-r29"

# Windows (PowerShell)
Expand-Archive android-ndk-r29-windows.zip -DestinationPath "$env:LOCALAPPDATA\Android\Sdk\ndk"
$env:ANDROID_NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\android-ndk-r29"
```

## Install cargo-ndk

```bash
cargo install cargo-ndk
```

## Build

```bash
cd your-project
cargo ndk -t arm64-v8a -o ./target/jniLibs build --release
```

Output: `target/jniLibs/arm64-v8a/lib<your_crate>.so`

## ShadowHook

```bash
VERSION=2.0.0
curl -L -o shadowhook.aar \
  "https://repo1.maven.org/maven2/com/bytedance/android/shadowhook/${VERSION}/shadowhook-${VERSION}.aar"

unzip -j shadowhook.aar \
  "prefab/modules/shadowhook/libs/android.arm64-v8a/libshadowhook.so" \
  -d ./target/jniLibs/arm64-v8a/
```

APK 内 `lib/arm64-v8a/` should contain both `.so` files.
