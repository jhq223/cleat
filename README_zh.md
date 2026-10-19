# cleat

[English](README.md) · [API 文档](docs/zh/API.md) · [构建说明](docs/zh/BUILD.md) · [修改历史](CHANGELOG.md)

cleat 是面向 Android Unity IL2CPP 游戏的 Rust 修改工具库，可以读取和修改字段、
访问属性、调用方法、替换文本和字体、安装 Hook，以及编写跨帧执行的逻辑。

项目实现使用 Rust，Hook 链接 Frida Gum 官方原生库，覆盖 ARM、ARM64、x86 和
x86_64。BNM 用于参考功能与研究 ABI，项目不包含或编译 BNM 代码。

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
当前仓库的 0.3 API 尚未发布到 crates.io。

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
    let _retained = manager.root()?; // 存储对象时，也保留这个 GC 根
    Ok(())
}
```

`get`、`set` 接受字段名或属性名，`call` 使用元组传参。相同参数数量的重载需要
指定参数类型；查找不明确或调用失败会返回错误。

## 功能

| 范围 | 支持内容 |
|---|---|
| 反射 | 程序集、类、嵌套类、字段、属性、方法、接口与继承关系 |
| 托管操作 | 构造对象、装箱/拆箱、泛型实例化、虚调用、委托与事件访问器 |
| 集合 | 数组、列表、字典；检查类型与边界，通过运行时遍历字典 |
| 汉化与资源 | 直接传入 Rust 字符串、UTF-16 访问、文本 Hook、AssetBundle/字体加载 |
| Hook | Frida Gum 替换、原函数调用、安装与卸载 |
| 协程 | Rust 帧调度器，以及已有托管 IEnumerator 的启动与停止 |
| 可选内部功能 | 注册新类/字段/方法/接口、覆盖虚方法、Rust IEnumerator 与 Unity 各阶段等待 |

与 BNM 的剩余差异见 [API 文档](docs/zh/API.md)。类注册与原生 IEnumerator 要选择一个
Unity feature：`unity-2019-4`、`unity-2020-3`（20 补丁及以上）、
`unity-2021-3`、`unity-2022-3` 或 `unity-2023-1`。
它们对应内部 ABI 布局，不能代替目标游戏验证。Unity 2023 的 GC 句柄 ABI 也需要
对应 feature。Unity 6 使用 `unity-6000` 支持公开 API 和 GC 句柄，暂不支持类注入。

## 线程与对象生命周期

`#[cleat::main]` 在 IL2CPP 初始化后，由已附加的工作线程调用。可以在这里安装
Hook、读取普通 Rust 数据。场景对象、UI、AssetBundle 和协程启动应放到 Unity
主线程回调中执行。

托管包装类型本身不阻止 GC 回收。对象跨回调保存，或在没有其他托管引用时跨分配
使用，需要同时保留 `GcHandle` 或 `object.root()` 返回的根。GC 根不提供线程同步。
自定义值类型布局和原始指针仍需调用者保证正确。

Gum 已在 MuMu Android 15 的原生 x86_64 与 Houdini ARM64 转译环境通过测试，
包括并发调用和 20 轮卸载重装。这个结果验证 Hook 后端；版本相关的类注册仍需
单独进行游戏内验证。

## 开发检查

```sh
cargo test --workspace
cargo test --workspace --features unity-2022-3
cargo clippy -p cleat -p cleat-macros --all-targets --features unity-2022-3 -- -D warnings
cargo ndk -t arm64-v8a build --example injection --features unity-2022-3
```

Unity feature 互斥，请不要使用 `--all-features`。
[内部 bridge crate](crates/il2cpp-bridge-rs/UPSTREAM.md) 随本仓库维护，构建不追踪
上游 Git 分支。第三方组件说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
