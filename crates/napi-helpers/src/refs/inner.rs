//! The shared core of a N-API reference: the `napi_ref` handle plus the
//! `napi_env` (stable for the addon's lifetime) needed to release it.
//!
//! The strength is a const generic: `RefInner<false>` (the default) holds a
//! pinned (strong, refcount 1) reference, `RefInner<true>` an unpinned
//! (weak, refcount 0) one. Those are the only two states a reference can be
//! in, and the state lives in the type.

use std::{ffi::c_void, mem::ManuallyDrop, ptr};

use napi::{Env, JsValue, Result, bindgen_prelude::Object, check_status, sys};

use crate::{Finalize, finalize_trampoline, native_log};

/// A `napi_ref` and the `napi_env` that owns it. Deleting the reference on
/// drop is the one cleanup every wrapper shares.
pub(crate) struct RefInner<const WEAK: bool = false> {
    inner: sys::napi_ref,
    env: sys::napi_env,
}

impl<const WEAK: bool> RefInner<WEAK> {
    pub fn new(env: &Env, obj: &Object) -> Result<RefInner<WEAK>> {
        unsafe { RefInner::from_raw(env.raw(), obj.raw()) }
    }

    /// Create a reference from raw handles with the initial refcount implied
    /// by `WEAK`.
    ///
    /// # Safety
    ///
    /// `env` must be a valid `napi_env` and `value` a valid JS value in that
    /// environment.
    pub unsafe fn from_raw(env: sys::napi_env, value: sys::napi_value) -> Result<RefInner<WEAK>> {
        let mut inner = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_create_reference(env, value, if WEAK { 0 } else { 1 }, &mut inner) },
            "RefInner: failed to create reference"
        )?;
        Ok(RefInner { inner, env })
    }

    /// Retrieve the referenced value as a raw handle.
    pub fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        let mut value = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_get_reference_value(env.raw(), self.inner, &mut value) },
            "RefInner: failed to get reference value"
        )?;
        Ok(value)
    }

    /// Retrieve the referenced object, or `None` if it has been collected.
    pub fn get_value<'env>(&self, env: &'env Env) -> Option<Object<'env>> {
        let mut value = ptr::null_mut();
        let status = unsafe { sys::napi_get_reference_value(env.raw(), self.inner, &mut value) };
        if status != sys::Status::napi_ok || value.is_null() {
            return None;
        }
        Some(Object::from_raw(env.raw(), value))
    }

    /// Whether the referenced object is still alive.
    pub fn is_alive(&self, env: &Env) -> bool {
        self.get_value(env).is_some()
    }

    /// Attach a finalizer to the referenced JS object. When V8 collects it,
    /// `data.finalize(env)` will be called. Ownership of `data` is
    /// transferred via `Box::into_raw`; the trampoline reclaims it before
    /// calling `finalize`.
    pub fn add_finalizer<T: Finalize + 'static>(&self, env: &Env, data: T) -> Result<()> {
        let obj = self.get_value(env).ok_or_else(|| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "RefInner::add_finalizer: target object already collected",
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
                "RefInner::add_finalizer: failed to register finalizer",
            ));
        }
        Ok(())
    }
}

impl RefInner<true> {
    /// Move this weak reference's handle into a strong `RefInner<false>`, via
    /// `napi_reference_ref` raising the refcount from 0 to 1. The handle is
    /// moved, not copied, so deletion still happens exactly once.
    pub fn upgrade(self, env: &Env) -> Result<RefInner<false>> {
        let this = ManuallyDrop::new(self);
        check_status!(
            unsafe { sys::napi_reference_ref(env.raw(), this.inner, ptr::null_mut()) },
            "RefInner::upgrade: failed to raise reference count"
        )?;
        Ok(RefInner {
            inner: this.inner,
            env: this.env,
        })
    }
}

impl RefInner<false> {
    /// Move this strong reference's handle into a weak `RefInner<true>`,
    /// via `napi_reference_unref` lowering the refcount from 1 to 0. The
    /// handle is moved, not copied, so deletion still happens exactly once.
    pub fn downgrade(self, env: &Env) -> Result<RefInner<true>> {
        let this = ManuallyDrop::new(self);
        check_status!(
            unsafe { sys::napi_reference_unref(env.raw(), this.inner, ptr::null_mut()) },
            "RefInner::downgrade: failed to lower reference count"
        )?;
        Ok(RefInner {
            inner: this.inner,
            env: this.env,
        })
    }
}

impl<const WEAK: bool> Drop for RefInner<WEAK> {
    fn drop(&mut self) {
        // Weak refs: the target may already be collected, so the handle can
        // be null by the time the reference is dropped; skip the delete in
        // that case. Strong refs keep the target alive, so the handle is
        // always valid - the `WEAK` branch is a compile-time constant.
        if WEAK && self.inner.is_null() {
            return;
        }
        // `napi_delete_reference` returns a raw status, not a `Result`, so
        // it cannot go through `discard_err!`; compare and log explicitly.
        let status = unsafe { sys::napi_delete_reference(self.env, self.inner) };
        if status != sys::Status::napi_ok {
            native_log!(
                "napi-blitz: RefInner: delete reference on drop failed with status {status}"
            );
        }
    }
}
