use napi::{Env, sys};
use std::ffi::c_void;

/// Trait implemented by finalizer data that needs to run when the JS
/// object is garbage-collected.
pub(crate) trait Finalize {
    fn finalize(&self, env: Env);
}

/// N-API finalizer trampoline. Reclaims the `Box<T>` and calls `T::finalize`.
pub unsafe extern "C" fn finalize_trampoline<T: Finalize>(
    env: sys::napi_env,
    finalize_data: *mut c_void,
    _finalize_hint: *mut c_void,
) {
    let data = unsafe { Box::from_raw(finalize_data as *mut T) };
    data.finalize(Env::from_raw(env));
}
