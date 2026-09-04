//! Shared napi helpers; the generic building blocks moved to
//! `napi-helpers` live under the same flat names here.
mod events;

pub(crate) use events::*;
pub(crate) use napi_helpers::Finalize;
pub(crate) use napi_helpers::JsWeakRef;
pub(crate) use napi_helpers::SwitchableRef;
pub(crate) use napi_helpers::discard_err;
pub(crate) use napi_helpers::finalize_trampoline;
