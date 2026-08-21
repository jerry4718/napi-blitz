//! `JsShellEventHandler`: unified dispatch for window/app lifecycle events.
//!
//! Mirrors `JsEventHandler` (DOM events) but for shell-level events that
//! originate from winit's `ApplicationHandler` and the async `openWindow`
//! / `closeWindow` paths: `open`, `close`, `closed` (window-level) and
//! `window:open`, `window:close`, `window:closed` (app-level echoes).
//!
//! Both event systems share the same dispatch primitives:
//! - `global::get_event_factory()` to build the JS `Event`
//! - `global::get_dispatch_fn()` to call `dispatchEvent(target, event)`
//! - `global::get_default_prevented_getter()` to read `defaultPrevented`
//!
//! The difference: shell events target `window` or `app` JS objects
//! (held via `JsWeakRef`), not DOM nodes, and there is no capture/bubble
//! chain walk — the target is the sole receiver. Keeping dispatch here
//! (rather than on the JS side) means the event model lives entirely in
//! Rust.

use std::{cell::RefCell, rc::Rc};

use crate::{dom::doc::SharedDoc, global, helpers::JsWeakRef};
use napi::{
    Env, Result, Status,
    bindgen_prelude::{FnArgs, JsObjectValue, Object},
};

/// Unified dispatcher for window/app lifecycle events.
///
/// Holds a weak ref to the JS `BlitzApp` (via `app_ref`); the window
/// target is resolved from `SharedDoc::js_window_ref`. Both are
/// refcount-0 weak references; JS keeps the objects alive on its side.
pub struct JsShellEventHandler {
    /// Weak ref to the JS `BlitzApp` object. Set lazily by
    /// `NativeApp::set_app_ref`.
    pub app_ref: Rc<RefCell<Option<JsWeakRef>>>,
}

impl JsShellEventHandler {
    pub fn new(app_ref: Rc<RefCell<Option<JsWeakRef>>>) -> Self {
        Self { app_ref }
    }

    /// Dispatch a cancelable event to the window. Returns `true` if the
    /// default was prevented. Used for the operation events `open` and
    /// `close`, where a listener can veto the action.
    pub fn dispatch_cancelable(&self, event_type: &str, doc: &Rc<SharedDoc>, env: &Env) -> bool {
        let Some(window) = resolve_window(doc, env) else {
            return false;
        };
        let mut event_obj = match build_event(event_type, true, env) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("napi-blitz: shell event factory failed for {event_type}: {e}");
                return false;
            }
        };
        let _ = dispatch_to_target(&window, &event_obj, env);
        let prevented = read_default_prevented(&event_obj, env);
        reset_dispatch_state(&mut event_obj, env);
        prevented
    }

    /// Dispatch a non-cancelable event to the window. Used for
    /// `closed` (and any future per-window notifications).
    pub fn dispatch_window_event(&self, event_type: &str, doc: &Rc<SharedDoc>, env: &Env) {
        let Some(window) = resolve_window(doc, env) else {
            return;
        };
        let mut event_obj = match build_event(event_type, false, env) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("napi-blitz: shell event factory failed for {event_type}: {e}");
                return;
            }
        };
        let _ = dispatch_to_target(&window, &event_obj, env);
        reset_dispatch_state(&mut event_obj, env);
    }

    /// Dispatch a non-cancelable event to the app. Used for
    /// `window:open`, `window:close`, `window:closed`.
    pub fn dispatch_app_event(&self, event_type: &str, env: &Env) {
        let Some(app) = resolve_app(&self.app_ref, env) else {
            return;
        };
        let mut event_obj = match build_event(event_type, false, env) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("napi-blitz: shell event factory failed for {event_type}: {e}");
                return;
            }
        };
        let _ = dispatch_to_target(&app, &event_obj, env);
        reset_dispatch_state(&mut event_obj, env);
    }

    /// Dispatch the full close sequence: `close` (cancelable) to the
    /// window, and if not prevented, `closed` to the window plus
    /// `window:close` + `window:closed` to the app. Returns `true` if
    /// the close should proceed (not prevented).
    pub fn close_sequence(&self, doc: &Rc<SharedDoc>, env: &Env) -> bool {
        if self.dispatch_cancelable("close", doc, env) {
            return false;
        }
        self.dispatch_window_event("closed", doc, env);
        self.dispatch_app_event("window:close", env);
        self.dispatch_app_event("window:closed", env);
        true
    }
}

// ── Dispatch primitives (shared with JsEventHandler) ───────────────────

/// Resolve the JS Window object from `SharedDoc::js_window_ref`.
fn resolve_window<'a>(doc: &Rc<SharedDoc>, env: &'a Env) -> Option<Object<'a>> {
    doc.js_window_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
}

/// Resolve the JS BlitzApp object from the app-level weak ref.
fn resolve_app<'a>(app_ref: &Rc<RefCell<Option<JsWeakRef>>>, env: &'a Env) -> Option<Object<'a>> {
    app_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
}

/// Build a JS `Event` object via the registered event factory.
fn build_event<'a>(event_type: &str, cancelable: bool, env: &'a Env) -> Result<Object<'a>> {
    use crate::dom::payload::EventPayload;
    let payload = EventPayload {
        event_type: event_type.to_string(),
        bubbles: false,
        cancelable,
        pointer: None,
        wheel: None,
        key: None,
        input: None,
        ime: None,
    };
    let factory_ref = global::get_event_factory()
        .ok_or_else(|| napi::Error::new(Status::GenericFailure, "event_factory not registered"))?;
    let factory_fn = factory_ref.borrow_back(env)?;
    let result_ref = factory_fn.call(FnArgs::from((payload,)))?;
    let result = result_ref.get_value(env)?;
    result_ref.unref(env)?;
    Ok(result)
}

/// Call `dispatchEvent(target, event)` via the registered dispatch fn.
fn dispatch_to_target(target: &Object, event: &Object, env: &Env) -> Result<()> {
    let dispatch_ref = global::get_dispatch_fn()
        .ok_or_else(|| napi::Error::new(Status::GenericFailure, "dispatch_fn not registered"))?;
    let dispatch_fn = dispatch_ref.borrow_back(env)?;
    let target_ref = target.create_ref::<true>()?;
    let event_ref = event.create_ref::<true>()?;
    dispatch_fn.call(FnArgs::from((target_ref, event_ref)))?;
    Ok(())
}

/// Read `event.defaultPrevented` via the registered getter.
fn read_default_prevented(event: &Object, env: &Env) -> bool {
    global::get_default_prevented_getter()
        .and_then(|dp_ref| dp_ref.borrow_back(env).ok())
        .and_then(|dp_fn| {
            let event_ref = event.create_ref::<true>().ok()?;
            dp_fn.call(FnArgs::from((event_ref,))).ok()
        })
        .unwrap_or(false)
}

/// Reset `currentTarget` to `null` and `eventPhase` to `0` (NONE).
fn reset_dispatch_state(event: &mut Object, _env: &Env) {
    let _ = event.set_named_property("currentTarget", ());
    let _ = event.set_named_property("eventPhase", 0u32);
}
