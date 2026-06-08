//! Conversions between Rust values and IL2CPP FFI pointers.
//!
//! See [`Args`] and [`Il2CppValueType`](super::Il2CppValueType).

use std::ffi::c_void;

use crate::Il2CppValueType;

/// Converts a tuple of arguments into an array of FFI raw pointers.
///
/// Pointers returned by [`to_arg_ptrs`](Args::to_arg_ptrs) are transient —
/// consume them immediately in an IL2CPP method call and drop.
pub unsafe trait Args {
    /// How many arguments in the tuple.
    fn len(&self) -> usize;

    /// True when there are no arguments.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pack each argument as `*mut c_void`.
    fn to_arg_ptrs(&self) -> Vec<*mut c_void>;
}

unsafe impl Args for () {
    fn len(&self) -> usize {
        0
    }
    fn to_arg_ptrs(&self) -> Vec<*mut c_void> {
        Vec::new()
    }
}

// ── tuple impls: 1 through 16 elements ────────────────────────────────────

macro_rules! impl_args_tuple {
    ($($T:ident),+) => {
        #[allow(non_snake_case)]
        unsafe impl<$($T: Il2CppValueType),*> Args for ($($T,)*) {
            fn len(&self) -> usize {
                let ($($T,)*) = self;
                let mut n = 0;
                $(let _ = $T; n += 1;)*
                n
            }

            fn to_arg_ptrs(&self) -> Vec<*mut c_void> {
                let ($($T,)*) = self;
                vec![$(unsafe { $T.to_arg_ptr() }),*]
            }
        }
    };
}

impl_args_tuple!(A);
impl_args_tuple!(A, B);
impl_args_tuple!(A, B, C);
impl_args_tuple!(A, B, C, D);
impl_args_tuple!(A, B, C, D, E);
impl_args_tuple!(A, B, C, D, E, F);
impl_args_tuple!(A, B, C, D, E, F, G);
impl_args_tuple!(A, B, C, D, E, F, G, H);
impl_args_tuple!(A, B, C, D, E, F, G, H, I);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_args_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
