//! `JsShellEventHandler`: unified dispatch for window/app lifecycle events.
//!
//! Mirrors `JsEventHandler` (DOM events) but for shell-level events that
//! originate from winit's `ApplicationHandler` and the async `openWindow`
//! / `closeWindow` paths: `window:open`, `window:close`, `window:closed`.
//!
//! The `window:` prefix marks the event's subject (the window — a listener
//! on the app sees `window:close` as "a window is closing", never as "the
//! app is closing"). Window and app therefore observe the SAME event type
//! via an ancestor chain: a `window:*` event dispatched on a window
//! propagates up to the app — the window's shell ancestor — in bubble
//! order. One event object, one type, both levels can call
//! `preventDefault()` (veto the default action) or `stopPropagation()`.
//! The receiver at each level is `event.currentTarget`; the originating
//! window is `event.target`, fixed across the walk.
//!
//! Both event systems share the same dispatch primitives:
//! - `global::get_event_factory()` to build the JS `Event`
//! - `global::get_dispatch_fn()` to call `dispatchEvent(target, event)`
//! - `global::get_default_prevented_getter()` / `get_cancel_bubble_getter()`

use std::{cell::RefCell, rc::Rc};

use crate::{dom::doc::SharedDoc, global, helpers::JsWeakRef};
use napi::{
    Env, JsValue, Result, Status,
    bindgen_prelude::{FnArgs, FromNapiValue, Function, JsObjectValue, Object, Unknown},
};

const BUBBLING_PHASE: u32 = 3;

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

    /// Dispatch a single event object that propagates from the window up to
    /// the app (bubble order): the window receives it at target phase, then
    /// the app — the window's ancestor — receives the same object in the
    /// bubble phase. A listener at either level may call
    /// `preventDefault()` (read back as the return value) or
    /// `stopPropagation()` (skips the app level). Returns `true` if the
    /// default was prevented.
    pub fn dispatch_propagating(
        &self,
        event_type: &str,
        cancelable: bool,
        doc: &Rc<SharedDoc>,
        env: &Env,
    ) -> bool {
        let Some(window) = resolve_window(doc, env) else {
            return false;
        };
        let Some(app) = resolve_app(&self.app_ref, env) else {
            return false;
        };
        let mut event_obj = match build_event(event_type, cancelable, true, env) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("napi-blitz: shell event factory failed for {event_type}: {e}");
                return false;
            }
        };
        // event.target = the originating window, fixed across the walk.
        let _ = set_lazy_target(&event_obj, doc, env);

        // Window (target phase), then app (bubble phase) unless stopped.
        let stopped = dispatch_to_receiver(&event_obj, &window, false, doc, &self.app_ref, env);
        if !stopped {
            let _ = dispatch_to_receiver(&event_obj, &app, true, doc, &self.app_ref, env);
        }

        let prevented = read_default_prevented(&event_obj, env);
        reset_dispatch_state(&mut event_obj, env);
        prevented
    }

    /// Dispatch the cancelable `window:close` request through the ancestor
    /// chain. A listener on the window or the app may `preventDefault()`.
    /// Returns `true` if the close should proceed (not prevented).
    pub fn close_request(&self, doc: &Rc<SharedDoc>, env: &Env) -> bool {
        !self.dispatch_propagating("window:close", true, doc, env)
    }

    /// Dispatch the full close sequence: the cancelable `window:close`
    /// request, and — if not prevented — the post-teardown notification
    /// `window:closed` (also propagated window → app). Returns `true` if
    /// the close should proceed (not prevented).
    pub fn close_sequence(&self, doc: &Rc<SharedDoc>, env: &Env) -> bool {
        if !self.close_request(doc, env) {
            return false;
        }
        self.notify_closed(doc, env);
        true
    }

    /// Dispatch the post-teardown notification `window:closed` along the
    /// ancestor chain (window + app). Non-cancelable.
    pub fn notify_closed(&self, doc: &Rc<SharedDoc>, env: &Env) {
        self.dispatch_propagating("window:closed", false, doc, env);
    }

    /// Dispatch the full open sequence: at window-creation time (before
    /// `openWindow` resolves — JS does not yet hold a Window object), the
    /// app-level `window:open` event is dispatched to the app as a
    /// cancelable request. A listener's `preventDefault()` cancels the
    /// open and rejects the `openWindow` promise. Returns `true` if the
    /// open should proceed (not prevented).
    pub fn open_sequence(&self, env: &Env) -> bool {
        !self.dispatch_app_cancelable("window:open", env)
    }

    /// Dispatch a cancelable event to the app alone. Used for `window:open`
    /// — at that moment no window-level receiver exists yet (JS has no
    /// Window object until `openWindow` resolves). Returns `true` if the
    /// default was prevented.
    fn dispatch_app_cancelable(&self, event_type: &str, env: &Env) -> bool {
        let Some(app) = resolve_app(&self.app_ref, env) else {
            return false;
        };
        let mut event_obj = match build_event(event_type, true, false, env) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("napi-blitz: shell event factory failed for {event_type}: {e}");
                return false;
            }
        };
        let _ = set_lazy_target_app(&event_obj, &self.app_ref, env);
        let _ = set_lazy_current_target_app(&event_obj, &self.app_ref, BUBBLING_PHASE, env);
        let _ = dispatch_to_target(&app, &event_obj, env);
        let prevented = read_default_prevented(&event_obj, env);
        reset_dispatch_state(&mut event_obj, env);
        prevented
    }
}

