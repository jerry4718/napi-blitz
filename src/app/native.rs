//! `BlitzApp`: the layer wrapper around a winit event loop.
//!
//! The napi boundary: every method translates JS arguments into one
//! `Lifecycle` call and wraps the result in a promise. The lifecycle
//! flow itself - opens, closes, teardowns, the synthetic exit - lives
//! in `Lifecycle`; the window facts live in `AppState`.
//!
//! JS drives the loop synchronously via `pumpAppEvents(millis)` from
//! the main thread; this keeps event callbacks re-entrant on the napi
//! env so we can call back into JS without a ThreadsafeFunction.
//!
//! # Async `openWindow` / `closeWindow`
//!
//! Both are async: they only queue the request and return a promise,
//! never driving the event loop themselves. Window creation happens
//! inside a *later* `pump_app_events` (via
//! `Lifecycle::drain_opening_windows`), where winit hands us an
//! `ActiveEventLoop`; a close requested from inside an event handler is
//! fulfilled by the current or next pump's `drain_closing_windows`.
//! This is what makes opening or closing a window from a click handler
//! safe - there is no nested event-loop recursion.

use crate::{
    app::{handler::AppHandler, lifecycle::Lifecycle},
    dom::DocumentLayer,
    events::base::EventTargetLayer,
    window::{
        WindowLayer,
        monitor::{MonitorInfo, monitor_to_info},
        options::WindowOptions,
    },
};
use std::{cell::RefCell, rc::Rc, time::Duration};

use blitz::shell::{BlitzShellProxy, EventLoop, create_default_event_loop};
use napi::{
    Env, Error, Result,
    bindgen_prelude::{FromNapiValue, JsValue, Object, ObjectRef, PromiseRaw, Undefined},
    check_status, sys,
};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, Super, layer_chain, new_from_chain, proc::layer, with_own},
};
use winit::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};

/// Result of one `pumpAppEvents` call.
#[napi(object)]
pub struct PumpResult {
    /// The loop is still running. Caller should pump again later.
    pub r#continue: bool,
    /// The loop has exited (e.g. all windows closed).
    pub exit: bool,
    /// Exit code, if `exit`.
    pub code: Option<i32>,
}

/// Own block of the `BlitzApp` class.
#[layer]
pub struct BlitzAppLayer {
    event_loop: RefCell<EventLoop>,
    lifecycle: Rc<Lifecycle>,
}

#[layer(js_name = "BlitzApp")]
impl BlitzAppLayer {
    #[layer(parent)]
    type Parent = EventTargetLayer;

