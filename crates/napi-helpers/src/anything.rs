//! A JS value normalized to Rust storage, with automatic cleanup on drop.

use std::{ptr, rc::Rc};

use napi::{
    Env, Result, ValueType,
    bindgen_prelude::{BigInt, FromNapiValue, Null, Object, ToNapiValue},
    check_status, sys, type_of,
};

use crate::refs::RefInner;

/// A strong napi reference to a JS object that deletes the underlying
/// `napi_ref` when the last clone is dropped.
///
/// napi's `UnknownRef` stores only the `napi_ref` handle, so dropping it
/// cannot reach the `Env` needed to delete the reference. The reference is
/// shared through `Rc`, so cloning a [`OtherRef`] (and thus an [`Anything`])
/// does not duplicate the `napi_ref`; the `napi_ref` lives until every clone
/// is gone.
#[derive(Clone)]
pub struct OtherRef {
    inner: Rc<RefInner>,
}

impl OtherRef {
    /// Create a strong reference to `obj`.
    pub fn new(env: &Env, obj: &Object) -> Result<Self> {
        Ok(Self {
            inner: Rc::new(RefInner::new(env, obj)?),
        })
    }

    /// Create a strong reference from raw handles.
    ///
    /// # Safety
    ///
    /// `env` must be a valid `napi_env` from the current native call, and
    /// `value` must be a valid JS value belonging to that environment.
    pub unsafe fn from_raw(env: sys::napi_env, value: sys::napi_value) -> Result<Self> {
        Ok(Self {
            inner: Rc::new(unsafe { RefInner::from_raw(env, value)? }),
        })
    }

    /// Retrieve the referenced value as a raw handle.
    pub fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        self.inner.raw_value(env)
    }
}

/// A JS value held in Rust-side storage, normalized by runtime type.
///
/// napi `Unknown`/`napi_value` handles are only valid inside the native call
/// that produced them, so an event can not store an arbitrary JS value as a
/// bare handle for later reads. Primitives are copied into Rust values;
/// objects/functions are retained through a reference that releases itself on
/// drop (the only cross-call handle in Node-API). Cloning an `Anything`
/// clones the stored value — primitives are copied, object references are
/// shared.
#[derive(Clone)]
pub enum Anything {
    String(String),
    Number(f64),
    Boolean(bool),
    BigInt(BigInt),
    Null,
    Undefined,
    Object(OtherRef),
    Function(OtherRef),
}

impl FromNapiValue for Anything {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> Result<Self> {
        // Classify the raw JS value directly; no `Unknown` round-trip.
        match type_of!(env, napi_val)? {
            ValueType::String => Ok(Self::String(unsafe {
                String::from_napi_value(env, napi_val)
            }?)),
            ValueType::Number => Ok(Self::Number(unsafe {
                f64::from_napi_value(env, napi_val)
            }?)),
            ValueType::Boolean => Ok(Self::Boolean(unsafe {
                bool::from_napi_value(env, napi_val)
            }?)),
            ValueType::BigInt => Ok(Self::BigInt(unsafe {
                BigInt::from_napi_value(env, napi_val)
            }?)),
            ValueType::Null => Ok(Self::Null),
            ValueType::Undefined => Ok(Self::Undefined),
            ValueType::Function => Ok(Self::Function(unsafe {
                OtherRef::from_raw(env, napi_val)?
            })),
            // Everything else (objects, symbols, ...) is retained
            // by reference so the value survives the call.
            _ => Ok(Self::Object(unsafe { OtherRef::from_raw(env, napi_val)? })),
        }
    }
}

impl ToNapiValue for Anything {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> Result<sys::napi_value> {
        // Rebuild the JS value per variant; no `Unknown` round-trip.
        match val {
            Self::String(s) => unsafe { String::to_napi_value(env, s) },
            Self::Number(n) => unsafe { f64::to_napi_value(env, n) },
            Self::Boolean(b) => unsafe { bool::to_napi_value(env, b) },
            Self::BigInt(b) => unsafe { BigInt::to_napi_value(env, b) },
            Self::Null => unsafe { Null::to_napi_value(env, Null) },
            Self::Undefined => {
                let mut raw = ptr::null_mut();
                check_status!(unsafe { sys::napi_get_undefined(env, &mut raw) })?;
                Ok(raw)
            }
            Self::Object(r) | Self::Function(r) => {
                let env = Env::from_raw(env);
                Ok(r.raw_value(&env)?)
            }
        }
    }
}
