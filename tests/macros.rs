//! Consumer compile tests: macros must only need the cleat dependency.
use cleat::prelude::*;

#[cleat::value_type]
#[derive(Clone, Copy)]
#[repr(C)]
struct Position {
    x: f32,
    y: f32,
}

#[cleat::hook("Assembly-CSharp", "Player", "Damage", ["System.Int32"])]
fn damage(this: &Object, amount: i32) -> Result<()> {
    damage::original(this, amount)
}

#[cleat::hook("Assembly-CSharp", "Player", "Update")]
fn update(this: &mut Il2CppObject) -> Result<()> {
    update::original(this)
}

#[cleat::hook("Assembly-CSharp", "Player", "Calculate")]
fn calculate(value: i32) -> Result<i32> {
    calculate::original(value)
}

#[cleat::hook("UnityEngine.UI.Text", "set_text", ["System.String"])]
fn translate(this: &Object, text: Il2CppString) -> Result<()> {
    translate::original(this, text)
}

#[test]
fn original_before_install_returns_error() {
    let _installers = [
        damage::install,
        update::install,
        calculate::install,
        translate::install,
    ];
    let object = Object::default();
    assert!(damage::original(&object, 1).is_err());
    assert!(calculate::original(1).is_err());
    assert!(!damage::is_installed());
    damage::uninstall().unwrap();
    let _ = Position { x: 1.0, y: 2.0 };
}
