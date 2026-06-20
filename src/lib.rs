//! # cleat — Android IL2CPP game modding toolkit
//!
//! Safe Rust bindings over IL2CPP. Read fields, call methods, install
//! inline hooks — the unsafe stays inside the library.
//!
//! ## Quick example
//!
//! ```ignore
//! use cleat::prelude::*;
//!
//! #[cleat::main]
//! fn my_mod() -> cleat::Result<()> {
//!     let player = Il2CppClass::find("Player")?;
//!     let max_hp: i32 = player.static_field_value("maxHealth")?;
//!     Ok(())
//! }
//!
//! #[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
//! fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
//!     god_mode::original(this, 0);
//!     Ok(())
//! }
//! ```
//!
//! ## Setup
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! cleat = "0.2"
//! ```
//!
//! ```bash
//! cargo ndk -t arm64-v8a -o ./target/jniLibs build --release
//! ```

// Re-export proc macros so users only need `use cleat::prelude::*`.
pub use cleat_macros::*;

// Re-export the full bridge crate for advanced use.
pub use il2cpp_bridge_rs as bridge;

mod error;
mod ffi;
mod hook;
mod init;
mod meta;
mod object;
pub mod prelude;
#[cfg(test)]
mod tests;
mod types;
mod unity;

pub use error::{Error, Result};
pub use ffi::method_ptr::MethodPtr;
pub use ffi::method_ptr::invoke_method_0;
pub use ffi::method_ptr::invoke_method_1;
pub use ffi::method_ptr::invoke_method_2;
pub use ffi::method_ptr::invoke_method_3;
pub use ffi::method_ptr::invoke_method_4;
pub use ffi::method_ptr::invoke_method_5;
pub use ffi::method_ptr::invoke_method_inst_0;
pub use ffi::method_ptr::invoke_method_inst_1;
pub use ffi::method_ptr::invoke_method_inst_2;
pub use ffi::method_ptr::invoke_method_inst_3;
pub use ffi::method_ptr::invoke_method_inst_4;
pub use ffi::method_ptr::invoke_method_inst_5;
pub use ffi::method_ptr::resolve_method;
pub use hook::HookGuard;
pub use init::init;
pub use meta::class::Il2CppClass;
pub use meta::method::MethodInfo;
pub use object::raw::{FieldHandle, Il2CppObject};
pub use types::{
    Args, Il2CppArray, Il2CppDictionary, Il2CppList, Il2CppString, Il2CppValueType, Quaternion,
    Vector2, Vector3, Vector4,
};
pub use unity::AssetBundle;
pub use unity::load_asset_from_memory;
