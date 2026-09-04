//! A JS value normalized to Rust storage, with automatic cleanup on drop.

use std::{ptr, rc::Rc};

use napi::{
    Env, Result, ValueType,
    bindgen_prelude::{BigInt, FromNapiValue, JsValue, Null, ToNapiValue, Unknown},
    check_status, sys, type_of,
};

/// The shared part of a [`AnyRef`]: the `napi_ref` plus the `napi_env`
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

/// A strong napi reference to a JS object that deletes the underlying
/// `napi_ref` when the last clone is dropped.
///
/// napi's `UnknownRef` stores only the `napi_ref` handle, so dropping it
/// cannot reach the `Env` needed to delete the reference. The reference is
/// shared through `Rc`, so cloning a [`AnyRef`] (and thus an [`AnyValue`])
/// does not duplicate the `napi_ref`; the `napi_ref` lives until every clone
/// is gone.
#[derive(Clone)]
pub struct AnyRef {
    inner: Rc<RefInner>,
}

impl AnyRef {
    /// Create a strong reference to `value`.
    pub fn new(env: &Env, value: sys::napi_value) -> Result<Self> {
        let mut inner = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_create_reference(env.raw(), value, 1, &mut inner) },
            "RefValue: failed to create reference"
        )?;
        Ok(Self {
            inner: Rc::new(RefInner {
                inner,
                env: env.raw(),
            }),
        })
    }

    /// Retrieve the referenced value.
    pub fn get_value<'e>(&self, env: &'e Env) -> Result<Unknown<'e>> {
        let mut value = ptr::null_mut();
        check_status!(
            unsafe { sys::napi_get_reference_value(env.raw(), self.inner.inner, &mut value) },
            "RefValue: failed to get reference value"
        )?;
        unsafe { Unknown::from_napi_value(env.raw(), value) }
    }
}

/// A JS value held in Rust-side storage, normalized by runtime type.
///
/// napi `Unknown`/`napi_value` handles are only valid inside the native call
/// that produced them, so an event can not store an arbitrary JS value as a
/// bare handle for later reads. Primitives are copied into Rust values;
/// objects/functions are retained through a reference that releases itself on
/// drop (the only cross-call handle in Node-API). Cloning an `AnyValue`
/// clones the stored value — primitives are copied, object references are
/// shared.
#[derive(Clone)]
pub enum AnyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    BigInt(BigInt),
    Null,
    Undefined,
    Object(AnyRef),
}

impl AnyValue {
    /// Normalize a JS value into Rust storage, classifying by runtime type.
    pub fn from_value(env: &Env, v: &Unknown<'_>) -> Result<Self> {
        match v.get_type()? {
            ValueType::String => Ok(Self::String(
                unsafe { v.cast::<napi::JsString>() }?
                    .into_utf8()?
                    .into_owned()?,
            )),
            ValueType::Number => Ok(Self::Number(
                unsafe { v.cast::<napi::JsNumber>() }?.get_double()?,
            )),
            ValueType::Boolean => Ok(Self::Boolean(unsafe { v.cast::<bool>() }?)),
            ValueType::BigInt => Ok(Self::BigInt(unsafe { v.cast::<BigInt>() }?)),
            ValueType::Null => Ok(Self::Null),
            ValueType::Undefined => Ok(Self::Undefined),
            _ => Ok(Self::Object(AnyRef::new(env, JsValue::raw(v))?)),
        }
    }

    /// Rebuild the JS value from the stored data.
    pub fn to_value(&self, env: &Env) -> Result<Unknown<'static>> {
        match self {
            Self::String(s) => {
                let js: napi::JsString = env.create_string(s)?;
                unsafe { Unknown::from_napi_value(env.raw(), js.raw()) }
            }
            Self::Number(n) => {
                let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), *n) }?;
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::Boolean(b) => {
                let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), *b) }?;
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::BigInt(b) => {
                let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), b.clone()) }?;
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::Null => {
                let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), Null) }?;
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::Undefined => {
                let mut raw = ptr::null_mut();
                unsafe { sys::napi_get_undefined(env.raw(), &mut raw) };
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::Object(r) => {
                let v = r.get_value(env)?;
                unsafe { Unknown::from_napi_value(env.raw(), JsValue::raw(&v)) }
            }
        }
    }
}

impl FromNapiValue for AnyValue {
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
            // Everything else (objects, functions, symbols, ...) is retained
            // by reference so the value survives the call.
            _ => Ok(Self::Object(AnyRef::new(&Env::from_raw(env), napi_val)?)),
        }
    }
}

impl ToNapiValue for AnyValue {
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
            Self::Object(r) => {
                let env = Env::from_raw(env);
                let v = r.get_value(&env)?;
                Ok(JsValue::raw(&v))
            }
        }
    }
}
