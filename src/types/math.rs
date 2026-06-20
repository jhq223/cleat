//! Math types re-exported from `il2cpp_bridge_rs` (which wraps
//! [`glam`](https://crates.io/crates/glam) under the hood).
//!
//! All four types implement `Il2CppValueType`, so you can read/write them
//! as fields and pass them as method arguments directly.

use il2cpp_bridge_rs::structs::Field;

use super::Il2CppValueType;
use crate::{Error, Result};

// Re-export so users don't need to pull in il2cpp_bridge_rs themselves.
pub use il2cpp_bridge_rs::structs::{Quaternion, Vector2, Vector3, Vector4};

macro_rules! impl_value_type_math {
    ($($t:ty),*) => {
        $(
            unsafe impl Il2CppValueType for $t {
                unsafe fn load_field(field: &Field) -> Result<Self> {
                    unsafe { field.get_value::<$t>().map_err(Error::Bridge) }
                }

                unsafe fn store_field(field: &Field, val: Self) -> Result<()> {
                    unsafe { field.set_value::<$t>(val).map_err(Error::Bridge) }
                }
            }
        )*
    };
}

impl_value_type_math!(Vector2, Vector3, Vector4, Quaternion);
