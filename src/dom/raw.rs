//! Centralized unsafe napi-sys wrappers.
//!
//! All direct `sys::napi_*` calls that involve `unsafe` live here so they
//! can be audited and converged to safe napi-rs APIs in one place. Each
//! function has a `// SAFETY:` comment and a safe signature.
//!
//! **Do not add `use crate::sys::*` outside this module.**

use std::ffi::c_void;
use std::ptr;

use napi::{bindgen_prelude::*, sys, Result};

// ── Function identity ───────────────────────────────────────────────

/// Test whether two `napi_value`s refer to the same JS object (===).
///
/// Wrapper around `napi_strict_equals`. The safe `Env::strict_equals` API
/// exists but requires `JsValue<'env>` bounds that are awkward to satisfy
/// when storing raw `napi_value` inside `FunctionRef`. This helper accepts
/// raw values directly.
///
/// SAFETY: `env`, `a`, `b` must be valid for the duration of the call.
pub fn strict_equals(env: sys::napi_env, a: sys::napi_value, b: sys::napi_value) -> Result<bool> {
    let mut result = false;
    check_status!(
        unsafe { sys::napi_strict_equals(env, a, b, &mut result) },
        "napi_strict_equals failed"
    )?;
    Ok(result)
}

// ── Reference refcount management ───────────────────────────────────

/// Increment the refcount of a `napi_ref`.
///
/// SAFETY: `env` and `napi_ref` must be valid.
pub fn reference_ref(env: sys::napi_env, napi_ref: sys::napi_ref) -> Result<u32> {
    let mut count = 0u32;
    check_status!(
        unsafe { sys::napi_reference_ref(env, napi_ref, &mut count) },
        "napi_reference_ref failed"
    )?;
    Ok(count)
}

/// Decrement the refcount of a `napi_ref`.
///
/// SAFETY: `env` and `napi_ref` must be valid.
pub fn reference_unref(env: sys::napi_env, napi_ref: sys::napi_ref) -> Result<u32> {
    let mut count = 0u32;
    check_status!(
        unsafe { sys::napi_reference_unref(env, napi_ref, &mut count) },
        "napi_reference_unref failed"
    )?;
    Ok(count)
}

/// Create a `napi_ref` with the given initial refcount.
///
/// `refcount = 0` produces a weak reference (does not prevent GC).
/// `refcount = 1` produces a strong reference.
///
/// SAFETY: `env` and `value` must be valid.
pub fn create_reference(
    env: sys::napi_env,
    value: sys::napi_value,
    initial_refcount: u32,
) -> Result<sys::napi_ref> {
    let mut napi_ref = ptr::null_mut();
    check_status!(
        unsafe { sys::napi_create_reference(env, value, initial_refcount, &mut napi_ref) },
        "napi_create_reference failed"
    )?;
    Ok(napi_ref)
}

/// Delete a `napi_ref`, releasing the reference.
///
/// SAFETY: `env` and `napi_ref` must be valid. The ref must not be used
/// after this call.
pub fn delete_reference(env: sys::napi_env, napi_ref: sys::napi_ref) -> Result<()> {
    check_status!(
        unsafe { sys::napi_delete_reference(env, napi_ref) },
        "napi_delete_reference failed"
    )?;
    Ok(())
}

/// Retrieve the `napi_value` pointed to by a `napi_ref`.
///
/// Returns `None` when the referenced object has been garbage-collected
/// (only possible for refs with refcount 0, i.e. weak refs).
///
/// SAFETY: `env` and `napi_ref` must be valid.
pub fn get_reference_value(
    env: sys::napi_env,
    napi_ref: sys::napi_ref,
) -> Result<Option<sys::napi_value>> {
    let mut value = ptr::null_mut();
    check_status!(
        unsafe { sys::napi_get_reference_value(env, napi_ref, &mut value) },
        "napi_get_reference_value failed"
    )?;
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

// ── Value utilities ─────────────────────────────────────────────────

/// Get the `undefined` JS value.
pub fn get_undefined(env: sys::napi_env) -> Result<sys::napi_value> {
    let mut value = ptr::null_mut();
    check_status!(
        unsafe { sys::napi_get_undefined(env, &mut value) },
        "napi_get_undefined failed"
    )?;
    Ok(value)
}

/// Call a JS function with `this=undefined` and the given arguments.
///
/// SAFETY: `env`, `func`, and all values in `args` must be valid.
pub fn call_function(
    env: sys::napi_env,
    func: sys::napi_value,
    args: &[sys::napi_value],
) -> Result<sys::napi_value> {
    let undefined = get_undefined(env)?;
    let mut return_value = ptr::null_mut();
    check_status!(
        unsafe {
            sys::napi_call_function(
                env,
                undefined,
                func,
                args.len(),
                args.as_ptr(),
                &mut return_value,
            )
        },
        "napi_call_function failed"
    )?;
    Ok(return_value)
}

// ── Finalizer registration ──────────────────────────────────────────

/// Type alias for the napi finalize callback.
pub type FinalizeCb = unsafe extern "C" fn(
    env: sys::napi_env,
    finalize_data: *mut c_void,
    finalize_hint: *mut c_void,
);

/// Register a finalizer on a JS object. The callback fires when V8 GCs
/// the object.
///
/// SAFETY: `env` and `js_object` must be valid. `finalize_cb` must be a
/// valid extern "C" function pointer. The `finalize_data` and
/// `finalize_hint` pointers are passed back to the callback; ownership
/// transfers to the callback.
///
/// Requires napi5+. Our project uses napi6.
pub fn add_finalizer(
    env: sys::napi_env,
    js_object: sys::napi_value,
    finalize_data: *mut c_void,
    finalize_cb: Option<FinalizeCb>,
    finalize_hint: *mut c_void,
) -> Result<sys::napi_ref> {
    let mut napi_ref = ptr::null_mut();
    check_status!(
        unsafe {
            sys::napi_add_finalizer(
                env,
                js_object,
                finalize_data,
                finalize_cb,
                finalize_hint,
                &mut napi_ref,
            )
        },
        "napi_add_finalizer failed"
    )?;
    Ok(napi_ref)
}
