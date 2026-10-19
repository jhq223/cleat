# Internal bridge provenance

- Original repository: https://github.com/Batchhh/il2cpp-bridge-rs
- Baseline commit: `ae667562174392df7c8f8f15b042a8ed034b91e2`
- Imported: 2026-10-19
- Original license: MIT, retained in [LICENSE](LICENSE)

This is an internal, locally maintained crate (`publish = false`). cleat uses a
workspace path dependency. Building or updating cleat does not fetch a bridge
Git repository or track an upstream branch.

The baseline contains the optional IL2CPP export resolution changes previously
carried in the jhq223 fork. Local changes cover native argument widths, GC
handle types, thread attachment ownership, array/string and field handling,
assembly-scoped lookup, injected metadata, and scalar Unity math layouts.
The root modification history records user-visible changes.
