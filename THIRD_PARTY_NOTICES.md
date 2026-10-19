# Third-party components

cleat and cleat-macros are MIT licensed. The internal bridge retains its
original MIT license and [import provenance](crates/il2cpp-bridge-rs/UPSTREAM.md).
Rust dependency versions are recorded in Cargo.lock; their own license terms
continue to apply.

Android builds statically link the official **Frida Gum 17.24.3** devkit from
[Frida releases](https://github.com/frida/frida/releases/tag/17.24.3).
Its upstream [COPYING](licenses/frida-gum-COPYING) is preserved here, including
the wxWindows Library Licence and notices for incorporated portions. Consult
the [Gum source distribution](https://github.com/frida/frida-gum/tree/17.24.3)
for notices applicable to bundled native dependencies when distributing binaries.

[BNM-Android](https://github.com/ByNameModding/BNM-Android) is a reference for
capabilities and IL2CPP ABI evaluation. cleat does not vendor, compile or link
BNM code. Version-specific metadata structures and runtime integration are
implemented in Rust within this repository.
