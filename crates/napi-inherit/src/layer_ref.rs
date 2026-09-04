//! A typed handle to a JS object of a specific layer class.
//!
//! A method that returns one of its own layers' JS objects (a node, an
//! element, the document, ...) declares `LayerRef<L>` as its return type:
//! the runtime hands back the exact object that already carries `L`'s own
//! block, so instance identity is preserved (no re-wrapping). `L` only
//! fixes the declared class at compile time - no layer data is copied. The
//! macro maps `LayerRef<L>` to the layer's JS class name in the TS defs.

use std::marker::PhantomData;
use std::ptr;

use napi::{
    Env, JsValue, Result,
    bindgen_prelude::{FromNapiValue, Object, ToNapiValue},
    check_status, sys,
};

use crate::layer::ExtendLayer;

/// A strong reference to a JS object whose own-data chain carries layer `L`.
///
/// Converted back into a JS value with [`ToNapiValue`] (which releases the
/// reference); if it is dropped unconverted the reference is released too,
/// so a `LayerRef` never leaks.
pub struct LayerRef<L: ExtendLayer> {
    inner: sys::napi_ref,
    env: sys::napi_env,
    _marker: PhantomData<fn() -> L>,
}

impl<L: ExtendLayer> LayerRef<L> {
    /// Create a strong reference to `obj`.
    pub fn new(obj: &Object, env: &Env) -> Result<Self> {
        let mut inner = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_create_reference(env.raw(), obj.raw(), 1, &mut inner) },
            "LayerRef: failed to create reference"
        )?;
        Ok(Self {
            inner,
            env: env.raw(),
            _marker: PhantomData,
        })
    }

    fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        let mut value = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_get_reference_value(env.raw(), self.inner, &mut value) },
            "LayerRef: failed to get reference value"
        )?;
        Ok(value)
    }
}

impl<L: ExtendLayer> ToNapiValue for LayerRef<L> {
    unsafe fn to_napi_value(env: sys::napi_env, mut val: Self) -> Result<sys::napi_value> {
        let raw = val.raw_value(&Env::from_raw(env))?;
        let _ = unsafe { sys::napi_delete_reference(env, val.inner) };
        val.inner = ptr::null_mut();
        Ok(raw)
    }
}

impl<L: ExtendLayer> FromNapiValue for LayerRef<L> {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> Result<Self> {
        let mut inner = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_create_reference(env, napi_val, 1, &mut inner) },
            "LayerRef: failed to create reference"
        )?;
        Ok(Self {
            inner,
            env,
            _marker: PhantomData,
        })
    }
}

impl<L: ExtendLayer> Drop for LayerRef<L> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let _ = unsafe { sys::napi_delete_reference(self.env, self.inner) };
            self.inner = ptr::null_mut();
        }
    }
}
