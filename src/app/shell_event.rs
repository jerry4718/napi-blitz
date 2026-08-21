//! `JsShellEventHandler`: unified dispatch for window/app lifecycle events.
//!
//! Mirrors `JsEventHandler` (DOM events) but for shell-level events that
//! originate from winit's `ApplicationHandler` and the async `openWindow`
//! / `closeWindow` paths: `open`, `close`, `closed` (window-level) and
//! `window:open`, `window:close`, `window:closed` (app-level echoes).
//!
//! A close request is currently dispatched to the two receivers
//! INDEPENDENTLY: the window receives `close` (its own event), and the app
//! receives `window:close` (the `window:` prefix marks the subject so the
//! app never mistakes it for its own close). Same moment, same capability
//! — a `preventDefault()` at either level vetoes the close.
//!
//! Both event systems share the same dispatch primitives:
//! - `global::get_event_factory()` to build the JS `Event`
//! - `global::get_dispatch_fn()` to call `dispatchEvent(target, event)`
//! - `event.get_named_property::<bool>("defaultPrevented" | "cancelBubble")`
//!   to read back dispatch flags
//!
//! DOM events set `target`/`currentTarget` via lazy getters in
//! `dom/event.rs`; shell events dispatch to the window/app directly and
//! let the JS `EventTarget.dispatchEvent` machinery set `target`.

use std::{cell::RefCell, rc::Rc};

use crate::{
    dom::doc::SharedDoc,
    helpers::{
        JsWeakRef, build_event_object, dispatch_event, read_event_flag, reset_dispatch_state,
        resolve_window,
    },
};
use napi::{Env, Result, Status, bindgen_prelude::Object};

/// Unified dispatcher for window/app lifecycle events.
///
/// Holds a weak ref to the JS `BlitzApp` (via `app_ref`); the window
/// target is resolved from `SharedDoc::js_window_ref`. Both are
/// refcount-0 weak references; JS keeps the objects alive on its side.
pub struct JsShellEventHandler {
    /// Weak ref to the JS `BlitzApp` object, set by `NativeApp::set_app_ref`
    /// when JS opts in.
    pub app_ref: Rc<RefCell<Option<JsWeakRef>>>,
}

impl JsShellEventHandler {
    pub fn new(app_ref: Rc<RefCell<Option<JsWeakRef>>>) -> Self {
        Self { app_ref }
    }

    /// Dispatch a cancelable event to the window alone. Returns `true` if
    /// the default was prevented. Used for the window-level `close`
    /// request, where a listener can veto the action.
    pub fn dispatch_cancelable(
        &self,
        event_type: &str,
        doc: &Rc<SharedDoc>,
        env: &Env,
    ) -> Result<bool> {
        let window = resolve_window(doc, env)
            .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no window to dispatch to"))?;
        let mut event_obj = build_event(event_type, true, false, env)?;
        dispatch_event(&window, &event_obj, env)?;
        let prevented = read_event_flag(&event_obj, "defaultPrevented");
        reset_dispatch_state(&mut event_obj, env);
        Ok(prevented)
    }

    /// Dispatch a non-cancelable event to the window alone. Used for the
    /// post-teardown notification `closed`.
    pub fn dispatch_window_event(
        &self,
        event_type: &str,
        doc: &Rc<SharedDoc>,
        env: &Env,
    ) -> Result<()> {
        let window = resolve_window(doc, env)
            .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no window to dispatch to"))?;
        let mut event_obj = build_event(event_type, false, false, env)?;
        dispatch_event(&window, &event_obj, env)?;
        reset_dispatch_state(&mut event_obj, env);
        Ok(())
    }

    /// Dispatch a non-cancelable event to the app alone. Used for the
    /// post-teardown notification `window:closed`.
    pub fn dispatch_app_event(&self, event_type: &str, env: &Env) -> Result<()> {
        let app = resolve_app(&self.app_ref, env)
            .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no app to dispatch to"))?;
        let mut event_obj = build_event(event_type, false, false, env)?;
        dispatch_event(&app, &event_obj, env)?;
        reset_dispatch_state(&mut event_obj, env);
        Ok(())
    }

    /// Dispatch a cancelable event to the app alone. Returns `true` if the
    /// default was prevented. Used for `window:open` (no window-level
    /// receiver exists at creation time).
    pub fn dispatch_app_cancelable(&self, event_type: &str, env: &Env) -> Result<bool> {
        let app = resolve_app(&self.app_ref, env)
            .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no app to dispatch to"))?;
        let mut event_obj = build_event(event_type, true, false, env)?;
        dispatch_event(&app, &event_obj, env)?;
        let prevented = read_event_flag(&event_obj, "defaultPrevented");
        reset_dispatch_state(&mut event_obj, env);
        Ok(prevented)
    }

    /// Dispatch the cancelable close request to the window (`close`) and,
    /// independently, to the app (`window:close`). A listener at either
    /// level may `preventDefault()`. Returns `Ok(true)` if the close should
    /// proceed (neither level prevented).
    pub fn close_request(&self, doc: &Rc<SharedDoc>, env: &Env) -> Result<bool> {
        let window_prevented = self.dispatch_cancelable("close", doc, env)?;
        let app_prevented = self.dispatch_app_cancelable("window:close", env)?;
        Ok(!window_prevented && !app_prevented)
    }

    /// Dispatch the full close sequence: the cancelable close request
    /// (window `close` + app `window:close`), and — if not prevented —
    /// the post-teardown notifications `closed` (window) and
    /// `window:closed` (app). Returns `Ok(true)` if the close should
    /// proceed (not prevented).
    pub fn close_sequence(&self, doc: &Rc<SharedDoc>, env: &Env) -> Result<bool> {
        if !self.close_request(doc, env)? {
            return Ok(false);
        }
        self.dispatch_window_event("closed", doc, env)?;
        self.dispatch_app_event("window:closed", env)?;
        Ok(true)
    }

    /// Dispatch the post-teardown notifications `closed` (window) and
    /// `window:closed` (app) after a close that already passed its
    /// cancelable request.
    pub fn notify_closed(&self, doc: &Rc<SharedDoc>, env: &Env) -> Result<()> {
        self.dispatch_window_event("closed", doc, env)?;
        self.dispatch_app_event("window:closed", env)?;
        Ok(())
    }

    /// Dispatch the full open sequence: at window-creation time (before
    /// `openWindow` resolves — JS does not yet hold a Window object), the
    /// app-level `window:open` event is dispatched to the app as a
    /// cancelable request. A listener's `preventDefault()` cancels the
    /// open and rejects the `openWindow` promise. Returns `Ok(true)` if the
    /// open should proceed (not prevented).
    pub fn open_sequence(&self, env: &Env) -> Result<bool> {
        Ok(!self.dispatch_app_cancelable("window:open", env)?)
    }
}

// ── Dispatch primitives ────────────────────────────────────────────────

/// Resolve the JS BlitzApp object from the app-level weak ref.
fn resolve_app<'a>(app_ref: &Rc<RefCell<Option<JsWeakRef>>>, env: &'a Env) -> Option<Object<'a>> {
    app_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
}

/// Build a JS `Event` object via the registered event factory.
fn build_event<'a>(
    event_type: &str,
    cancelable: bool,
    bubbles: bool,
    env: &'a Env,
) -> Result<Object<'a>> {
    use crate::dom::payload::EventPayload;
    let payload = EventPayload {
        event_type: event_type.to_string(),
        bubbles,
        cancelable,
        pointer: None,
        wheel: None,
        key: None,
        input: None,
        ime: None,
    };
    build_event_object(payload, env)
}
