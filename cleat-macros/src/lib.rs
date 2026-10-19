//! Proc macros for cleat.
//!
//! | Attribute               | What it does                                          |
//! |-------------------------|-------------------------------------------------------|
//! | `#[cleat::main]`        | Generates Android's `JNI_OnLoad` entry point. |
//! | `#[cleat::hook]`        | Creates an inline hook with install / uninstall helpers. |
//! | `#[cleat::value_type]`  | Derives `Il2CppValueType` for a `#[repr(C)]` struct.  |

// Suppress linker noise on Windows — proc-macro crates are built as dylibs
// and produce `.dll.lib` / `.dll.exp` artifacts.
#![allow(linker_messages)]

mod hook_macro;
mod main_macro;
mod managed_macro;
mod type_macro;

use proc_macro::TokenStream;

/// Wraps your function in the Android entry point.
///
/// Generates `JNI_OnLoad`, called by Java's `System.loadLibrary`.
/// Initialization waits for IL2CPP on a worker thread, then calls this function
/// on an attached worker. Defer Unity engine operations to a main-thread hook.
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    main_macro::main_macro(attr, item)
}

/// Injects an inline hook for the given method.
///
/// Usage: `#[cleat::hook("AssemblyName", "ClassName", "MethodName")]`.
/// The function body becomes the replacement, and a module named after
/// the function is generated with `install()`, `uninstall()`, and
/// `original()`.
#[proc_macro_attribute]
pub fn hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    hook_macro::hook_macro(attr, item)
}

/// Derives `Il2CppValueType` for a `#[repr(C)]` struct.
///
/// Apply this, then you can read and write the struct directly through
/// IL2CPP fields and method calls.
/// The author must ensure the struct is blittable and exactly matches the
/// managed fields, size and alignment. The macro cannot prove that contract.
#[proc_macro_attribute]
pub fn value_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    type_macro::value_type_macro(attr, item)
}

/// Expose a Rust callback in an injected managed class. Requires a unity-* feature.
/// The generated module provides definition() for ClassBuilder::method.
#[proc_macro_attribute]
pub fn managed_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    managed_macro::expand(attr, item)
}
