//! Math types re-exported from `il2cpp_bridge_rs` (which wraps
//! [`glam`](https://crates.io/crates/glam) under the hood).
//!
//! All four types are `Il2CppValueType`, so you can read/write them as
//! fields and pass them as method arguments directly:
//!
//! ```ignore
//! let pos: Vector3 = obj.load("position")?;
//! let dist = pos.distance(Vector3::ZERO);
//! obj.store("position", pos + Vector3::new(0.0, 1.0, 0.0))?;
//! ```

use crate::{Error, Il2CppValueType, Result};
use il2cpp_bridge_rs::structs::Field;

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
