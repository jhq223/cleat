//! Build as an Android shared library; replace class names for your game.
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
