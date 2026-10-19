//! Diagnostic mod: write metadata after the runtime is ready, without gameplay changes.
use cleat::prelude::*;

#[cleat::main]
fn probe() -> Result<()> {
    cleat::__log::info!(
        "cleat probe: runtime ready, {} assemblies",
        Assembly::all().len()
    );
    let base = "/storage/emulated/0/Android/data/com.aniplex.fategrandorder/files/Mod";
    std::fs::create_dir_all(base).map_err(|error| Error::Bridge(error.to_string()))?;
    let assembly = cleat::bridge::api::cache::assembly("Assembly-CSharp")
        .ok_or_else(|| Error::AssemblyNotFound("Assembly-CSharp".into()))?;
    std::fs::write(format!("{base}/cleat-metadata.cs"), assembly.to_string())
        .map_err(|error| Error::Bridge(error.to_string()))?;
    cleat::__log::info!("cleat probe: metadata saved");
    Ok(())
}
