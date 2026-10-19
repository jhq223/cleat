//! Tuple arguments with Rust-string conversion and temporary GC roots.
use crate::{
    Error, GcHandle, Il2CppValueType, IntoIl2Cpp, Object, Result, bridge::structs::Method,
};
use std::ffi::c_void;

/// Method arguments: `()`, `(42,)`, `("text", true)`, up to 16 values.
pub trait Args: Sized {
    type Values: PreparedArgs;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn prepare(self) -> Result<PreparedArguments<Self::Values>>;
}

#[doc(hidden)]
pub struct PreparedArguments<T: PreparedArgs> {
    values: T,
    _roots: Vec<GcHandle>,
}
impl<T: PreparedArgs> PreparedArguments<T> {
    pub fn validate(&self, method: &Method) -> Result<()> {
        self.values.validate(method)
    }
    pub fn to_arg_ptrs(&mut self) -> Vec<*mut c_void> {
        self.values.to_arg_ptrs()
    }
}

/// # Safety
/// Returned pointers must point to live values matching the validated metadata.
/// They must remain valid while self is borrowed. Reference values use the
/// object pointer; value types use a pointer to their payload.
#[doc(hidden)]
pub unsafe trait PreparedArgs {
    fn validate(&self, method: &Method) -> Result<()>;
    fn to_arg_ptrs(&mut self) -> Vec<*mut c_void>;
}
impl Args for () {
    type Values = ();
    fn len(&self) -> usize {
        0
    }
    fn prepare(self) -> Result<PreparedArguments<()>> {
        Ok(PreparedArguments {
            values: (),
            _roots: vec![],
        })
    }
}
unsafe impl PreparedArgs for () {
    fn validate(&self, method: &Method) -> Result<()> {
        if method.args.is_empty() {
            Ok(())
        } else {
            Err(Error::Bridge("argument count mismatch".into()))
        }
    }
    fn to_arg_ptrs(&mut self) -> Vec<*mut c_void> {
        vec![]
    }
}

fn root<T: Il2CppValueType>(value: &T, roots: &mut Vec<GcHandle>) -> Result<()> {
    if T::IS_REFERENCE {
        let pointer = unsafe { value.to_arg_ptr() };
        if !pointer.is_null() {
            roots.push(GcHandle::pinned(unsafe { Object::from_raw(pointer) })?);
        }
    }
    Ok(())
}
/// Pass a mutable value to a C# ref parameter.
pub fn by_ref<T: Il2CppValueType>(value: &mut T) -> RefArg<'_, T> {
    RefArg {
        value,
        output: false,
    }
}
/// Pass a slot to a C# out parameter. The slot is initialized to its default.
pub fn out<T: Il2CppValueType + Default>(value: &mut T) -> RefArg<'_, T> {
    *value = T::default();
    RefArg {
        value,
        output: true,
    }
}
pub struct RefArg<'a, T> {
    value: &'a mut T,
    output: bool,
}

#[doc(hidden)]
pub trait IntoArgument {
    type Prepared: PreparedArgument;
    fn prepare_argument(self, roots: &mut Vec<GcHandle>) -> Result<Self::Prepared>;
}
/// # Safety
/// Pointers must match the metadata validated by validate and remain valid
/// until the prepared argument is dropped. A by-ref slot must be writable.
#[doc(hidden)]
pub unsafe trait PreparedArgument {
    fn validate(&self, ty: &crate::bridge::structs::Type) -> Result<()>;
    fn pointer(&mut self) -> *mut c_void;
}
#[doc(hidden)]
pub struct ValueArgument<T>(T);
impl<T: IntoIl2Cpp> IntoArgument for T {
    type Prepared = ValueArgument<T::Value>;
    fn prepare_argument(self, roots: &mut Vec<GcHandle>) -> Result<Self::Prepared> {
        let value = self.into_il2cpp()?;
        root(&value, roots)?;
        Ok(ValueArgument(value))
    }
}
unsafe impl<T: Il2CppValueType> PreparedArgument for ValueArgument<T> {
    fn validate(&self, ty: &crate::bridge::structs::Type) -> Result<()> {
        if ty.is_byref() {
            return Err(Error::Bridge(
                "ref/out parameter requires by_ref(&mut value) or out(&mut value)".into(),
            ));
        }
        self.0.validate_value(ty)
    }
    fn pointer(&mut self) -> *mut c_void {
        unsafe { self.0.to_arg_ptr() }
    }
}
impl<'a, T: Il2CppValueType> IntoArgument for RefArg<'a, T> {
    type Prepared = Self;
    fn prepare_argument(self, roots: &mut Vec<GcHandle>) -> Result<Self> {
        if !self.output {
            root(self.value, roots)?;
        }
        Ok(self)
    }
}
unsafe impl<T: Il2CppValueType> PreparedArgument for RefArg<'_, T> {
    fn validate(&self, ty: &crate::bridge::structs::Type) -> Result<()> {
        if !ty.is_byref() {
            return Err(Error::Bridge(
                "by_ref/out used for a value parameter".into(),
            ));
        }
        if self.output {
            T::validate_type(ty)
        } else {
            self.value.validate_value(ty)
        }
    }
    fn pointer(&mut self) -> *mut c_void {
        (self.value as *mut T).cast()
    }
}
macro_rules! impl_args {
    ($($T:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($T: IntoArgument),+> Args for ($($T,)+) {
            type Values = ($($T::Prepared,)+);
            fn len(&self) -> usize { [$(stringify!($T)),+].len() }
            fn prepare(self) -> Result<PreparedArguments<Self::Values>> {
                let ($($T,)+) = self;
                let mut roots = Vec::new();
                $(let $T = $T.prepare_argument(&mut roots)?;)+
                Ok(PreparedArguments { values: ($($T,)+), _roots: roots })
            }
        }
        #[allow(non_snake_case)]
        unsafe impl<$($T: PreparedArgument),+> PreparedArgs for ($($T,)+) {
            fn validate(&self, method: &Method) -> Result<()> {
                if [$(stringify!($T)),+].len() != method.args.len() { return Err(Error::Bridge("argument count mismatch".into())); }
                let ($($T,)+) = self;
                let mut types = method.args.iter();
                $(
                    let ty = &types.next().unwrap().type_info;
                    $T.validate(ty)?;
                )+
                Ok(())
            }
            fn to_arg_ptrs(&mut self) -> Vec<*mut c_void> {
                let ($($T,)+) = self;
                vec![$($T.pointer()),+]
            }
        }
    };
}
impl_args!(A);
impl_args!(A, B);
impl_args!(A, B, C);
impl_args!(A, B, C, D);
impl_args!(A, B, C, D, E);
impl_args!(A, B, C, D, E, F);
impl_args!(A, B, C, D, E, F, G);
impl_args!(A, B, C, D, E, F, G, H);
impl_args!(A, B, C, D, E, F, G, H, I);
impl_args!(A, B, C, D, E, F, G, H, I, J);
impl_args!(A, B, C, D, E, F, G, H, I, J, K);
impl_args!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_args!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_args!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_args!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_args!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
