//! `UnityEngine.AssetBundle` high-level wrapper.
//!
//! Encapsulates the multi-step AssetBundle loading flow (LoadFromMemoryAsync →
//! get_assetBundle → LoadAssetAsync → get_allAssets) behind a clean API.
//!
//! Uses [`resolve_method`](crate::ffi::method_ptr::resolve_method) internally
//! so it works on both old Unity (iCall binding) and Unity 6+ (`[FreeFunction]`
//! binding).

use crate::{
    Il2CppArray, Il2CppClass, Il2CppObject, Il2CppString, Result,
    ffi::method_ptr::{
        invoke_method_2, invoke_method_inst_0, invoke_method_inst_2, resolve_method,
    },
};

const ASM: &str = "UnityEngine.AssetBundleModule.dll";
const CLASS_AB: &str = "UnityEngine.AssetBundle";
const CLASS_REQ: &str = "UnityEngine.AssetBundleCreateRequest";
const CLASS_AREQ: &str = "UnityEngine.AssetBundleRequest";

/// A loaded Unity `AssetBundle`.
///
/// Wraps the managed `AssetBundle` object and provides high-level methods
/// for loading assets from it.
pub struct AssetBundle {
    obj: Il2CppObject,
}

impl AssetBundle {
    /// Load an `AssetBundle` from a byte slice in memory.
    ///
    /// Equivalent to `AssetBundle.LoadFromMemoryAsync(data).assetBundle`.
    pub fn load_from_memory(data: &[u8]) -> Result<Self> {
        // Build a managed byte[] from the raw bytes.
        let byte_class = Il2CppClass::find("System.Byte")?;
        let mut arr = Il2CppArray::new(&byte_class, data.len())?;
        for (i, &b) in data.iter().enumerate() {
            unsafe { arr.set_unchecked(i, b) };
        }

        // 1. LoadFromMemoryAsync_Internal(byte[], uint) — static
        let m_load = resolve_method(ASM, CLASS_AB, "LoadFromMemoryAsync_Internal")?;
        let req = unsafe { invoke_method_2(m_load, arr.as_ptr(), std::ptr::null_mut()) };
        if req.is_null() {
            return Err(crate::Error::Hook(
                "LoadFromMemoryAsync returned null".into(),
            ));
        }

        // 2. get_assetBundle() — instance on AssetBundleCreateRequest
        let m_get = resolve_method(ASM, CLASS_REQ, "get_assetBundle")?;
        let bundle = unsafe { invoke_method_inst_0(m_get, req) };
        if bundle.is_null() {
            return Err(crate::Error::Hook("get_assetBundle returned null".into()));
        }

        Ok(Self {
            obj: unsafe { Il2CppObject::from_raw(bundle) },
        })
    }

    /// Load a single asset by name.
    ///
    /// `type_class` and `type_asm` identify the C# type of the asset
    /// (e.g. `"UnityEngine.Font"`, `"UnityEngine.TextRenderingModule.dll"`).
    ///
    /// Returns the first asset in the `allAssets` array, or an error if
    /// the array is empty.
    pub fn load_asset(&self, name: &str, type_class: &str, type_asm: &str) -> Result<Il2CppObject> {
        // 3. Create a dummy object to obtain System.Type.
        let font_class = Il2CppClass::find_with(type_class, type_asm)?;
        let dummy = font_class.new_object()?;
        let font_type: Il2CppObject = dummy.invoke::<Il2CppObject>("GetType", ())?;

        // 4. LoadAssetAsync_Internal(string, Type) — instance on AssetBundle
        let m_load = resolve_method(ASM, CLASS_AB, "LoadAssetAsync_Internal")?;
        let font_name = Il2CppString::new(name);
        let req = unsafe {
            invoke_method_inst_2(
                m_load,
                self.obj.raw_ptr(),
                font_name.as_ptr(),
                font_type.raw_ptr(),
            )
        };
        if req.is_null() {
            return Err(crate::Error::Hook("LoadAssetAsync returned null".into()));
        }

        // 5. get_allAssets() — instance on AssetBundleRequest
        let m_all = resolve_method(ASM, CLASS_AREQ, "get_allAssets")?;
        let all = unsafe { invoke_method_inst_0(m_all, req) };
        if all.is_null() {
            return Err(crate::Error::Hook("get_allAssets returned null".into()));
        }

        let arr = unsafe { Il2CppArray::<Il2CppObject>::from_raw(all) };
        arr.get(0)
            .ok_or_else(|| crate::Error::Hook("no asset in bundle".into()))
    }

    /// Access the underlying managed object.
    pub fn as_object(&self) -> &Il2CppObject {
        &self.obj
    }

    /// Consume the wrapper and return the raw object.
    pub fn into_object(self) -> Il2CppObject {
        self.obj
    }
}

// ── convenience function ──────────────────────────────────────────────────

/// Load a single asset from an in-memory AssetBundle.
///
/// Convenience wrapper around [`AssetBundle::load_from_memory`] +
/// [`AssetBundle::load_asset`].
///
/// ```ignore
/// let font = cleat::load_asset_from_memory(
///     &data,
///     "FGO-Main-Font-Mod",
///     "UnityEngine.Font",
///     "UnityEngine.TextRenderingModule.dll",
/// )?;
/// ```
pub fn load_asset_from_memory(
    data: &[u8],
    asset_name: &str,
    type_class: &str,
    type_asm: &str,
) -> Result<Il2CppObject> {
    AssetBundle::load_from_memory(data)?.load_asset(asset_name, type_class, type_asm)
}

#[cfg(test)]
mod tests {
    // AssetBundle methods require a live IL2CPP runtime, so we only test
    // that the module compiles correctly and constants are non-empty.

    use super::*;

    #[test]
    fn constants_non_empty() {
        assert!(!ASM.is_empty());
        assert!(!CLASS_AB.is_empty());
        assert!(!CLASS_REQ.is_empty());
        assert!(!CLASS_AREQ.is_empty());
    }
}
