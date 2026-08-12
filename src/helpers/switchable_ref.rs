//! A reference whose strength can be toggled between strong and weak at
//! runtime.
//!
//! Backed by a single `napi_ref` whose refcount is switched between 1
//! (strong, prevents GC) and 0 (weak, allows GC). Used by `NodeCache` to
//! keep in-document nodes alive while letting detached nodes be collected.
//!
//! All `sys::napi_*` unsafe calls are concentrated here.

use std::{ffi::c_void, ptr};

use napi::{Env, JsValue, Result, bindgen_prelude::Object, check_status, sys};

use crate::helpers::{Finalize, finalize_trampoline};

/// A reference to a JS object whose strength can be switched at runtime.
///
/// - `make_strong` sets refcount to 1 (object cannot be GC'd).
/// - `make_weak` sets refcount to 0 (object can be GC'd).
/// - `get_value` retrieves the object if still alive.
/// - `add_finalizer` attaches a callback that fires when the object is
///   collected (only possible while in weak mode).
pub(crate) struct SwitchableRef {
    inner: sys::napi_ref,
    env: sys::napi_env,
    strong: bool,
}

impl SwitchableRef {
    /// Create a new reference with the given initial strength.
    ///
    /// `strong = true` -> refcount 1, `strong = false` -> refcount 0.
    pub(crate) fn new(obj: &Object, env: &Env, strong: bool) -> Result<Self> {
        let mut inner = ptr::null_mut();
        let initial_count: u32 = if strong { 1 } else { 0 };
        check_status!(
            unsafe { sys::napi_create_reference(env.raw(), obj.raw(), initial_count, &mut inner) },
            "SwitchableRef: failed to create reference"
        )?;
        Ok(Self {
            inner,
            env: env.raw(),
            strong,
        })
    }

    /// Retrieve the JS object. Returns `None` if it has been collected
    /// (only possible in weak mode).
    pub(crate) fn get_value<'env>(&self, env: &'env Env) -> Option<Object<'env>> {
        let mut value = ptr::null_mut();
        let status = unsafe { sys::napi_get_reference_value(env.raw(), self.inner, &mut value) };
        if status != sys::Status::napi_ok || value.is_null() {
            return None;
        }
        Some(Object::from_raw(env.raw(), value))
    }

    /// Whether the object is still alive.
    pub(crate) fn is_alive(&self, env: &Env) -> bool {
        self.get_value(env).is_some()
    }

    /// Whether the reference is currently strong.
    #[allow(unused)]
    pub(crate) fn is_strong(&self) -> bool {
        self.strong
    }

    /// Switch to strong (refcount 1). No-op if already strong.
    pub(crate) fn make_strong(&mut self, env: &Env) -> Result<()> {
        if self.strong {
            return Ok(());
        }
        let mut result_count: u32 = 0;
        check_status!(
            unsafe { sys::napi_reference_ref(env.raw(), self.inner, &mut result_count) },
            "SwitchableRef::make_strong: napi_reference_ref failed"
        )?;
        self.strong = true;
        Ok(())
    }

    /// Switch to weak (refcount 0). No-op if already weak.
    pub(crate) fn make_weak(&mut self, env: &Env) -> Result<()> {
        if !self.strong {
            return Ok(());
        }
        let mut result_count: u32 = 0;
        check_status!(
            unsafe { sys::napi_reference_unref(env.raw(), self.inner, &mut result_count) },
            "SwitchableRef::make_weak: napi_reference_unref failed"
        )?;
        self.strong = false;
        Ok(())
    }

    /// Attach a finalizer to the referenced JS object. When V8 collects
    /// it (only possible while in weak mode), `data.finalize(env)` will
    /// be called.
    pub(crate) fn add_finalizer<T: Finalize + 'static>(&self, env: &Env, data: T) -> Result<()> {
        let obj = self.get_value(env).ok_or_else(|| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "SwitchableRef::add_finalizer: target object already collected",
            )
        })?;
        let boxed = Box::new(data);
        let raw = Box::into_raw(boxed);
        let status = unsafe {
            sys::napi_add_finalizer(
                env.raw(),
                obj.raw(),
                raw as *mut c_void,
                Some(finalize_trampoline::<T>),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if status != sys::Status::napi_ok {
            let _ = unsafe { Box::from_raw(raw) };
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                "SwitchableRef::add_finalizer: failed to register finalizer",
            ));
        }
        Ok(())
    }
}

impl Drop for SwitchableRef {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let _ = unsafe { sys::napi_delete_reference(self.env, self.inner) };
            self.inner = ptr::null_mut();
        }
    }
}
