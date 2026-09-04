//! Small building blocks on top of napi that the higher layers
//! (napi-inherit, wintertc-events, napi-blitz-dom, ...) share.

pub mod anything;
pub mod deferred;
pub mod finalize;
pub mod js_weak_ref;
pub mod log;
pub mod proxy;
pub mod switchable_ref;

use napi_inherit as inherit;

pub mod inherits {
    pub use super::inherit::*;
}

pub use deferred::Deferred;
pub use finalize::{Finalize, finalize_trampoline};
pub use js_weak_ref::JsWeakRef;
pub use switchable_ref::SwitchableRef;
