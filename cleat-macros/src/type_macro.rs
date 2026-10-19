//! `#[cleat::value_type]` — auto-derives `Il2CppValueType` for any
//! `#[repr(C)]` struct so you can read and write it through IL2CPP fields.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, LitStr, Meta, Token, parse_macro_input, punctuated::Punctuated};

pub fn value_type_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_item = parse_macro_input!(item as ItemStruct);
    let managed_name = if attr.is_empty() {
        quote!(None)
    } else {
        let name = parse_macro_input!(attr as LitStr);
        quote!(Some(#name))
    };
    let mut compatible = false;
    for attr in &struct_item.attrs {
        if attr.path().is_ident("repr") {
            let Ok(options) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            else {
                continue;
            };
            for option in options {
                if option.path().is_ident("packed") {
                    return syn::Error::new_spanned(
                        option,
                        "packed structs cannot represent managed value types",
                    )
                    .to_compile_error()
                    .into();
                }
                compatible |= option.path().is_ident("C") || option.path().is_ident("transparent");
            }
        }
    }
    if !compatible {
        return syn::Error::new_spanned(
            &struct_item.ident,
            "value_type requires #[repr(C)] or #[repr(transparent)]",
        )
        .to_compile_error()
        .into();
    }
    let type_name = &struct_item.ident;
    let (impl_generics, ty_generics, where_clause) = struct_item.generics.split_for_impl();

    let expanded = quote! {
        // The struct definition itself, passed through unchanged.
        #struct_item

        unsafe impl #impl_generics cleat::Il2CppValueType for #type_name #ty_generics #where_clause {
            const MANAGED_NAME: Option<&'static str> = #managed_name;
            unsafe fn load_field(
                field: &cleat::bridge::structs::Field
            ) -> cleat::Result<Self> {
                unsafe { field.get_value::<Self>().map_err(|e| cleat::Error::Bridge(e)) }
            }

            unsafe fn store_field(
                field: &cleat::bridge::structs::Field,
                val: Self,
            ) -> cleat::Result<()> {
                unsafe { field.set_value::<Self>(val).map_err(|e| cleat::Error::Bridge(e)) }
            }
        }
    };

    TokenStream::from(expanded)
}
