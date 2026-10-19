//! Synchronous AssetBundle operations. Call from Unity's main thread.
use crate::{Class, GcHandle, Il2CppArray, Object, Result};

pub struct AssetBundle {
    object: Object,
    _root: GcHandle,
}
impl AssetBundle {
    /// Load asynchronously, yielding to the cleat coroutine driver each frame.
    /// Poll this future on Unity's main thread.
    pub async fn load_from_memory_async(data: &[u8]) -> Result<Self> {
        let (_bytes, request) = {
            let array = Il2CppArray::<u8>::from_slice(&Class::find("System.Byte")?, data)?;
            let bytes = array.as_object().root()?;
            let class = Class::find("UnityEngine.AssetBundle")?;
            let request = match class.method(("LoadFromMemoryAsync", ["System.Byte[]"])) {
                Ok(method) => method.invoke((array,))?,
                Err(crate::Error::MethodNotFound(_)) => class.call(
                    (
                        "LoadFromMemoryAsync_Internal",
                        ["System.Byte[]", "System.UInt32"],
                    ),
                    (array, 0u32),
                )?,
                Err(error) => return Err(error),
            };
            (bytes, request)
        };
        let request = wait_for(request).await?;
        Self::from_object(request.object().get("assetBundle")?)
    }

    /// Load an asset asynchronously. The bundle must remain loaded until this
    /// future completes; poll it on Unity's main thread.
    pub async fn load_asset_async(&self, name: &str, type_class: &str) -> Result<Object> {
        self.load_asset_as_async(name, &Class::find(type_class)?)
            .await
    }

    pub async fn load_asset_as_async(&self, name: &str, class: &Class) -> Result<Object> {
        let request = self.object.call(
            ("LoadAssetAsync", ["System.String", "System.Type"]),
            (name, class.type_object()?),
        )?;
        let request = wait_for(request).await?;
        let object: Object = request.object().get("asset")?;
        if object.is_null() {
            return Err(crate::Error::NullObject);
        }
        Ok(object)
    }
    /// Load a bundle directly from disk on Unity's main thread.
    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::Error::Bridge("asset bundle path is not UTF-8".into()))?;
        let class = Class::find("UnityEngine.AssetBundle")?;
        let object = match class.method(("LoadFromFile", ["System.String"])) {
            Ok(method) => method.invoke((path,))?,
            Err(crate::Error::MethodNotFound(_)) => class.call(
                (
                    "LoadFromFile_Internal",
                    ["System.String", "System.UInt32", "System.UInt64"],
                ),
                (path, 0u32, 0u64),
            )?,
            Err(error) => return Err(error),
        };
        Self::from_object(object)
    }
    pub fn load_from_memory(data: &[u8]) -> Result<Self> {
        let bytes = Class::find("System.Byte")?;
        let array = Il2CppArray::<u8>::from_slice(&bytes, data)?;
        let class = Class::find("UnityEngine.AssetBundle")?;
        let object = match class.method(("LoadFromMemory", ["System.Byte[]"])) {
            Ok(method) => method.invoke((array,))?,
            Err(crate::Error::MethodNotFound(_)) => class.call(
                (
                    "LoadFromMemory_Internal",
                    ["System.Byte[]", "System.UInt32"],
                ),
                (array, 0u32),
            )?,
            Err(error) => return Err(error),
        };
        Self::from_object(object)
    }
    pub fn from_object(object: Object) -> Result<Self> {
        if !object.is_instance_of(&Class::find("UnityEngine.AssetBundle")?)? {
            return Err(crate::Error::Bridge("object is not an AssetBundle".into()));
        }
        Ok(Self {
            object,
            _root: GcHandle::pinned(object)?,
        })
    }
    /// Load an asset by name and its managed type, e.g. `UnityEngine.Font`.
    pub fn load_asset(&self, name: &str, type_class: &str) -> Result<Object> {
        self.load_asset_as(name, &Class::find(type_class)?)
    }
    pub fn load_asset_as(&self, name: &str, class: &Class) -> Result<Object> {
        self.object.call(
            ("LoadAsset", ["System.String", "System.Type"]),
            (name, class.type_object()?),
        )
    }
    pub fn contains(&self, name: &str) -> Result<bool> {
        self.object.call("Contains", (name,))
    }
    pub fn unload(&self, unload_loaded_objects: bool) -> Result<()> {
        self.object.call("Unload", (unload_loaded_objects,))
    }
    pub fn as_object(&self) -> &Object {
        &self.object
    }
    pub fn into_object(self) -> Object {
        self.object
    }
}

async fn wait_for(request: Object) -> Result<GcHandle> {
    let root = request.root()?;
    while !root.object().get::<bool>("isDone")? {
        crate::coroutine::next_frame().await;
    }
    Ok(root)
}

pub fn load_asset_from_memory(
    data: &[u8],
    name: &str,
    type_class: &str,
    type_assembly: &str,
) -> Result<Object> {
    AssetBundle::load_from_memory(data)?
        .load_asset_as(name, &Class::find_in(type_assembly, type_class)?)
}
