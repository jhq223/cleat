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
//!     cleat::set_app_data("/data/data/com.example.game/files");
//!
//!     let player = Il2CppClass::find("Player")?;
//!     let max_hp: i32 = player.static_field_value("maxHealth")?;
//!
//!     Ok(())
//! }
//!
//! #[cleat::hook("Assembly-CSharp", "Player", "TakeDamage")]
//! fn god_mode(this: &Il2CppObject, damage: i32) -> cleat::Result<()> {
//!     // Nullify incoming damage.
//!     god_mode::original(this, 0);
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! * Field access — read and write static/instance fields.
//! * Method calls — parameterised and generic methods supported.
//! * Inline hooks — `#[cleat::hook]` wraps ShadowHook, no manual trampoline code.
//! * Custom value types — `#[cleat::value_type]` derives FFI-safe read/write for
//!   your `#[repr(C)]` structs.
//!
//! ## Setup
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! cleat = { git = "https://github.com/jhq223/cleat" }
//! ```
//!
//! ```bash
//! cargo ndk -t arm64-v8a -o ./target/jniLibs build --release
//! ```
//!
//! ## Requirements
//!
//! * Rust nightly (edition 2024)
//! * Android NDK r26+
//! * cargo-ndk ≥ 3.0
//! * libshadowhook.so (runtime, bundled in the APK)

// Re-export proc macros so users only need `use cleat::prelude::*`.
pub use cleat_macros::*;

// Internal modules.
mod class;
mod error;
mod init;
mod math;
mod method;
mod object;
mod types;
mod value_type;

#[cfg(target_os = "android")]
mod hook;

// Public re-exports.
pub mod prelude;

pub use class::Il2CppClass;
pub use error::{Error, Result};
pub use init::{app_data, init, set_app_data};
pub use math::{Quaternion, Vector3};
pub use method::MethodInfo;
pub use object::Il2CppObject;
pub use types::{Il2CppArray, Il2CppList, Il2CppString};
pub use value_type::{Args, Il2CppValueType};

#[cfg(target_os = "android")]
pub use hook::HookGuard;

/// Desktop stub — allows `cargo check` on non-Android platforms.
/// Hooks are no-ops and will fail gracefully at runtime.
#[cfg(not(target_os = "android"))]
pub struct HookGuard;

#[cfg(not(target_os = "android"))]
impl HookGuard {
    pub fn install(_target: *const std::ffi::c_void, _replace: *const std::ffi::c_void) -> crate::Result<Self> {
        log::warn!("HookGuard::install called on non-Android platform — skipping");
        Ok(Self)
    }
    pub fn trampoline(&self) -> *const std::ffi::c_void {
        std::ptr::null()
    }
    pub fn uninstall(self) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
unsafe impl Send for HookGuard {}
#[cfg(not(target_os = "android"))]
unsafe impl Sync for HookGuard {}
