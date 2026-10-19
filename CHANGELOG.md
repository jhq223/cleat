# Modification history

## 0.3.0 — unreleased

### Runtime and build

- Replace ShadowHook entirely with Frida Gum 17.24.3. Support Android ARM, ARM64, x86 and x86_64 with checked official devkit downloads and an offline devkit override.
- Implement the integration in Rust. Link the NDK compiler runtime explicitly for ARM instruction-cache flushing; no project C/C++ shim or BNM source is compiled.
- Synchronize `il2cpp-bridge-rs` to upstream `ae667562174392df7c8f8f15b042a8ed034b91e2`, then maintain it as a private path dependency. Remove Git and branch dependencies and track Cargo.lock.
- Fix bridge array-length and alignment ABI declarations, thread attachment ownership, assembly-scoped lookup, injected parameter metadata, and Unity 2023 GC handle width.
- Non-Android hook installation now returns an unsupported-platform error.

### API changes

- Prefer `Class`, `Object`, `get`, `set` and `call`. Search all assemblies, report ambiguous names/overloads, and accept Rust strings in fields, properties and calls.
- Add assemblies, properties/indexers, event accessors, rooted delegates, constructors, boxing/unboxing, generic specialization and virtual dispatch.
- Add `by_ref` and `out` arguments. Validate managed value shape and reference assignability before invocation or storage.
- Hooks have synchronized installation state, fallible `original`, panic/error boundaries, explicit overloads and a preserved hidden MethodInfo argument. Ref/out and value-type instance hooks remain unsupported by the macro.
- Add explicit GC roots and thread guards. Object wrappers remain unrooted; copying them does not retain managed objects.
- Replace list/dictionary private-memory access with managed collection methods. Collection operations and dictionary iteration are fallible. Remove dictionary entry-buffer access.
- Fix issue [#1](https://github.com/jhq223/cleat/issues/1): dictionary access no longer assumes Mono/CoreFX private layouts or treats Count as the last occupied entry. Regression coverage exercises distinct layouts, deleted slots and mutation during enumeration.
- Reference array writes use GC barriers and check the actual element type. Add bulk array construction and lossless UTF-16 string conversion.
- Use scalar-compatible Unity math layouts. Poll actual asynchronous AssetBundle requests on the main thread and retain their GC roots until completion; keep synchronous APIs for runtimes exposing them.
- Fix the managed-reference field-write ABI: pass reference values directly to IL2CPP, while passing value types by address. This fixes invalid references later reached by Unity asset garbage collection.
- Add rooted list snapshots for cross-frame traversal, the Unity 6 public API/GC-handle profile, and Android JavaVM/thread/local-frame helpers backed by the jni crate.
- Scene object searches now return `Result` and honor `include_inactive` or report an unsupported overload.

### Coroutines and registered classes

- Add frame-driven Rust futures using async-task/futures-util, cancellation, status reporting, reentrancy protection and prompt cleanup on shutdown.
- Add managed coroutine handles and version-specific Rust IEnumerator generators using genawaiter, including next-frame, fixed-update, end-of-frame, scaled/realtime waits and existing managed yield instructions.
- Add Rust class builders for fields, methods, inherited virtual overrides and interfaces in existing assemblies.
- Isolate internal metadata layouts behind mutually exclusive Unity 2019.4, 2020.3 (patch 20+), 2021.3, 2022.3 and 2023.1 features. Assert key ABI offsets at compilation and check the running version/layout before registration.
- BNM is a behavioral/ABI reference, not a source dependency. Existing-class layout mutation and new generic classes are not implemented.

See the API/build guides for current usage and verification limits.
