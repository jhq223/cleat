#[cfg(target_os = "android")]
mod guard;
#[cfg(target_os = "android")]
mod readiness;
#[cfg(target_os = "android")]
pub(crate) use readiness::RuntimeObserver;
#[cfg(not(target_os = "android"))]
mod stub;

#[cfg(target_os = "android")]
pub use guard::HookGuard;
#[cfg(all(target_os = "android", feature = "class-injection"))]
pub(crate) use guard::initialize;
#[cfg(not(target_os = "android"))]
pub use stub::HookGuard;
mod slot;
pub use slot::{HookInvocation, HookSlot};
