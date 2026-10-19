use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf, time::Duration};

const VERSION: &str = "17.24.3";

fn main() {
    let profiles = [
        ("UNITY_2019_4", "194"),
        ("UNITY_2020_3", "203"),
        ("UNITY_2021_3", "213"),
        ("UNITY_2022_3", "222"),
        ("UNITY_2023_1", "231"),
        ("UNITY_6000", "6000"),
    ];
    let selected: Vec<_> = profiles
        .iter()
        .filter(|(name, _)| env::var_os(format!("CARGO_FEATURE_{name}")).is_some())
        .collect();
    let injection = env::var_os("CARGO_FEATURE_CLASS_INJECTION").is_some();
    assert!(
        selected.len() <= 1,
        "Unity version features are mutually exclusive; select exactly one unity-* feature"
    );
    assert!(
        !injection || selected.len() == 1,
        "class-injection requires exactly one unity-* version feature"
    );
    assert!(
        !injection || env::var_os("CARGO_FEATURE_UNITY_6000").is_none(),
        "unity-6000 supports public IL2CPP APIs; a validated Unity 6 class-injection layout is not yet available"
    );
    println!("cargo:rerun-if-env-changed=CLEAT_GUM_DEVKIT");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let (arch, digest) = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => (
            "arm64",
            "7430d42f12711d005b645becf6646627730c69b17eb708664ef84dc3dfd23841",
        ),
        "arm" => (
            "arm",
            "2d7121d3590257aef72f9f04d09aaa3c2f6b43b2720905d93c3fd192d1be1cbe",
        ),
        "x86" => (
            "x86",
            "8eca1f7c98c550726a84bc7468f4bc012ea4bfcd779c3bb0b60f439f70bab857",
        ),
        "x86_64" => (
            "x86_64",
            "114b888240ff30a750fd60e770474804d3a02eba5ad7a4819d44112dca2b3e8a",
        ),
        other => panic!("Frida Gum is not configured for Android {other}"),
    };
    let devkit = match env::var_os("CLEAT_GUM_DEVKIT") {
        Some(path) => PathBuf::from(path),
        None => {
            let out = PathBuf::from(env::var_os("OUT_DIR").unwrap())
                .join(format!("gum-{VERSION}-{arch}"));
            let marker = out.join(".complete");
            if !marker.is_file() {
                let asset = format!("frida-gum-devkit-{VERSION}-android-{arch}.tar.xz");
                let url =
                    format!("https://github.com/frida/frida/releases/download/{VERSION}/{asset}");
                eprintln!("Downloading Frida Gum {VERSION} for Android {arch}");
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(180))
                    .build()
                    .unwrap();
                let bytes = client.get(url).send().and_then(|r| r.error_for_status())
                    .and_then(|r| r.bytes()).expect("Gum download failed; set CLEAT_GUM_DEVKIT to an extracted official devkit for offline builds");
                let actual = Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                assert_eq!(actual, digest, "Frida devkit checksum mismatch");
                fs::create_dir_all(&out).unwrap();
                let decoder = lzma_rust2::XzReader::new(Cursor::new(bytes), true);
                tar::Archive::new(decoder)
                    .unpack(&out)
                    .expect("could not extract Gum devkit");
                fs::write(marker, digest).unwrap();
            }
            out
        }
    };
    assert!(
        devkit.join("frida-gum.h").is_file() && devkit.join("libfrida-gum.a").is_file(),
        "CLEAT_GUM_DEVKIT must contain frida-gum.h and libfrida-gum.a from Frida {VERSION} for Android {arch}"
    );
    println!(
        "cargo:rerun-if-changed={}",
        devkit.join("frida-gum.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        devkit.join("libfrida-gum.a").display()
    );
    println!("cargo:rustc-link-search=native={}", devkit.display());
    println!("cargo:rustc-link-lib=static=frida-gum");
    // Rust links with -nodefaultlibs. Gum's ARM instruction-cache flush uses
    // __clear_cache from the NDK compiler runtime, which would otherwise stay
    // unresolved in a cdylib and fail only when Android loads it. cc is used
    // solely to discover the configured cross compiler; no C/C++ is compiled.
    let output = cc::Build::new()
        .get_compiler()
        .to_command()
        .arg("-print-libgcc-file-name")
        .output()
        .expect("query NDK compiler runtime");
    assert!(
        output.status.success(),
        "cannot locate NDK compiler runtime"
    );
    let builtins = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    assert!(
        builtins.is_file(),
        "NDK compiler runtime does not exist: {}",
        builtins.display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        builtins.parent().unwrap().display()
    );
    let library = builtins
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("lib")
        .unwrap();
    println!("cargo:rustc-link-lib=static={library}");
    for library in ["log", "m", "atomic", "dl"] {
        println!("cargo:rustc-link-lib={library}");
    }
}
