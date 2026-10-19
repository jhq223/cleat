//! High-level Unity-specific wrappers built on top of cleat's core API.

pub mod asset_bundle;

pub use asset_bundle::AssetBundle;
pub use asset_bundle::load_asset_from_memory;
pub mod coroutine;

/// Find loaded Unity objects on Unity's main thread, optionally including hidden objects.
pub fn find_objects(
    type_name: &str,
    include_inactive: bool,
) -> crate::Result<crate::Il2CppArray<crate::Object>> {
    let class = crate::Class::find(type_name)?;
    crate::Class::find("UnityEngine.Object")?.call(
        ("FindObjectsOfType", ["System.Type", "System.Boolean"]),
        (class.type_object()?, include_inactive),
    )
}
