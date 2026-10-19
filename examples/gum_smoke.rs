//! Android executable: exercises Gum without needing a Unity game.
use cleat::{HookGuard, Result};
use std::{
    ffi::c_void,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
};

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);

#[inline(never)]
extern "C" fn target(value: i32) -> i32 {
    black_box(value).wrapping_add(7)
}
#[inline(never)]
extern "C" fn replacement(value: i32) -> i32 {
    let original: extern "C" fn(i32) -> i32 =
        unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    original(value).wrapping_mul(3)
}
fn call(value: i32) -> i32 {
    black_box(target as extern "C" fn(i32) -> i32)(black_box(value))
}
fn main() -> Result<()> {
    assert_eq!(call(5), 12);
    for cycle in 0..20 {
        let guard =
            unsafe { HookGuard::install(target as *const c_void, replacement as *const c_void)? };
        ORIGINAL.store(guard.trampoline() as usize, Ordering::Release);
        assert_eq!(call(5), 36, "replacement cycle {cycle}");
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    for value in 0..1000 {
                        assert_eq!(call(value), (value + 7) * 3);
                    }
                });
            }
        });
        assert!(
            unsafe { HookGuard::install(target as *const c_void, replacement as *const c_void) }
                .is_err()
        );
        guard.uninstall()?;
        assert_eq!(call(5), 12, "revert cycle {cycle}");
    }
    println!(
        "PASS: Gum replacement, original trampoline, concurrent calls, duplicate rejection, uninstall and reinstall (20 cycles)"
    );
    Ok(())
}
