//! A typed handle to a JS object of a specific layer class.
//!
//! A method that returns one of its own layers' JS objects (a node, an
//! element, the document, ...) declares `LayerRef<L>` as its return type:
//! the runtime hands back the exact object that already carries `L`'s own
//! block, so instance identity is preserved (no re-wrapping). `L` only
//! fixes the declared class at compile time - no layer data is copied. The
//! macro maps `LayerRef<L>` to the layer's JS class name in the TS defs.
//!
//! TODO: Add `FromNapiRef`/`FromNapiMutRef` for `&L` / `&mut L` so that
//! layer methods can receive typed layer references as arguments. The
//! runtime would use `napi_unwrap` to reach the own block and expose it as
//! `&'static L` / `&'static mut L` (mirroring napi-rs's class borrows).
//! This is deferred until a concrete use case appears.

use std::{marker::PhantomData, ptr, rc::Rc};

use napi::{
    Env, JsValue, Result,
    bindgen_prelude::{FromNapiValue, Object, ToNapiValue},
    check_status, sys,
};

use crate::layer::ExtendLayer;

/// The shared part of a [`LayerRef`]: the `napi_ref` plus the `napi_env`
/// (stable for the addon's lifetime) needed to delete it.
struct RefInner {
    inner: sys::napi_ref,
    env: sys::napi_env,
}

impl Drop for RefInner {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let _ = unsafe { sys::napi_delete_reference(self.env, self.inner) };
        }
    }
}

/// A strong reference to a JS object whose own-data chain carries layer `L`.
///
/// Cloning shares the underlying reference; the `napi_ref` is released when
/// the last clone is dropped. Converting back into a JS value hands out the
/// referenced object without releasing the reference.
pub struct LayerRef<L: ExtendLayer> {
    inner: Rc<RefInner>,
    _marker: PhantomData<fn() -> L>,
}

impl<L: ExtendLayer> Clone for LayerRef<L> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            _marker: PhantomData,
        }
    }
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
            inner: Rc::new(RefInner {
                inner,
                env: env.raw(),
            }),
            _marker: PhantomData,
        })
    }

    fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        let mut value = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_get_reference_value(env.raw(), self.inner.inner, &mut value) },
            "LayerRef: failed to get reference value"
        )?;
        Ok(value)
    }
}

impl<L: ExtendLayer> ToNapiValue for LayerRef<L> {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> Result<sys::napi_value> {
        val.raw_value(&Env::from_raw(env))
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
            inner: Rc::new(RefInner { inner, env }),
            _marker: PhantomData,
        })
    }
}
