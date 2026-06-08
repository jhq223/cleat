//! cleat prelude — imports all commonly used types in one line.
//!
//! ```ignore
//! use cleat::prelude::*;
//! ```

pub use crate::error::{Error, Result};
pub use crate::ffi::icall::resolve as resolve_icall;
pub use crate::meta::class::Il2CppClass;
pub use crate::meta::method::MethodInfo;
pub use crate::object::raw::{FieldHandle, Il2CppObject};
pub use crate::types::{Args, Il2CppArray, Il2CppList, Il2CppString, Il2CppValueType};
pub use crate::types::{Quaternion, Vector2, Vector3, Vector4};
