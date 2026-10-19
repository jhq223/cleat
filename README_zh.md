# cleat

[English](README.md) · [API 文档](docs/zh/API.md) · [构建说明](docs/zh/BUILD.md) · [修改历史](CHANGELOG.md)

cleat 是面向 Android Unity IL2CPP 游戏的 Rust 修改工具库，可以读取和修改字段、
访问属性、调用方法、替换文本和字体、安装 Hook，以及编写跨帧执行的逻辑。

Hook 使用 Frida Gum，支持 Android ARM、ARM64、x86 和 x86_64。

## 从文本替换开始

将此仓库放在 Mod 项目旁边，通过本地路径引用：

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

运行 `cargo ndk -t arm64-v8a build --release`，再通过 Android 的
`System.loadLibrary` 加载生成的库。ABI 应与游戏原生库一致：模拟器运行 ARM64
游戏时也使用 ARM64 构建。[translation 示例](examples/translation.rs) 可以直接编译。

## 常用操作

```rust
use cleat::prelude::*;

fn update_player(player: Object) -> Result<()> {
    let hp: i32 = player.get("health")?;
    player.set("health", hp + 100)?;
    player.set("displayName", "御主")?;
    player.call::<()>("Refresh", ())?;

    let game = Class::find_in("Assembly-CSharp", "GameManager")?;
    let manager: Object = game.get("instance")?;
    manager.call::<()>("Save", ())?;
    Ok(())
}
```

`get`、`set` 接受字段名或属性名，`call` 使用元组传参。方法重载可以按参数类型选择。
重载、集合、协程和字体的用法见 [API 文档](docs/zh/API.md)。

## 功能

| 范围 | 支持内容 |
|---|---|
| 反射 | 程序集、类、嵌套类、字段、属性、方法、接口与继承关系 |
| 托管操作 | 构造对象、装箱/拆箱、泛型实例化、虚调用、委托与事件访问器 |
| 集合 | 数组、列表、字典；检查类型与边界，通过运行时遍历字典 |
| 汉化与资源 | 直接传入 Rust 字符串、UTF-16 访问、文本 Hook、AssetBundle/字体加载 |
| Hook | Frida Gum 替换、原函数调用、安装与卸载 |
| 协程 | Rust 帧调度器，以及已有托管 IEnumerator 的启动与停止 |
| 类注册 | 新类、字段、方法、接口、虚方法覆盖与 Rust IEnumerator |

根据游戏使用的 Unity 版本选择 feature，见[版本表](docs/zh/BUILD.md#unity-版本)。

## 线程与对象生命周期

`#[cleat::main]` 在 IL2CPP 初始化后，由已附加的工作线程调用。可以在这里安装
Hook、读取普通 Rust 数据。场景对象、UI、AssetBundle 和协程启动应放到 Unity
主线程回调中执行。

跨回调保存对象时，同时保留 `object.root()?` 返回的 GC 根，让对象在使用期间保持存活。
具体用法见[对象寿命](docs/zh/API.md#对象寿命)。

## 开发检查

```sh
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy -p cleat -p cleat-macros --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example injection --features unity-2022-3
```

每次构建选择一个 Unity feature。环境准备和离线构建见[构建说明](docs/zh/BUILD.md)。

## 许可

MIT。依赖组件见[第三方许可](THIRD_PARTY_NOTICES.md)。
