//! Rust-side lifecycle event dispatch for the window/app shell.
//!
//! The `Event` layer instance is built from Rust data and dispatched
//! through the receiver's `EventTargetLayer` slot directly — the same
//! path the DOM event driver uses — so no JS factory or dispatch-function
//! registry is involved.

use std::{cell::RefCell, rc::Rc};

use napi::{Env, Error, Result, Status, bindgen_prelude::Object};
use napi_helpers::{
    JsWeakRef,
    inherits::{LayerChain, new_from_chain, with_own},
};

use crate::{
    dom::shared::doc::SharedDocument,
    events::base::{EventLayer, EventTargetLayer},
};

/// Resolve the JS Window object from the document's weak ref.
pub(crate) fn resolve_window<'a>(doc: &Rc<SharedDocument>, env: &'a Env) -> Option<Object<'a>> {
    doc.js_window_ref()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
}

/// Build a plain `Event` layer instance from Rust data.
fn build_event<'env>(
    env: &'env Env,
    event_type: &str,
    bubbles: bool,
    cancelable: bool,
) -> Result<Object<'env>> {
    let chain = LayerChain {
        own: EventLayer::with_init(event_type.to_string(), bubbles, cancelable, false),
        parent: (),
    };
    new_from_chain::<EventLayer>(env, chain)
}

/// Dispatch the event through the receiver's `EventTargetLayer` slot.
fn dispatch_to(target: &Object, event: &Object, env: &Env) -> Result<()> {
    let _ = with_own::<EventTargetLayer, _>(target, |d| d.dispatch_event(env, *event))?;
    Ok(())
}

/// Dispatch a lifecycle event to the JS Window object resolved from the
/// document. Returns whether a listener called `preventDefault`.
pub(crate) fn dispatch_window_event(
    doc: &Rc<SharedDocument>,
    event_type: &str,
    cancelable: bool,
    env: &Env,
) -> Result<bool> {
    let window = resolve_window(doc, env)
        .ok_or_else(|| Error::new(Status::GenericFailure, "no window to dispatch to"))?;
    let event = build_event(env, event_type, false, cancelable)?;
    dispatch_to(&window, &event, env)?;
    with_own::<EventLayer, _>(&event, |d| d.state_ref().canceled)
}

/// Dispatch a lifecycle event to the JS `BlitzApp` object resolved from
/// the app-level weak ref. Returns whether a listener called
/// `preventDefault`.
pub(crate) fn dispatch_app_event(
    app_ref: &Rc<RefCell<Option<JsWeakRef>>>,
    event_type: &str,
    cancelable: bool,
    env: &Env,
) -> Result<bool> {
    let app = app_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
        .ok_or_else(|| Error::new(Status::GenericFailure, "no app to dispatch to"))?;
    let event = build_event(env, event_type, false, cancelable)?;
    dispatch_to(&app, &event, env)?;
    with_own::<EventLayer, _>(&event, |d| d.state_ref().canceled)
}
