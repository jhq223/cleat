//! cleat prelude — imports all commonly used types in one line.
//!
//! ```ignore
//! use cleat::prelude::*;
//! ```

pub use crate::error::{Error, Result};
pub use crate::meta::class::Il2CppClass;
pub use crate::meta::method::MethodInfo;
pub use crate::object::raw::{FieldHandle, Il2CppObject};
pub use crate::types::{
    Args, Il2CppArray, Il2CppDictionary, Il2CppList, Il2CppString, Il2CppValueType,
};
pub use crate::types::{Quaternion, Vector2, Vector3, Vector4};
pub use crate::unity::{AssetBundle, load_asset_from_memory};
