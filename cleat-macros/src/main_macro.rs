//! `#[cleat::main]` — generates the JNI_OnLoad entry point that Android
//! calls via `System.loadLibrary`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn main_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let expanded = quote! {
        #input_fn

        #[cfg_attr(target_os = "android", unsafe(no_mangle))]
        #[allow(non_snake_case)] // JNI's required entry-point spelling.
        pub unsafe extern "C" fn JNI_OnLoad(
            _vm: *mut ::std::ffi::c_void,
            _reserved: *mut ::std::ffi::c_void,
        ) -> ::std::ffi::c_int {
            #[cfg(target_os = "android")]
            unsafe { cleat::android::initialize(_vm); }
            cleat::init("libil2cpp.so", move || {
                // Catch panics before they cross the FFI boundary — Rust
                // panicking into C is UB.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #fn_name()
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => cleat::__log::error!("main failed: {e}"),
                    Err(panic) => {
                        let msg = panic
                            .downcast_ref::<&str>()
                            .map(|s| *s)
                            .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                            .unwrap_or("<unknown panic>");
                        cleat::__log::error!("main panicked: {msg}");
                    }
                }
            });
            0x0001_0006u32 as ::std::ffi::c_int // JNI_VERSION_1_6
        }
    };

    TokenStream::from(expanded)
}
