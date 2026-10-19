//! Replace PlayerController/Start with a main-thread method in the target game.
use cleat::injection::{ClassBuilder, InjectionRuntime, UnityCoroutine};
use cleat::prelude::*;
use std::sync::OnceLock;

#[cleat::main]
fn setup() -> Result<()> {
    start::install()
}

#[cleat::managed_method("Awake")]
fn mod_awake(this: &Object) -> Result<()> {
    this.set("speed", 2.0f32)
}

#[cleat::hook("Assembly-CSharp", "PlayerController", "Start")]
fn start(this: &Object) -> Result<()> {
    start::original(this)?;
    static MOD_CLASS: OnceLock<Class> = OnceLock::new();
    // Start is a Unity main-thread callback; select the game's exact Unity feature.
    let runtime = unsafe { InjectionRuntime::initialize()? };
    let class = if let Some(class) = MOD_CLASS.get() {
        class.clone()
    } else {
        let class = ClassBuilder::new(
            "Assembly-CSharp",
            "MyMod.Controller",
            &Class::find("UnityEngine.MonoBehaviour")?,
        )
        .field::<f32>("speed")?
        .method(mod_awake::definition()?)
        .register(&runtime)?;
        let _ = MOD_CLASS.set(class.clone());
        class
    };
    let game_object: Object = this.get("gameObject")?;
    let _component: Object =
        game_object.call(("AddComponent", ["System.Type"]), (class.type_object()?,))?;
    UnityCoroutine::start(&runtime, *this, |co| async move {
        co.end_of_frame().await;
        co.seconds(0.5).await;
        co.fixed_update().await;
        Ok(())
    })?;
    Ok(())
}
