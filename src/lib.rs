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
//! cleat = "0.1"
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
mod types;

pub use error::{Error, Result};
pub use ffi::icall::resolve as resolve_icall;
pub use ffi::icall::invoke_0 as invoke_icall_0;
pub use ffi::icall::invoke_1 as invoke_icall_1;
pub use ffi::icall::invoke_2 as invoke_icall_2;
pub use ffi::icall::invoke_3 as invoke_icall_3;
pub use ffi::icall::invoke_4 as invoke_icall_4;
pub use ffi::icall::invoke_5 as invoke_icall_5;
pub use ffi::icall::invoke_6 as invoke_icall_6;
pub use ffi::icall::invoke_7 as invoke_icall_7;
pub use ffi::icall::invoke_8 as invoke_icall_8;
pub use ffi::icall::invoke_9 as invoke_icall_9;
pub use ffi::icall::invoke_10 as invoke_icall_10;
pub use ffi::icall::invoke_11 as invoke_icall_11;
pub use ffi::icall::invoke_12 as invoke_icall_12;
pub use ffi::icall::invoke_13 as invoke_icall_13;
pub use ffi::icall::invoke_14 as invoke_icall_14;
pub use ffi::icall::invoke_15 as invoke_icall_15;
pub use hook::HookGuard;
pub use init::init;
pub use meta::class::Il2CppClass;
pub use meta::method::MethodInfo;
pub use object::raw::{FieldHandle, Il2CppObject};
pub use types::{
    Args, Il2CppArray, Il2CppList, Il2CppString, Il2CppValueType, Quaternion, Vector2, Vector3,
    Vector4,
};
