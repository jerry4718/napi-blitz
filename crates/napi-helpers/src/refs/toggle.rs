//! A reference whose strength can be toggled between strong and weak at
//! runtime.
//!
//! Backed by a single napi handle held inside [`ToggleInner`], which
//! discriminates the current strength: a strong (`RefInner<false>`) or weak
//! (`RefInner<true>`) reference.

use napi::{Env, Result, bindgen_prelude::Object, sys};

use crate::{Finalize, refs::RefInner};

/// The current strength of a [`ToggleRef`]: a weak (`RefInner<true>`) or
/// strong (`RefInner<false>`) reference.
enum ToggleInner {
    Weak(RefInner<true>),
    Strong(RefInner<false>),
}

/// A reference to a JS object whose strength can be switched at runtime.
///
/// - `make_strong` switches to strong (object cannot be GC'd); no-op if
///   already strong.
/// - `make_weak` switches to weak (object can be GC'd); no-op if already
///   weak.
/// - `get_value` retrieves the object if still alive.
/// - `add_finalizer` attaches a callback that fires when the object is
///   collected (only possible while in weak mode).
pub struct ToggleRef {
    inner: Option<ToggleInner>,
}

impl ToggleRef {
    /// Create a strong reference (refcount 1): the target cannot be
    /// garbage-collected until switched weak.
    pub fn new_strong(env: &Env, obj: &Object) -> Result<Self> {
        Ok(Self {
            inner: Some(ToggleInner::Strong(RefInner::new(env, obj)?)),
        })
    }

    /// Create a weak reference (refcount 0): the target may be collected;
    /// probe with [`ToggleRef::get_value`].
    pub fn new_weak(env: &Env, obj: &Object) -> Result<Self> {
        Ok(Self {
            inner: Some(ToggleInner::Weak(RefInner::new(env, obj)?)),
        })
    }

    /// Create a reference from raw handles with the strength selected by
    /// `strong`.
    ///
    /// # Safety
    ///
    /// `env` must be a valid `napi_env` from the current native call, and
    /// `value` must be a valid JS value belonging to that environment.
    pub unsafe fn from_raw(
        env: sys::napi_env,
        value: sys::napi_value,
        strong: bool,
    ) -> Result<Self> {
        let inner = if strong {
            ToggleInner::Strong(unsafe { RefInner::from_raw(env, value)? })
        } else {
            ToggleInner::Weak(unsafe { RefInner::from_raw(env, value)? })
        };
        Ok(Self { inner: Some(inner) })
    }

    /// Retrieve the referenced value as a raw handle.
    pub fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        match self.inner.as_ref() {
            Some(ToggleInner::Strong(inner)) => inner.raw_value(env),
            Some(ToggleInner::Weak(inner)) => inner.raw_value(env),
            None => unreachable!(
                "ToggleRef inner must stay Some; a None here means its strength-switch state was corrupted"
            ),
        }
    }

    /// Whether the reference is currently strong.
    #[allow(unused)]
    pub fn is_strong(&self) -> bool {
        matches!(self.inner.as_ref(), Some(ToggleInner::Strong(_)))
    }

    /// Switch to strong (refcount 1). No-op if already strong.
    pub fn make_strong(&mut self, env: &Env) -> Result<()> {
        self.inner = Some(match self.inner.take() {
            None => unreachable!(
                "ToggleRef inner must stay Some; a None here means its strength-switch state was corrupted"
            ),
            Some(ToggleInner::Weak(w)) => ToggleInner::Strong(w.upgrade(env)?),
            Some(taken) => taken,
        });
        Ok(())
    }

    /// Switch to weak (refcount 0). No-op if already weak.
    pub fn make_weak(&mut self, env: &Env) -> Result<()> {
        self.inner = Some(match self.inner.take() {
            None => unreachable!(
                "ToggleRef inner must stay Some; a None here means its strength-switch state was corrupted"
            ),
            Some(ToggleInner::Strong(s)) => ToggleInner::Weak(s.downgrade(env)?),
            Some(taken) => taken,
        });
        Ok(())
    }

    /// Retrieve the JS object. Returns `None` if it has been collected
    /// (only possible in weak mode).
    pub fn get_value<'env>(&self, env: &'env Env) -> Option<Object<'env>> {
        match self.inner.as_ref() {
            Some(ToggleInner::Weak(inner)) => inner.get_value(env),
            Some(ToggleInner::Strong(inner)) => inner.get_value(env),
            None => None,
        }
    }

    /// Whether the object is still alive.
    pub fn is_alive(&self, env: &Env) -> bool {
        self.get_value(env).is_some()
    }

    /// Attach a finalizer to the referenced JS object. When V8 collects
    /// it (only possible while in weak mode), `data.finalize(env)` will
    /// be called.
    pub fn add_finalizer<T: Finalize + 'static>(&self, env: &Env, data: T) -> Result<()> {
        match self.inner.as_ref() {
            Some(ToggleInner::Weak(inner)) => inner.add_finalizer(env, data),
            Some(ToggleInner::Strong(inner)) => inner.add_finalizer(env, data),
            None => Ok(()),
        }
    }
}
