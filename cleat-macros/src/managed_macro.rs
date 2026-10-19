use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, LitStr, Pat, ReturnType, Type, parse_macro_input};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let name = if attr.is_empty() {
        LitStr::new(&function.sig.ident.to_string(), function.sig.ident.span())
    } else {
        parse_macro_input!(attr as LitStr)
    };
    match generate(function, name) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
fn generate(function: ItemFn, managed_name: LitStr) -> syn::Result<proc_macro2::TokenStream> {
    let sig = &function.sig;
    let error = || {
        syn::Error::new_spanned(
            sig,
            "managed methods must be synchronous, non-generic functions returning Result<T>",
        )
    };
    if sig.asyncness.is_some()
        || sig.unsafety.is_some()
        || sig.variadic.is_some()
        || !sig.generics.params.is_empty()
    {
        return Err(error());
    }
    let ReturnType::Type(_, ty) = &sig.output else {
        return Err(error());
    };
    let Type::Path(path) = &**ty else {
        return Err(error());
    };
    let segment = path.path.segments.last().ok_or_else(error)?;
    if segment.ident != "Result" {
        return Err(error());
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(error());
    };
    let Some(syn::GenericArgument::Type(ret)) = arguments.args.first() else {
        return Err(error());
    };
    let mut names = Vec::new();
    let mut types = Vec::new();
    let mut this = None;
    for (index, argument) in sig.inputs.iter().enumerate() {
        let FnArg::Typed(argument) = argument else {
            return Err(error());
        };
        let Pat::Ident(name) = &*argument.pat else {
            return Err(error());
        };
        if name.by_ref.is_some() || name.subpat.is_some() {
            return Err(error());
        }
        if index == 0 {
            this = crate::hook_macro::object_ref(&argument.ty);
        }
        if index == 0 && this.is_some() {
            continue;
        }
        names.push(&name.ident);
        types.push(&argument.ty);
    }
    let indices: Vec<_> = (0..names.len()).collect();
    let static_method = this.is_none();
    let this_arg = this.map(|mutable| {
        if mutable {
            quote! { mut this: cleat::Object, }
        } else {
            quote! { this: cleat::Object, }
        }
    });
    let this_call = this.map(|mutable| {
        if mutable {
            quote! { &mut this, }
        } else {
            quote! { &this, }
        }
    });
    let this_invoke = this.map(|_| quote! { unsafe { cleat::Object::from_raw(object) }, });
    let name = &sig.ident;
    let vis = &function.vis;
    let attrs = function.attrs.iter().filter(|attr| {
        ["cfg", "cfg_attr", "doc", "allow", "warn", "deny", "forbid"]
            .iter()
            .any(|name| attr.path().is_ident(name))
    });
    Ok(quote! {
        #function
        #(#attrs)*
        #vis mod #name {
            #[allow(unused_imports)] use super::*;
            pub fn definition() -> cleat::Result<cleat::injection::ManagedMethod> {
                Ok(unsafe { cleat::injection::ManagedMethod::from_parts(#managed_name,
                    cleat::injection::type_of::<#ret>()?, vec![#(cleat::injection::type_of::<#types>()?),*], #static_method,
                    __native as *mut std::ffi::c_void, __old as *mut std::ffi::c_void, __new as *mut std::ffi::c_void) })
            }
            unsafe extern "C" fn __native(#this_arg #(#names: #types,)* _method: *mut std::ffi::c_void) -> #ret {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::#name(#this_call #(#names),*))) {
                    Ok(Ok(value)) => value,
                    Ok(Err(error)) => { cleat::__log::error!("{}: {error}", #managed_name); Default::default() }
                    Err(_) => { cleat::__log::error!("{} panicked", #managed_name); Default::default() }
                }
            }
            unsafe extern "C" fn __old(_native: *mut std::ffi::c_void, method: *mut std::ffi::c_void,
                object: *mut std::ffi::c_void, arguments: *mut *mut std::ffi::c_void) -> *mut std::ffi::c_void {
                let value = unsafe { __native(#this_invoke #(cleat::injection::read_argument::<#types>(*arguments.add(#indices)),)* method) };
                unsafe { cleat::injection::box_return(method, value) }
            }
            unsafe extern "C" fn __new(_native: *mut std::ffi::c_void, method: *mut std::ffi::c_void,
                object: *mut std::ffi::c_void, arguments: *mut *mut std::ffi::c_void, output: *mut std::ffi::c_void) {
                let value = unsafe { __native(#this_invoke #(cleat::injection::read_argument::<#types>(*arguments.add(#indices)),)* method) };
                if std::mem::size_of::<#ret>() != 0 { unsafe { output.cast::<#ret>().write_unaligned(value) }; }
            }
        }
    })
}