    #[layer(constructor)]
    fn build(_sup: Super<EventTargetLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "construct a BlitzApp via BlitzApp.create()",
        ))
    }

    /// Build the winit event loop and register this app as the lifecycle
    /// dispatch target for app-level events (`window:open` and friends).
    #[layer]
    pub fn create(env: &Env) -> Result<ObjectRef> {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        let lifecycle = Rc::new(Lifecycle::new(Env::clone(env), proxy, receiver));
        let app = new_from_chain::<BlitzAppLayer>(
            env,
            layer_chain!(
                EventTargetLayer::fresh(),
                BlitzAppLayer {
                    event_loop: RefCell::new(event_loop),
                    lifecycle: Rc::clone(&lifecycle),
                },
            ),
        )?;
        lifecycle.set_app_ref(app)?;
        let app_ref = unsafe { ObjectRef::from_napi_value(env.raw(), JsValue::raw(&app))? };
        Ok(app_ref)
    }

    /// Open a new window for an existing `HTMLDocument`.
    /// Construct window attributes with `WindowOptions.builder()`.
    ///
    /// Async: the window is physically created by the next event-loop pump, so
    /// this resolves once the OS window exists. Safe to call from inside an
    /// event handler (e.g. a click) — the native side never recursively
    /// pumps the event loop.
    ///
    /// Rust dispatches the cancelable app-level `window:open` event while
    /// creating the window, before this promise resolves. A listener's
    /// `preventDefault()` rejects this promise (the native side drops the
    /// fresh view, so no `Window` is ever handed out).
    #[layer]
    pub fn open_window(
        &self,
        env: &Env,
        doc: Object,
        options: Option<Object>,
    ) -> Result<PromiseRaw<'static, Anything>> {
        let shared_doc = with_own::<DocumentLayer, _>(&doc, |d| d.shared.clone())?;
        // The layer trampoline converts borrowed arguments (`&T`) through
        // napi-rs's native-borrow scope, which it does not set; unwrap the
        // `WindowOptions` instance to its Rust value directly instead.
        let options = options
            .as_ref()
            .map(|obj| Self::window_options_ref(env, obj))
            .transpose()?;
        self.lifecycle.open_window(shared_doc, options)
    }

    /// Queue the given window for closure and return a promise that
    /// resolves once the native `View` has actually been torn down, or
    /// rejects if a `close` listener calls `preventDefault()`.
    #[layer]
    pub fn close_window(&self, window: Object) -> Result<PromiseRaw<'static, Undefined>> {
        with_own::<WindowLayer, _>(&window, |d| d.close())?
    }

    /// List all available monitors with full metadata. Returns `[]` if
    /// no windows have been created yet.
    #[layer]
    pub fn available_monitors(&self) -> Vec<MonitorInfo> {
        let state = self.lifecycle.state().borrow();
        let Some(entry) = state.windows.values().next() else {
            return Vec::new();
        };
        entry
            .view
            .borrow()
            .window
            .available_monitors()
            .map(monitor_to_info)
            .collect()
    }

    /// The primary monitor. Returns `None` if no windows have been
    /// created yet.
    #[layer]
    pub fn primary_monitor(&self) -> Option<MonitorInfo> {
        let state = self.lifecycle.state().borrow();
        let entry = state.windows.values().next()?;
        entry
            .view
            .borrow()
            .window
            .primary_monitor()
            .map(monitor_to_info)
    }

    /// Pump pending winit events for at most `millis` milliseconds.
    #[layer]
    pub fn pump_app_events(&self, millis: f64) -> PumpResult {
        self.pump_app_events_inner(millis)
    }
}

impl BlitzAppLayer {
    /// Unwrap a `WindowOptions` class instance to its Rust value without
    /// going through napi-rs's borrowed-argument conversion (which
    /// requires a native-borrow scope the layer trampoline does not set).
    fn window_options_ref<'a>(env: &Env, obj: &Object) -> Result<&'a WindowOptions> {
        let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        check_status!(unsafe { sys::napi_unwrap(env.raw(), obj.raw(), &mut ptr) })?;
        if ptr.is_null() {
            return Err(Error::from_reason(
                "argument is not a WindowOptions instance",
            ));
        }
        Ok(unsafe { &*(ptr as *const WindowOptions) })
    }

    fn poll_live_views(&self) {
        let state = self.lifecycle.state().borrow();
        for entry in state.windows.values() {
            entry.view.borrow_mut().poll();
        }
    }

    /// Pump pending winit events for at most `millis` milliseconds. JS should
    /// call this in a loop (typically once per animation frame) to drive the
    /// renderer and event handling.
    fn pump_app_events_inner(&self, millis: f64) -> PumpResult {
        // Give host-driven DOM mutations from the previous JS turn a chance to
        // flow through Blitz's normal `View::poll -> Document::poll ->
        // request_redraw` path before winit waits for more events.
        self.poll_live_views();

        // Pending windows are promoted to live Views by
        // `Lifecycle::drain_opening_windows` during the pump.

        // A caller may invoke `window.close()` between pump ticks. In that
        // case no winit/blitz document dispatch is active, so it is safe and
        // necessary to drop the queued views before the synthetic-exit check
        // below observes zero outstanding windows.
        self.lifecycle.drain_closing_windows();

        if self.lifecycle.should_synthetic_exit() {
            return PumpResult {
                r#continue: false,
                exit: true,
                code: Some(0),
            };
        }

        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));

        let mut handler = AppHandler {
            lifecycle: Rc::clone(&self.lifecycle),
        };
        let status = self
            .event_loop
            .borrow_mut()
            .pump_app_events(timeout, &mut handler);
        self.lifecycle.drain_closing_windows();
        self.poll_live_views();

        match status {
            PumpStatus::Continue => PumpResult {
                r#continue: true,
                exit: false,
                code: None,
            },
            PumpStatus::Exit(code) => PumpResult {
                r#continue: false,
                exit: true,
                code: Some(code),
            },
        }
    }
}
