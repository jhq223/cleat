//! # cleat — Android IL2CPP game modding toolkit
//!
//! Rust APIs for managed fields, properties, methods, translation and coroutines.
//! Android hooks use Frida Gum on ARM, ARM64, x86 and x86_64.
//! Managed handles are unrooted references; retain a [`GcHandle`] when storing
//! an object between callbacks. Run Unity engine operations on its main thread.
//!
//! ## Quick example
//!
//! ```no_run
//! use cleat::prelude::*;
//!
//! #[cleat::main]
//! fn my_mod() -> cleat::Result<()> {
//!     translate::install()
//! }
//!
//! #[cleat::hook("UnityEngine.UI", "UnityEngine.UI.Text", "set_text")]
//! fn translate(this: &Object, text: Il2CppString) -> Result<()> {
//!     let text = match text.to_string_lossy().as_str() {
//!         "Start Game" => Il2CppString::try_new("开始游戏")?,
//!         _ => text,
//!     };
//!     translate::original(this, text)
//! }
//! # fn main() {}
//! ```
//!
//! ## Setup
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! cleat = { path = "../cleat" }
//! ```
//!
//! ```bash
//! cargo ndk -t arm64-v8a -o ./target/jniLibs build --release
//! ```
//!
//! Version-specific class registration and native `IEnumerator` support live
//! in `injection`, enabled with one `unity-*` feature. See the repository API
//! guide for supported layouts and runtime requirements.

// Re-export proc macros so users only need `use cleat::prelude::*`.
extern crate self as cleat;
pub use cleat_macros::*;

// Re-export the full bridge crate for advanced use.
pub use il2cpp_bridge_rs as bridge;

#[cfg(target_os = "android")]
pub mod android;
mod error;
mod ffi;
mod hook;
mod init;
#[cfg(feature = "class-injection")]
pub mod injection;
mod meta;
mod object;
pub mod prelude;
mod runtime;
#[cfg(test)]
mod tests;
mod types;
pub mod unity;
pub use runtime::{GcHandle, ThreadGuard};
#[doc(hidden)]
pub use types::args::{
    IntoArgument, PreparedArgs, PreparedArgument, PreparedArguments, ValueArgument,
};
pub use types::args::{RefArg, by_ref, out};
pub use types::value::IntoIl2Cpp;

pub use Il2CppClass as Class;
pub use Il2CppObject as Object;
pub use bridge::structs::core::hierarchy::class::MethodSelector;
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
#[doc(hidden)]
pub use hook::{HookInvocation, HookSlot};
pub use init::init;
#[doc(hidden)]
pub use log as __log;
pub use meta::assembly::Assembly;
pub use meta::class::Il2CppClass;
pub use meta::delegate::Delegate;
pub use meta::method::MethodInfo;
pub use meta::{event::Event, property::Property};
pub use object::raw::{FieldHandle, Il2CppObject};
pub use types::{
    Args, Bounds, Color, Il2CppArray, Il2CppDictionary, Il2CppList, Il2CppString, Il2CppValueType,
    Matrix3x3, Matrix4x4, Quaternion, Ray, Rect, RootedArray, Vector2, Vector3, Vector4,
};
pub use unity::AssetBundle;
pub use unity::coroutine;
pub use unity::coroutine::{
    CoroutineRunner, CoroutineScheduler, CoroutineStatus, CoroutineTask, ManagedCoroutine,
};
pub use unity::load_asset_from_memory;
