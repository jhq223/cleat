# 编译指南

## 安装 Rust

```bash
rustup install nightly
rustup default nightly
rustup target add aarch64-linux-android
```

## 安装 Android NDK

| 平台 | 地址 |
|------|------|
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

## 安装 cargo-ndk

```bash
cargo install cargo-ndk
```

## 编译

```bash
cd your-project
cargo ndk -t arm64-v8a -o ./target/jniLibs build --release
```

产物：`target/jniLibs/arm64-v8a/lib<你的 crate 名>.so`

## ShadowHook

```bash
VERSION=2.0.0
curl -L -o shadowhook.aar \
  "https://repo1.maven.org/maven2/com/bytedance/android/shadowhook/${VERSION}/shadowhook-${VERSION}.aar"

unzip -j shadowhook.aar \
  "prefab/modules/shadowhook/libs/android.arm64-v8a/libshadowhook.so" \
  -d ./target/jniLibs/arm64-v8a/
```

APK 内 `lib/arm64-v8a/` 需包含两份 `.so`。
