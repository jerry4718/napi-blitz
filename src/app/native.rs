//! `BlitzApp`: the JS-facing wrapper around a winit event loop.
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
    window::{
        NativeWindow,
        monitor::{MonitorInfo, monitor_to_info},
        options::WindowOptions,
    },
};
use std::{cell::RefCell, rc::Rc, time::Duration};

use blitz::shell::{BlitzShellProxy, EventLoop, create_default_event_loop};
use napi::{
    Env, Error, Result,
    bindgen_prelude::{Object, PromiseRaw, Undefined},
};
use napi_helpers::inherits::with_own;
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

#[napi]
pub struct NativeApp {
    event_loop: RefCell<EventLoop>,
    lifecycle: Rc<Lifecycle>,
}

#[napi]
impl NativeApp {
    /// Build the winit event loop.
    #[napi(factory)]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        Self {
            event_loop: RefCell::new(event_loop),
            lifecycle: Rc::new(Lifecycle::new(proxy, receiver)),
        }
    }

    /// Store a weak ref to the JS `BlitzApp` object so Rust can
    /// dispatch app-level lifecycle events (`window:open`,
    /// `window:close`, `window:closed`) to it.
    #[napi]
    pub fn set_app_ref(&self, env: Env, app: Object) -> Result<()> {
        self.lifecycle.set_app_ref(env, app)
    }

    /// Attach a new window to the given document. The same document
    /// handle can only be attached to one window. The JS Document keeps
    /// working after this call (it shares state with the window via
    /// Rc<RefCell<...>>), so JS can keep mutating the DOM after
    /// `openWindow`.
    ///
    /// `options` maps directly to a winit `WindowAttributes`. If the
    /// document carries a `<title>` element, blitz's mutator-flush will
    /// overwrite the title shortly after open; this is expected, with
    /// the document treated as the source of truth for window-title
    /// content.
    ///
    /// Returns a `Promise<NativeWindow>` that resolves once a
    /// `pump_app_events` call promotes the request to a live window.
    /// This method never drives the event loop itself, which makes it
    /// safe to invoke from inside an event handler; because creation is
    /// deferred to the caller's pump, the caller must ensure a pump is
    /// (or will be) running before `await`-ing the result - otherwise
    /// the promise never resolves.
    #[napi]
    pub fn open_window(
        &self,
        env: Env,
        doc: Object,
        options: Option<&WindowOptions>,
    ) -> Result<PromiseRaw<'_, NativeWindow>> {
        let shared_doc = with_own::<DocumentLayer, _>(&doc, |d| d.shared.clone())?;
        self.lifecycle.open_window(env, shared_doc, options)
    }

    /// Queue the given window for closure and return a promise that
    /// resolves once the native `View` has actually been torn down
    /// (during the next pump), or rejects if a `close` listener calls
    /// `preventDefault()`. The cancelable `close` event is dispatched
    /// at the moment the close is requested; if not prevented, the
    /// JS-side `closed` flag is set immediately and only the physical
    /// teardown is async. `close()` is idempotent.
    ///
    /// This is intentionally not GC-driven: dropping the JS `Window`
    /// object does not close the OS window. Callers must invoke this
    /// explicitly.
    #[napi]
    pub fn close_window(
        &self,
        env: Env,
        window: &NativeWindow,
    ) -> Result<PromiseRaw<'_, Undefined>> {
        self.lifecycle.request_close(env, window)
    }

    // -- Per-window runtime configuration -----------------------------------
    //
    // The napi `Window` handle does not own a reference to the live winit
    // `Arc<dyn Window>`; the lifecycle state does. So all per-window
    // setters/getters live on `BlitzApp` and look the view up by window_id.
    // The JS-side `Window` class delegates through these.

    /// List all available monitors with full metadata. Returns `[]` if
    /// no windows have been created yet.
    #[napi]
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
    #[napi]
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
    #[napi]
    pub fn pump_app_events(&self, millis: f64) -> PumpResult {
        self.pump_app_events_inner(millis)
    }

    /// Set the document zoom level. `1.0` is unzoomed. Combined with the
    /// system scale factor to produce the total viewport scale
    /// (`hidpi_scale * zoom`) that scales layout and CSS transforms.
    #[napi]
    pub fn set_zoom(&self, window: &NativeWindow, zoom: f64) -> Result<()> {
        let state = self.lifecycle.state().borrow();
        let entry = state
            .windows
            .get(&window.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        entry
            .view
            .borrow_mut()
            .with_viewport(|v| v.set_zoom(zoom as f32));
        Ok(())
    }

    /// Get the current document zoom level.
    #[napi]
    pub fn get_zoom(&self, window: &NativeWindow) -> Result<f32> {
        let state = self.lifecycle.state().borrow();
        let entry = state
            .windows
            .get(&window.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        Ok(entry.view.borrow().doc.inner().viewport().zoom())
    }
}

impl NativeApp {
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
