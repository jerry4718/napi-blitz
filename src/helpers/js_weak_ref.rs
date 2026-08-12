//! A refcount-0 N-API reference that does not keep its JS object alive.
//!
//! napi-rs `ObjectRef` always creates a strong reference (refcount ≥ 1),
//! so the weak-reference operations are kept in this small wrapper.
//!
//! All `sys::napi_*` unsafe calls and `extern "C"` trampolines are
//! concentrated here.

use std::{ffi::c_void, ptr};

use crate::helpers::{Finalize, finalize_trampoline};
use napi::{Env, JsValue, Result, bindgen_prelude::Object, check_status, sys};

/// A weak reference to a JS object.
///
/// Created via [`JsWeakRef::new`] with an initial refcount of **0**,
/// meaning it does not prevent V8 from garbage-collecting the target.
/// Use [`JsWeakRef::get_value`] to probe whether the object is still alive.
pub(crate) struct JsWeakRef {
    inner: sys::napi_ref,
    env: sys::napi_env,
}

impl JsWeakRef {
    /// Create a weak reference to `obj`.
    pub(crate) fn new(obj: &Object, env: &Env) -> Result<Self> {
        let mut inner = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_create_reference(env.raw(), obj.raw(), 0, &mut inner) },
            "JsWeakRef: failed to create weak reference"
        )?;
        Ok(Self {
            inner,
            env: env.raw(),
        })
    }

    /// Try to retrieve the JS object. Returns `None` if it has been
    /// garbage-collected.
    pub(crate) fn get_value<'env>(&self, env: &'env Env) -> Option<Object<'env>> {
        let mut value = ptr::null_mut();
        let status = unsafe { sys::napi_get_reference_value(env.raw(), self.inner, &mut value) };
        if status != sys::Status::napi_ok || value.is_null() {
            return None;
        }
        Some(Object::from_raw(env.raw(), value))
    }

    /// Whether the JS object is still alive (not yet collected).
    #[allow(unused)]
    pub(crate) fn is_alive(&self, env: &Env) -> bool {
        self.get_value(env).is_some()
    }

    /// Attach a finalizer to the referenced JS object. When V8 collects
    /// it, `data.finalize(env)` will be called. Ownership of `data` is
    /// transferred via `Box::into_raw`; the trampoline reclaims it before
    /// calling `finalize`.
    #[allow(unused)]
    pub(crate) fn add_finalizer<T: Finalize + 'static>(&self, env: &Env, data: T) -> Result<()> {
        let obj = self.get_value(env).ok_or_else(|| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "JsWeakRef::add_finalizer: target object already collected",
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
            // Reclaim the Box if registration failed.
            let _ = unsafe { Box::from_raw(raw) };
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                "JsWeakRef::add_finalizer: failed to register finalizer",
            ));
        }
        Ok(())
    }
}

impl Drop for JsWeakRef {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let _ = unsafe { sys::napi_delete_reference(self.env, self.inner) };
            self.inner = ptr::null_mut();
        }
    }
}
