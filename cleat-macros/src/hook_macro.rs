//! `#[cleat::hook]` — inline hook proc macro.
//!
//! Generates all the plumbing for hooking an IL2CPP method via ShadowHook:
//! a C-ABI wrapper that the hook engine calls, an `original()` trampoline
//! to invoke the real method, and `install()` / `uninstall()` helpers.
//!
//! # What signatures work
//!
//! * Instance methods — first param is `&Il2CppObject` or `&mut Il2CppObject`.
//! * Static methods — no `this`.
//! * Return type — `Result<()>` or `Result<T>` (anything `Default`).
//! * Extra params passed through the `Args` trait.
//!
//! # Error behaviour
//!
//! The generated C wrapper must not panic (it's called from native code),
//! so when the user's function returns `Err(...)`:
//!
//! * **Returning `Result<T>`** — the wrapper logs the error and returns
//!   `T::default()` (e.g. `0` for `i32`). The game code gets a zero value
//!   instead of crashing.
//! * **Returning `Result<()>`** — the wrapper just logs the error.
//!
//! This is deliberate. Hooking is a "best effort" thing — if your hook
//! fails the game should keep running.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, FnArg, ItemFn, Pat, PatType, ReturnType, Type,
};

// ── helper functions ──────────────────────────────────────────────────────

/// Test whether a type is `&Il2CppObject` or `&mut Il2CppObject`.
///
/// Returns `Some(true)` for `&mut`, `Some(false)` for `&`, `None` otherwise.
fn is_il2cpp_object_ref(ty: &Type) -> Option<bool> {
    if let Type::Reference(ref_ty) = ty {
        if let Type::Path(type_path) = &*ref_ty.elem {
            if type_path
                .path
                .segments
                .last()
                .map(|s| s.ident == "Il2CppObject")
                .unwrap_or(false)
            {
                return Some(ref_ty.mutability.is_some());
            }
        }
    }
    None
}

