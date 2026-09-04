//! Dispatch plumbing shared by the event classes: JS value helpers the
//! layers use to build/resolve values from a napi `Env` handed in through
//! the `#[layer]` `env: &Env` injection.
//!
//! The full three-phase dispatch walk (capture → target → bubble) over a
//! listener store is layered on top of these primitives later; this module
//! only provides the base the event classes build on.

use napi::{
    Env, JsNumber, JsString, Result, UnknownRef, ValueType,
    bindgen_prelude::{FromNapiValue, JsValue, Null, ToNapiValue, Unknown},
};

/// A JS `null` value as `Unknown`.
pub fn null_unknown(env: &Env) -> Result<Unknown<'static>> {
    let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), Null) }?;
    unsafe { Unknown::from_napi_value(env.raw(), raw) }
}

/// A JS `undefined` value as `Unknown`.
pub fn undefined_unknown(env: &Env) -> Result<Unknown<'static>> {
    let mut raw = std::ptr::null_mut();
    unsafe { napi::sys::napi_get_undefined(env.raw(), &mut raw) };
    unsafe { Unknown::from_napi_value(env.raw(), raw) }
}

/// Convert a borrowed JS value into an owned `'static` `Unknown`.
pub fn to_unknown(env: &Env, v: &Unknown<'_>) -> Result<Unknown<'static>> {
    unsafe { Unknown::from_napi_value(env.raw(), JsValue::raw(v)) }
}

/// A JS value held by an event layer, normalized to Rust storage.
///
/// napi `Unknown`/`napi_value` handles are only valid inside the native call
/// that produced them, so an event can not store an arbitrary JS value as a
/// bare handle for later reads. Primitives are copied into Rust values;
/// objects/functions are retained through a napi reference (the only
/// cross-call handle in Node-API).
pub enum StoredValue {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Undefined,
    Object(UnknownRef),
}

impl StoredValue {
    /// Normalize a JS value into Rust storage, classifying by its runtime
    /// type.
    pub fn from_value(v: &Unknown<'_>) -> Result<Self> {
        match v.get_type()? {
            ValueType::String => Ok(Self::Str(
                unsafe { v.cast::<JsString>() }?.into_utf8()?.into_owned()?,
            )),
            ValueType::Number => Ok(Self::Num(unsafe { v.cast::<JsNumber>() }?.get_double()?)),
            ValueType::Boolean => Ok(Self::Bool(unsafe { v.cast::<bool>() }?)),
            ValueType::Null => Ok(Self::Null),
            ValueType::Undefined => Ok(Self::Undefined),
            _ => Ok(Self::Object(v.create_ref()?)),
        }
    }

    /// Rebuild the JS value from the stored data.
    pub fn to_value(&self, env: &Env) -> Result<Unknown<'static>> {
        match self {
            Self::Str(s) => {
                let js: JsString = env.create_string(s)?;
                unsafe { Unknown::from_napi_value(env.raw(), js.raw()) }
            }
            Self::Num(n) => {
                let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), *n) }?;
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::Bool(b) => {
                let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), *b) }?;
                unsafe { Unknown::from_napi_value(env.raw(), raw) }
            }
            Self::Null => null_unknown(env),
            Self::Undefined => undefined_unknown(env),
            Self::Object(r) => {
                let v = r.get_value(env)?;
                to_unknown(env, &v)
            }
        }
    }
}
