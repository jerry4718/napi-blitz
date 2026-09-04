//! Dispatch plumbing shared by the event classes: a global napi `Env`
//! (event methods need it to invoke stored JS callbacks — the `#[layer]`
//! method bodies receive no env directly).
//!
//! The full three-phase dispatch walk (capture → target → bubble) over a
//! listener store is layered on top of these primitives later; this module
//! only provides the base the event classes build on.

use std::cell::RefCell;

use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{FromNapiValue, JsValue, Null, ToNapiValue, Unknown},
};

thread_local! {
    static ENV: RefCell<Option<Env>> = const { RefCell::new(None) };
}

/// Register the addon-level napi `Env`. Called once during addon init,
/// before any event is dispatched.
pub fn set_env(env: Env) {
    ENV.with(|e| *e.borrow_mut() = Some(env));
}

/// Resolve the registered napi `Env`.
pub fn env() -> Result<Env> {
    ENV.with(|e| {
        e.borrow()
            .ok_or_else(|| Error::new(Status::GenericFailure, "dispatch env not initialized"))
    })
}

/// A JS `null` value as `Unknown`.
pub fn null_unknown(env: &Env) -> Result<Unknown<'static>> {
    let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), Null) }?;
    unsafe { Unknown::from_napi_value(env.raw(), raw) }
}

/// Convert a borrowed JS value into an owned `'static` `Unknown`.
pub fn to_unknown(env: &Env, v: &Unknown<'_>) -> Result<Unknown<'static>> {
    unsafe { Unknown::from_napi_value(env.raw(), JsValue::raw(v)) }
}