/// Peel `Result<T, E>` and return the inner type `T`.
///
/// Returns `Some((has_value, token_stream))`. `has_value = false` when
/// `T = ()` (the empty tuple — a void-ish result).
fn extract_result_inner(ty: &Type) -> Option<(bool, TokenStream2)> {
    if let Type::Path(type_path) = ty {
        let seg = type_path.path.segments.last()?;
        if seg.ident != "Result" {
            return None;
        }
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
            let first = args.args.first()?;
            if let syn::GenericArgument::Type(t) = first {
                let is_unit = matches!(t, Type::Tuple(tup) if tup.elems.is_empty());
                Some((!is_unit, quote! { #t }))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Pull non-`this` parameter names and types from the function input list.
fn extract_params<'a>(
    inputs: &'a syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    has_this: bool,
) -> Vec<(&'a syn::Ident, &'a Type)> {
    let mut params = Vec::new();
    for (i, arg) in inputs.iter().enumerate() {
        if i == 0 && has_this {
            continue;
        }
        if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
            if let Pat::Ident(pat_ident) = &**pat {
                params.push((&pat_ident.ident, &**ty));
            }
        }
    }
    params
}

// ── the macro itself ──────────────────────────────────────────────────────

pub fn hook_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // ── parse the attribute: #[cleat::hook("Assembly", "Class", "Method")] ──

    let attr_str = attr.to_string();
    let parts: Vec<String> = attr_str
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .collect();

    if parts.len() != 3 {
        return syn::Error::new(
            proc_macro::Span::call_site().into(),
            "expected #[cleat::hook(\"assembly\", \"class\", \"method\")]",
        )
        .to_compile_error()
        .into();
    }

    let assembly_lit = proc_macro2::Literal::string(&parts[0]);
    let class_lit    = proc_macro2::Literal::string(&parts[1]);
    let method_lit   = proc_macro2::Literal::string(&parts[2]);

    // ── inspect user function signature ──

    let fn_name    = input_fn.sig.ident.clone();
    let inner_name = format_ident!("{}_inner", fn_name);
    let mod_name   = fn_name.clone();
    let fn_body    = &input_fn.block;

    // `this` detection
    let mut has_this    = false;
    let mut is_this_mut = false;
    if let Some(first_arg) = input_fn.sig.inputs.first() {
        if let FnArg::Typed(PatType { ty, .. }) = first_arg {
            if let Some(mut_ref) = is_il2cpp_object_ref(ty) {
                has_this    = true;
                is_this_mut = mut_ref;
            }
        }
    }

    // The rest of the params
    let params       = extract_params(&input_fn.sig.inputs, has_this);
    let param_idents: Vec<_> = params.iter().map(|(id, _)| *id).collect();
    let param_types:  Vec<_> = params.iter().map(|(_, ty)| *ty).collect();

    // Return type
    let (has_ret, ret_ty) = match &input_fn.sig.output {
        ReturnType::Default => (false, quote! { () }),
        ReturnType::Type(_, ty) => {
            if let Some((has_val, inner)) = extract_result_inner(ty) {
                (has_val, inner)
            } else {
                (false, quote! { () })
            }
        }
    };

    // The user's original signature, renamed to _inner.
    let mut inner_sig = input_fn.sig.clone();
    inner_sig.ident = inner_name.clone();

    // ── build codegen fragments ──

    let inner_fn = quote! {
        #[allow(non_snake_case)]
        #inner_sig {
            #fn_body
        }
    };

    // C wrapper parameter list.
    //
    // `this` is received **by value** as `cleat::Il2CppObject` because:
    //  - at the C ABI level it's `Il2CppObject*` (a single 8-byte pointer)
    //  - `cleat::Il2CppObject` is `#[repr(transparent)]` over that pointer,
    //    so the bytes land correctly in the struct without needing `&*`.
    // Using `*mut cleat::Il2CppObject` would cause `&*this` to read the
    // *first field* (klass) of the managed object instead of the object
    // pointer itself.
    let wrapper_params_c: TokenStream2 = if has_this {
        if param_idents.is_empty() {
            quote! { this: cleat::Il2CppObject }
        } else {
            quote! { this: cleat::Il2CppObject, #(#param_idents: #param_types),* }
        }
    } else {
        quote! { #(#param_idents: #param_types),* }
    };

    let wrapper_ret: TokenStream2 = if has_ret {
        quote! { -> #ret_ty }
    } else {
        quote! {}
    };

    // Trampoline function pointer type (what we transmute the guard's
    // trampoline pointer into before calling through it).
    let trampoline_type: TokenStream2 = if has_this {
        if has_ret {
            if param_types.is_empty() {
                quote! { unsafe extern "C" fn(cleat::Il2CppObject) -> #ret_ty }
            } else {
                quote! { unsafe extern "C" fn(cleat::Il2CppObject, #(#param_types),*) -> #ret_ty }
            }
        } else {
            if param_types.is_empty() {
                quote! { unsafe extern "C" fn(cleat::Il2CppObject) }
            } else {
                quote! { unsafe extern "C" fn(cleat::Il2CppObject, #(#param_types),*) }
            }
        }
    } else {
        if has_ret {
            quote! { unsafe extern "C" fn(#(#param_types),*) -> #ret_ty }
        } else {
            quote! { unsafe extern "C" fn(#(#param_types),*) }
        }
    };

    // How the C wrapper calls super::inner.
    // `this` is `cleat::Il2CppObject` by value — just borrow it directly.
    let inner_call_args: TokenStream2 = if has_this {
        let deref = if is_this_mut {
            quote! { &mut this }
        } else {
            quote! { &this }
        };
        if param_idents.is_empty() {
            quote! { super::#inner_name(#deref) }
        } else {
            quote! { super::#inner_name(#deref, #(#param_idents),*) }
        }
    } else {
        quote! { super::#inner_name(#(#param_idents),*) }
    };

    // The user-facing original() signature (friendly borrows, not C types)
    let original_params: TokenStream2 = if has_this {
        let ref_kw = if is_this_mut { quote! { &mut } } else { quote! { & } };
        if param_idents.is_empty() {
            quote! { this: #ref_kw cleat::Il2CppObject }
        } else {
            quote! { this: #ref_kw cleat::Il2CppObject, #(#param_idents: #param_types),* }
        }
    } else {
        quote! { #(#param_idents: #param_types),* }
    };

    let original_ret: TokenStream2 = if has_ret {
        quote! { -> #ret_ty }
    } else {
        quote! {}
    };

    // How original() calls through the trampoline.
    // `original()` receives `this: &Il2CppObject` — deref & pass by value.
    let trampoline_call_args: TokenStream2 = if has_this {
        if param_idents.is_empty() {
            quote! { *this }
        } else {
            quote! { *this, #(#param_idents),* }
        }
    } else {
        quote! { #(#param_idents),* }
    };

    // Wrapper body: logs errors, returns Default on failure.
    let wrapper_body: TokenStream2 = if has_ret {
        quote! {
            match #inner_call_args {
                Ok(v) => v,
                Err(e) => {
                    log::error!("{}: {e}", stringify!(#fn_name));
                    Default::default()
                }
            }
        }
    } else {
        quote! {
            if let Err(e) = #inner_call_args {
                log::error!("{}: {e}", stringify!(#fn_name));
            }
        }
    };

    // original() body: transmute guard trampoline → call through it.
    let original_body: TokenStream2 = if has_ret {
        quote! {
            let guard = HOOK.get().expect("hook not installed");
            let trampoline: #trampoline_type =
                unsafe { std::mem::transmute(guard.trampoline()) };
            unsafe { trampoline(#trampoline_call_args) }
        }
    } else {
        quote! {
            let guard = HOOK.get().expect("hook not installed");
            let trampoline: #trampoline_type =
                unsafe { std::mem::transmute(guard.trampoline()) };
            unsafe { trampoline(#trampoline_call_args); }
        }
    };

    // ── stitch everything together ──

    let expanded = quote! {
        #inner_fn

        pub(crate) mod #mod_name {
            use std::sync::OnceLock;
            use cleat::prelude::*;
            use cleat::HookGuard;

            static HOOK: OnceLock<HookGuard> = OnceLock::new();

            /// Install the hook (safe to call multiple times — subsequent
            /// calls are no-ops).
            pub fn install() -> cleat::Result<()> {
                if HOOK.get().is_some() {
                    return Ok(());
                }

                let class  = cleat::Il2CppClass::find_with(#class_lit, #assembly_lit)?;
                let method = class.method_ptr(#method_lit)?;

                let guard = unsafe {
                    HookGuard::install(
                        method.function,
                        __wrapper as *const std::ffi::c_void,
                    )?
                };

                HOOK.set(guard)
                    .map_err(|_| cleat::Error::Hook("already installed".into()))
            }

            /// Remove the hook and restore the original code.
            pub fn uninstall() -> cleat::Result<()> {
                // Safety: IL2CPP runs single-threaded — no concurrent mutation.
                let taken = unsafe {
                    let ptr = &raw const HOOK as *mut OnceLock<HookGuard>;
                    (*ptr).take()
                };
                taken
                    .map(|g| g.uninstall())
                    .unwrap_or(Ok(()))
            }

            /// Call the original (unhooked) method through the trampoline.
            pub fn original(#original_params) #original_ret {
                #original_body
            }

            /// C callback. Signature must match the hooked method exactly —
            /// ShadowHook transfers control here when the game calls the
            /// patched function.
            unsafe extern "C" fn __wrapper(#wrapper_params_c) #wrapper_ret {
                #wrapper_body
            }
        }
    };

    TokenStream::from(expanded)
}
