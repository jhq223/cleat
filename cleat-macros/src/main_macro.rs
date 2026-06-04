//! `#[cleat::main]` — generates the `load()` entry point that Android calls
//! via `System.loadLibrary`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn main_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let expanded = quote! {
        #input_fn

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn load() {
            cleat::init("libil2cpp.so", move || {
                // Catch panics before they cross the FFI boundary — Rust
                // panicking into C is UB.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #fn_name()
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => log::error!("main failed: {e}"),
                    Err(panic) => {
                        let msg = panic
                            .downcast_ref::<&str>()
                            .map(|s| *s)
                            .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                            .unwrap_or("<unknown panic>");
                        log::error!("main panicked: {msg}");
                    }
                }
            });
        }
    };

    TokenStream::from(expanded)
}
