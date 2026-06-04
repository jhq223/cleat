//! `#[cleat::value_type]` — auto-derives `Il2CppValueType` for any
//! `#[repr(C)]` struct so you can read and write it through IL2CPP fields.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, parse_macro_input};

pub fn value_type_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_item = parse_macro_input!(item as ItemStruct);
    let type_name = &struct_item.ident;

    let expanded = quote! {
        // The struct definition itself, passed through unchanged.
        #struct_item

        unsafe impl cleat::Il2CppValueType for #type_name {
            unsafe fn load_field(
                field: &il2cpp_bridge_rs::structs::Field
            ) -> cleat::Result<Self> {
                unsafe { field.get_value::<Self>().map_err(|e| cleat::Error::Bridge(e)) }
            }

            unsafe fn store_field(
                field: &il2cpp_bridge_rs::structs::Field,
                val: Self,
            ) -> cleat::Result<()> {
                unsafe { field.set_value::<Self>(val).map_err(|e| cleat::Error::Bridge(e)) }
            }
        }
    };

    TokenStream::from(expanded)
}
