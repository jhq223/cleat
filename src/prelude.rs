//! cleat prelude — imports all commonly used types in one line.
//!
//! ```ignore
//! use cleat::prelude::*;
//! ```

pub use crate::class::Il2CppClass;
pub use crate::error::{Error, Result};
pub use crate::init::{app_data, set_app_data};
pub use crate::math::{Quaternion, Vector2, Vector3, Vector4};
pub use crate::method::MethodInfo;
pub use crate::object::Il2CppObject;
pub use crate::types::{Il2CppArray, Il2CppList, Il2CppString};
pub use crate::value_type::{Args, Il2CppValueType};