// ── Dispatch primitives ────────────────────────────────────────────────

/// Resolve the JS Window object from `SharedDoc::js_window_ref`.
fn resolve_window<'a>(doc: &Rc<SharedDoc>, env: &'a Env) -> Option<Object<'a>> {
    doc.js_window_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
}

/// Resolve the JS BlitzApp object from the app-level weak ref.
fn resolve_app<'a>(
    app_ref: &Rc<RefCell<Option<JsWeakRef>>>,
    env: &'a Env,
) -> Option<Object<'a>> {
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

/// Dispatch the event to one receiver of the chain, setting its
/// `currentTarget`/`eventPhase`, and report whether the listener called
/// `stopPropagation()` (`cancelBubble`).
fn dispatch_to_receiver(
    event: &Object,
    receiver: &Object,
    is_app: bool,
    doc: &Rc<SharedDoc>,
    app_ref: &Rc<RefCell<Option<JsWeakRef>>>,
    env: &Env,
) -> bool {
    let _ = set_lazy_current_target(event, doc, app_ref, is_app, BUBBLING_PHASE, env);
    let _ = dispatch_to_target(receiver, event, env);
    read_cancel_bubble(event, env)
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

/// Read `event.cancelBubble` via the registered getter (true when a
/// listener called `stopPropagation()`).
fn read_cancel_bubble(event: &Object, env: &Env) -> bool {
    global::get_cancel_bubble_getter()
        .and_then(|cb_ref| cb_ref.borrow_back(env).ok())
        .and_then(|cb_fn| {
            let event_ref = event.create_ref::<true>().ok()?;
            cb_fn.call(FnArgs::from((event_ref,))).ok()
        })
        .unwrap_or(false)
}

/// Reset `currentTarget` to `null` and `eventPhase` to `0` (NONE) after
/// dispatch completes, per DOM spec.
fn reset_dispatch_state(event: &mut Object, _env: &Env) {
    let _ = event.set_named_property("currentTarget", ());
    let _ = event.set_named_property("eventPhase", 0u32);
}

// ── Lazy target/currentTarget setters ──────────────────────────────────

/// Set `event.target` to a lazy getter resolving the JS Window object.
fn set_lazy_target(event: &Object, doc: &Rc<SharedDoc>, env: &Env) -> Result<()> {
    let setter_ref = global::get_lazy_target_setter().ok_or_else(|| {
        napi::Error::new(Status::GenericFailure, "lazy_target_setter not registered")
    })?;
    let setter = setter_ref.borrow_back(env)?;
    let doc_clone = doc.clone();
    let getter: Function<(), Unknown> =
        env.create_function_from_closure("shell_target_getter", move |ctx| {
            let env_raw = ctx.env.raw();
            let obj = resolve_window(&doc_clone, &ctx.env)
                .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no window"))?;
            let raw = JsValue::raw(&obj);
            unsafe { Unknown::from_napi_value(env_raw, raw) }
        })?;
    let event_ref = event.create_ref::<true>()?;
    let getter_raw = JsValue::raw(&getter);
    let getter_unknown = unsafe { Object::from_napi_value(env.raw(), getter_raw) }?;
    let getter_ref = getter_unknown.create_ref::<true>()?;
    setter.call(FnArgs {
        data: (event_ref, getter_ref),
    })?;
    Ok(())
}

/// Set `event.target` to a lazy getter resolving the JS BlitzApp object.
fn set_lazy_target_app(
    event: &Object,
    app_ref: &Rc<RefCell<Option<JsWeakRef>>>,
    env: &Env,
) -> Result<()> {
    let setter_ref = global::get_lazy_target_setter().ok_or_else(|| {
        napi::Error::new(Status::GenericFailure, "lazy_target_setter not registered")
    })?;
    let setter = setter_ref.borrow_back(env)?;
    let app_clone = app_ref.clone();
    let getter: Function<(), Unknown> =
        env.create_function_from_closure("shell_target_app_getter", move |ctx| {
            let env_raw = ctx.env.raw();
            let obj = resolve_app(&app_clone, &ctx.env)
                .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no app"))?;
            let raw = JsValue::raw(&obj);
            unsafe { Unknown::from_napi_value(env_raw, raw) }
        })?;
    let event_ref = event.create_ref::<true>()?;
    let getter_raw = JsValue::raw(&getter);
    let getter_unknown = unsafe { Object::from_napi_value(env.raw(), getter_raw) }?;
    let getter_ref = getter_unknown.create_ref::<true>()?;
    setter.call(FnArgs {
        data: (event_ref, getter_ref),
    })?;
    Ok(())
}

/// Set `event.currentTarget` to a lazy getter resolving the window or the
/// app (per `is_app`) and set `event.eventPhase`.
fn set_lazy_current_target(
    event: &Object,
    doc: &Rc<SharedDoc>,
    app_ref: &Rc<RefCell<Option<JsWeakRef>>>,
    is_app: bool,
    phase: u32,
    env: &Env,
) -> Result<()> {
    let setter_ref = global::get_lazy_current_target_setter().ok_or_else(|| {
        napi::Error::new(
            Status::GenericFailure,
            "lazy_current_target_setter not registered",
        )
    })?;
    let setter = setter_ref.borrow_back(env)?;
    let doc_clone = doc.clone();
    let app_clone = app_ref.clone();
    let getter: Function<(), Unknown> =
        env.create_function_from_closure("shell_ct_getter", move |ctx| {
            let env_raw = ctx.env.raw();
            let obj = if is_app {
                resolve_app(&app_clone, &ctx.env)
            } else {
                resolve_window(&doc_clone, &ctx.env)
            }
            .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no receiver"))?;
            let raw = JsValue::raw(&obj);
            unsafe { Unknown::from_napi_value(env_raw, raw) }
        })?;
    let event_ref = event.create_ref::<true>()?;
    let getter_raw = JsValue::raw(&getter);
    let getter_unknown = unsafe { Object::from_napi_value(env.raw(), getter_raw) }?;
    let getter_ref = getter_unknown.create_ref::<true>()?;
    setter.call(FnArgs {
        data: (event_ref, getter_ref, phase),
    })?;
    Ok(())
}

/// Set `event.currentTarget` to a lazy getter resolving the app (used by
/// app-only events like `window:open`).
fn set_lazy_current_target_app(
    event: &Object,
    app_ref: &Rc<RefCell<Option<JsWeakRef>>>,
    phase: u32,
    env: &Env,
) -> Result<()> {
    let setter_ref = global::get_lazy_current_target_setter().ok_or_else(|| {
        napi::Error::new(
            Status::GenericFailure,
            "lazy_current_target_setter not registered",
        )
    })?;
    let setter = setter_ref.borrow_back(env)?;
    let app_clone = app_ref.clone();
    let getter: Function<(), Unknown> =
        env.create_function_from_closure("shell_ct_app_getter", move |ctx| {
            let env_raw = ctx.env.raw();
            let obj = resolve_app(&app_clone, &ctx.env)
                .ok_or_else(|| napi::Error::new(Status::GenericFailure, "no app"))?;
            let raw = JsValue::raw(&obj);
            unsafe { Unknown::from_napi_value(env_raw, raw) }
        })?;
    let event_ref = event.create_ref::<true>()?;
    let getter_raw = JsValue::raw(&getter);
    let getter_unknown = unsafe { Object::from_napi_value(env.raw(), getter_raw) }?;
    let getter_ref = getter_unknown.create_ref::<true>()?;
    setter.call(FnArgs {
        data: (event_ref, getter_ref, phase),
    })?;
    Ok(())
}
