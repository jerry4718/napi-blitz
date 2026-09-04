//! A pending-JS-promise handle: the promise is created up front and
//! settled later on the JS thread (`FontFace.loaded`, `FontFaceSet.ready`).

use std::{cell::Cell, ptr};

use napi::{Env, Error, JsError, Result, bindgen_prelude::ToNapiValue, check_status, sys};

use crate::anything::{Anything, OtherRef};

/// A JS promise created pending, settleable exactly once. Settling twice is
/// a no-op, matching the JS semantics of `resolve`/`reject` on a deferred.
pub struct Deferred {
    promise: OtherRef,
    deferred: Cell<Option<sys::napi_deferred>>,
}

impl Deferred {
    /// Create a pending promise together with its deferred handle.
    pub fn new(env: &Env) -> Result<Self> {
        let mut deferred = ptr::null_mut();
        let mut promise = ptr::null_mut();
        check_status!(unsafe { sys::napi_create_promise(env.raw(), &mut deferred, &mut promise) })?;
        Ok(Self {
            promise: unsafe { OtherRef::new(env.raw(), promise)? },
            deferred: Cell::new(Some(deferred)),
        })
    }

    /// The promise value, wrapped for return to JS.
    pub fn value(&self) -> Anything {
        Anything::Object(self.promise.clone())
    }

    /// Settle the promise with a JS value (`raw`).
    ///
    /// `raw` is a `napi_value` handle from a trusted caller; it is only
    /// forwarded to the napi layer, not dereferenced here.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn resolve(&self, env: &Env, raw: sys::napi_value) -> Result<()> {
        let Some(deferred) = self.deferred.take() else {
            return Ok(());
        };
        check_status!(unsafe { sys::napi_resolve_deferred(env.raw(), deferred, raw) })
    }

    /// Settle the promise as rejected with `error`.
    pub fn reject(&self, env: &Env, error: Error) -> Result<()> {
        let Some(deferred) = self.deferred.take() else {
            return Ok(());
        };
        let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), JsError::from(error))? };
        check_status!(unsafe { sys::napi_reject_deferred(env.raw(), deferred, raw) })
    }
}
