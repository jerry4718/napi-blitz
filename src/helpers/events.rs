//! Shared napi event-dispatch primitives for the DOM event pipeline
//! (`src/dom/event.rs`) and the shell/window lifecycle dispatch
//! (`src/app/shell_event.rs`). Both drive `dispatchEvent` over the same
//! registered JS factory + dispatch fn, so the underlying steps live here.

use std::rc::Rc;

use super::discard_err;
use crate::{
    dom::{doc::SharedDoc, payload::EventPayload},
    global,
};
use napi::bindgen_prelude::JsObjectValue;
use napi::{
    Env, Error, JsValue, Result, Status,
    bindgen_prelude::{FnArgs, Object},
};

/// Resolve the JS Window object from `SharedDoc::js_window_ref`.
pub(crate) fn resolve_window<'a>(doc: &Rc<SharedDoc>, env: &'a Env) -> Option<Object<'a>> {
    doc.js_window_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
}

/// Build a JS `Event` object from a payload via the registered event
/// factory. The factory's return value is received as `Option<Unknown>`:
/// `Unknown` wraps the value without converting or creating a reference
/// (so any JS return value is accepted without a napi conversion error);
/// `None` means the factory produced no event.
pub(crate) fn build_event_object(payload: EventPayload, env: &Env) -> Result<Object<'_>> {
    let factory_ref = global::get_event_factory()
        .ok_or_else(|| Error::new(Status::GenericFailure, "event_factory not registered"))?;
    let factory_fn = factory_ref.borrow_back(env)?;
    let unknown = factory_fn.call(FnArgs::from((payload,)))?;
    unknown
        .ok_or_else(|| Error::new(Status::GenericFailure, "event factory returned no event"))?
        .coerce_to_object()
}

/// Call `dispatchEvent(target, event)` via the registered dispatch fn.
/// JS exceptions and napi boundary failures propagate via `?`.
pub(crate) fn dispatch_event(target: &Object, event: &Object, env: &Env) -> Result<()> {
    let dispatch_ref = global::get_dispatch_fn()
        .ok_or_else(|| Error::new(Status::GenericFailure, "dispatch_fn not registered"))?;
    let dispatch_fn = dispatch_ref.borrow_back(env)?;
    let target_ref = target.create_ref::<true>()?;
    let event_ref = event.create_ref::<true>()?;
    dispatch_fn.call(FnArgs::from((target_ref, event_ref)))?;
    Ok(())
}

/// Read a boolean flag off the event object (`defaultPrevented`,
/// `cancelBubble`). Conservative `false` when the property is unreadable.
pub(crate) fn read_event_flag(event: &Object, name: &str) -> bool {
    event.get_named_property::<bool>(name).unwrap_or(false)
}

/// Reset `currentTarget` to `null` and `eventPhase` to `0` (NONE) after
/// dispatch completes, per DOM spec. Best-effort: a reset failure is
/// non-fatal, so the error is logged and deliberately dropped.
pub(crate) fn reset_dispatch_state(event: &mut Object, _env: &Env) {
    discard_err!(
        event.set_named_property("currentTarget", ()),
        "failed to reset currentTarget"
    );
    discard_err!(
        event.set_named_property("eventPhase", 0u32),
        "failed to reset eventPhase"
    );
}
