//! A refcount-0 N-API reference that does not keep its JS object alive.
//!
//! napi-rs `ObjectRef` always creates a strong reference (refcount ≥ 1), so
//! the weak-reference operations are kept in this small wrapper over
//! [`RefInner`].

use crate::{Finalize, refs::RefInner};
use napi::{Env, Result, bindgen_prelude::Object, sys};

/// A weak reference to a JS object.
///
/// Created via [`WeakRef::new`] with an initial refcount of **0**, meaning
/// it does not prevent V8 from garbage-collecting the target. Use
/// [`WeakRef::get_value`] to probe whether the object is still alive.
pub struct WeakRef {
    inner: RefInner<true>,
}

impl WeakRef {
    /// Create a weak reference to `obj`.
    pub fn new(env: &Env, obj: &Object) -> Result<Self> {
        Ok(Self {
            inner: RefInner::new(env, obj)?,
        })
    }

    /// Create a weak reference from raw handles.
    ///
    /// # Safety
    ///
    /// `env` must be a valid `napi_env` from the current native call, and
    /// `value` must be a valid JS value belonging to that environment.
    pub unsafe fn from_raw(env: sys::napi_env, value: sys::napi_value) -> Result<Self> {
        Ok(Self {
            inner: unsafe { RefInner::from_raw(env, value)? },
        })
    }

    /// Retrieve the referenced value as a raw handle.
    pub fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        self.inner.raw_value(env)
    }

    /// Try to retrieve the JS object. Returns `None` if it has been
    /// garbage-collected.
    pub fn get_value<'env>(&self, env: &'env Env) -> Option<Object<'env>> {
        self.inner.get_value(env)
    }

    /// Whether the JS object is still alive (not yet collected).
    #[allow(unused)]
    pub fn is_alive(&self, env: &Env) -> bool {
        self.inner.is_alive(env)
    }

    /// Attach a finalizer to the referenced JS object. When V8 collects it,
    /// `data.finalize(env)` will be called.
    #[allow(unused)]
    pub fn add_finalizer<T: Finalize + 'static>(&self, env: &Env, data: T) -> Result<()> {
        self.inner.add_finalizer(env, data)
    }
}
