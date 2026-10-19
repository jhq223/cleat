//! IL2CPP hooks backed by Frida Gum.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, ItemFn, LitStr, Pat, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

struct Target {
    assembly: Option<LitStr>,
    class: LitStr,
    method: LitStr,
    params: Option<Vec<LitStr>>,
}
impl Parse for Target {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let first: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let second: LitStr = input.parse()?;
        let (assembly, class, method) = if input.peek(Token![,]) {
            let ahead = input.fork();
            ahead.parse::<Token![,]>()?;
            if ahead.peek(LitStr) {
                input.parse::<Token![,]>()?;
                (Some(first), second, input.parse()?)
            } else {
                (None, first, second)
            }
        } else {
            (None, first, second)
        };
        let mut params = None;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
            if !input.is_empty() {
                let content;
                syn::bracketed!(content in input);
                params = Some(
                    Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?
                        .into_iter()
                        .collect(),
                );
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }
        }
        Ok(Self {
            assembly,
            class,
            method,
            params,
        })
    }
}

pub(crate) fn object_ref(ty: &Type) -> Option<bool> {
    let Type::Reference(reference) = ty else {
        return None;
    };
    let Type::Path(path) = &*reference.elem else {
        return None;
    };
    matches!(
        path.path.segments.last()?.ident.to_string().as_str(),
        "Il2CppObject" | "Object"
    )
    .then_some(reference.mutability.is_some())
}

pub fn hook_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    let target = parse_macro_input!(attr as Target);
    let function = parse_macro_input!(item as ItemFn);
    match expand(target, function) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(target: Target, function: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let sig = &function.sig;
    if sig.asyncness.is_some()
        || sig.unsafety.is_some()
        || sig.variadic.is_some()
        || !sig.generics.params.is_empty()
    {
        return Err(syn::Error::new_spanned(
            sig,
            "hooks must be non-generic, synchronous safe functions returning cleat::Result<T>",
        ));
    }
    let ReturnType::Type(_, return_type) = &sig.output else {
        return Err(syn::Error::new_spanned(
            sig,
            "hook must return cleat::Result<T>",
        ));
    };
    let Type::Path(path) = &**return_type else {
        return Err(syn::Error::new_spanned(
            return_type,
            "expected cleat::Result<T>",
        ));
    };
    let segment = path.path.segments.last().unwrap();
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            return_type,
            "expected cleat::Result<T>",
        ));
    };
    let Some(syn::GenericArgument::Type(ret)) = args.args.first() else {
        return Err(syn::Error::new_spanned(
            return_type,
            "expected cleat::Result<T>",
        ));
    };
    if segment.ident != "Result" {
        return Err(syn::Error::new_spanned(
            return_type,
            "expected cleat::Result<T>",
        ));
    }
    let mut names = Vec::new();
    let mut types = Vec::new();
    let mut this_mut = None;
    for (index, arg) in sig.inputs.iter().enumerate() {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new_spanned(arg, "self is not supported"));
        };
        let Pat::Ident(name) = &*arg.pat else {
            return Err(syn::Error::new_spanned(&arg.pat, "use a named parameter"));
        };
        if name.by_ref.is_some() || name.subpat.is_some() {
            return Err(syn::Error::new_spanned(name, "use a plain named parameter"));
        }
        if index == 0 {
            this_mut = object_ref(&arg.ty);
        }
        if index == 0 && this_mut.is_some() {
            continue;
        }
        names.push(&name.ident);
        types.push(&arg.ty);
    }
    let count = names.len();
    let indices: Vec<_> = (0..count).collect();
    let Target {
        assembly,
        class,
        method,
        params,
    } = target;
    let resolve_class = match assembly {
        Some(assembly) => quote! { cleat::Class::find_in(#assembly, #class)? },
        None => quote! { cleat::Class::find(#class)? },
    };
    let selector = if let Some(params) = params {
        if params.len() != count {
            return Err(syn::Error::new_spanned(
                sig,
                "managed parameter type count must match the hook signature (excluding this)",
            ));
        }
        quote! { (#method, [#(#params),*] as [&str; #count]) }
    } else {
        quote! { (#method, #count) }
    };
    let name = &sig.ident;
    let inner_name = format_ident!("{}_inner", name);
    let mut inner = function.clone();
    inner.sig.ident = inner_name.clone();
    let vis = &function.vis;
    let attrs = function.attrs.iter().filter(|attr| {
        ["cfg", "cfg_attr", "doc", "allow", "warn", "deny", "forbid"]
            .iter()
            .any(|name| attr.path().is_ident(name))
    });
    let has_this = this_mut.is_some();
    let this_param = this_mut.map(|mutable| {
        if mutable {
            quote! { mut this: cleat::Il2CppObject, }
        } else {
            quote! { this: cleat::Il2CppObject, }
        }
    });
    let this_type = has_this.then(|| quote! { cleat::Il2CppObject, });
    let this_original = this_mut.map(|mutable| {
        if mutable {
            quote! { this: &mut cleat::Il2CppObject, }
        } else {
            quote! { this: &cleat::Il2CppObject, }
        }
    });
    let borrow_this = this_mut.map(|mutable| {
        if mutable {
            quote! { &mut this, }
        } else {
            quote! { &this, }
        }
    });
    let pass_this = has_this.then(|| quote! { *this, });
    Ok(quote! {
        #inner
        #(#attrs)*
        #vis mod #name {
            #[allow(unused_imports)]
            use super::*;
            static HOOK: cleat::HookSlot = cleat::HookSlot::new();

            /// Install once; repeated calls are idempotent.
            /// The Rust signature must match the selected managed method's ABI.
            pub fn install() -> cleat::Result<()> {
                if HOOK.is_installed() { return Ok(()); }
                let class = #resolve_class;
                let method = class.method(#selector)?;
                if #has_this && class.is_valuetype() {
                    return Err(cleat::Error::Hook("use a raw hook for value-type instance methods".into()));
                }
                if method.is_static() == #has_this {
                    return Err(cleat::Error::Hook("hook this parameter does not match method staticness".into()));
                }
                #(method.validate_parameter::<#types>(#indices)?;)*
                method.validate_hook_return::<#ret>()?;
                unsafe { HOOK.install(method.native()?, __wrapper as *const std::ffi::c_void) }
            }
            /// Remove the hook after any active Rust callbacks finish.
            pub fn uninstall() -> cleat::Result<()> { HOOK.uninstall() }
            pub fn is_installed() -> bool { HOOK.is_installed() }

            /// Call the original, preserving the caller's hidden MethodInfo.
            pub fn original(#this_original #(#names: #types),*) -> cleat::Result<#ret> {
                HOOK.with_original(|method| {
                    let trampoline: unsafe extern "C" fn(#this_type #(#types,)* *mut std::ffi::c_void) -> #ret =
                        unsafe { std::mem::transmute(method.function) };
                    unsafe { trampoline(#pass_this #(#names,)* method.method_info) }
                })
            }

            unsafe extern "C" fn __wrapper(#this_param #(#names: #types,)* __method_info: *mut std::ffi::c_void) -> #ret {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _invocation = HOOK.enter(__method_info)?;
                    super::#inner_name(#borrow_this #(#names),*)
                }));
                match result {
                    Ok(Ok(value)) => value,
                    Ok(Err(error)) => { cleat::__log::error!("{}: {error}", stringify!(#name)); Default::default() }
                    Err(_) => { cleat::__log::error!("{} panicked", stringify!(#name)); Default::default() }
                }
            }
        }
    })
}
