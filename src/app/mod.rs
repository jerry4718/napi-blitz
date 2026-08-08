//! `BlitzApp`: the JS-facing wrapper around a winit event loop.
//!
//! `BlitzApp.create()` builds an event loop. Calling `openWindow(docHandle)`
//! produces a `Box<dyn Document>` from the handle and attaches a fresh window
//! to it. JS drives the loop synchronously via `pumpAppEvents(millis)` from
//! the main thread; this keeps event callbacks re-entrant on the napi env so
//! we can call back into JS without a ThreadsafeFunction.

mod bridge;
mod handler;

use std::{
    cell::RefCell, collections::HashMap, rc::Rc, sync::Arc, sync::mpsc::Receiver, time::Duration,
};

use crate::window::WindowInner;
use crate::{
    app::{
        bridge::{APP_EVENT_CLOSED, AppDispatchResult, AppEventPayload, JsAppBridge},
        handler::AppHandler,
    },
    dom::doc::{NativeDoc, make_window_document},
    renderer::CurrentRenderer,
    window::{
        NativeWindow, WindowOptions, build_window_attributes,
        monitor::{MonitorInfo, monitor_to_info},
    },
};
use blitz::{
    shell::{
        BlitzShellEvent, BlitzShellProxy, EventLoop, View, WindowConfig, create_default_event_loop,
    },
    traits::shell::DummyShellProvider,
};
use napi::{
    Env, Error, Result,
    bindgen_prelude::{BigInt, Function, FunctionRef},
};
use winit::{
    event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    window::WindowId,
};

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
    event_loop: EventLoop,
    pub(crate) inner: AppInner,
}

/// A live window: the blitz `View` plus the JS-side `Window` handle
/// that holds an `Arc<dyn Window>`. Dropping the view alone does not
/// release the winit window if the JS `Window` still holds a clone,
/// so `WindowEntry::close` takes the Arc out before dropping the view.
pub(crate) struct WindowEntry {
    pub(crate) view: View<CurrentRenderer>,
    pub(crate) inner: Rc<RefCell<WindowInner>>,
}

impl WindowEntry {
    fn close(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.window = None;
        inner.closed = true;
        drop(inner);
        self.view
            .doc
            .inner_mut()
            .set_shell_provider(Arc::new(DummyShellProvider));
    }
}

pub(crate) struct AppInner {
    /// Live windows keyed by winit `WindowId`.
    pub(crate) windows: HashMap<WindowId, WindowEntry>,
    /// Window configs requested via `openWindow` but not yet promoted to live `View`s.
    pub(crate) pending: Vec<WindowConfig<CurrentRenderer>>,
    /// Proxy for sending events into the event loop (redraw, poll, etc.).
    pub(crate) proxy: BlitzShellProxy,
    /// Receiver for `BlitzShellEvent`s from the proxy channel.
    pub(crate) event_queue: Receiver<BlitzShellEvent>,
    /// Window ids requested to close from JS. We intentionally defer live
    /// `View` removal until after the current `pumpAppEvents` call has
    /// returned from winit event dispatch. This makes `window.close()`
    /// safe to call from within that same window's click handler.
    pub(crate) closing_window_ids: Vec<WindowId>,
    /// JS-side bridge for app/window events (close / closed). Set
    /// lazily by `setAppEventHandler`; absent until JS opts in.
    pub(crate) bridge: Option<JsAppBridge>,
    /// Number of windows currently considered "alive". Incremented
    /// on `openWindow`, decremented in the `close_window` path when we
    /// successfully remove a window from `windows` and in the native
    /// `CloseRequested` path via `JsAppHandler::outstanding`.
    pub(crate) outstanding_windows: usize,
    /// True once at least one window has ever been opened. Without
    /// this, calling `pump_app_events` before any `open_window` would
    /// wrongly synthesise an exit on the very first pump.
    pub(crate) has_opened_window: bool,
}

#[napi]
impl NativeApp {
    /// Build the winit event loop.
    #[napi(factory)]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        Self {
            event_loop,
            inner: AppInner {
                windows: HashMap::new(),
                pending: Vec::new(),
                proxy,
                event_queue: receiver,
                closing_window_ids: Vec::new(),
                bridge: None,
                outstanding_windows: 0,
                has_opened_window: false,
            },
        }
    }

    /// Install (or replace) the JS callback that receives app/window
    /// events. JS wires this in its `BlitzApp` constructor; calling
    /// again replaces the previous handler.
    ///
    /// The callback receives an `AppEventPayload` and must return an
    /// `AppDispatchResult` reporting whether the JS-side `Event` had
    /// `preventDefault()` called on it.
    #[napi]
    pub fn set_app_event_handler(
        &mut self,
        env: Env,
        callback: Function<AppEventPayload, AppDispatchResult>,
    ) -> Result<()> {
        let callback_ref: FunctionRef<AppEventPayload, AppDispatchResult> =
            callback.create_ref()?;
        self.inner.bridge = Some(JsAppBridge::new(env, callback_ref));
        Ok(())
    }

    /// Attach a new window to the given document handle. The same handle can
    /// only be attached to one window. The JS DocHandle keeps working after
    /// this call (it shares state with the window via Rc<RefCell<...>>), so
    /// JS can keep mutating the DOM after `openWindow`.
    ///
    /// `options` maps directly to a winit `WindowAttributes`. If the document
    /// carries a `<title>` element, blitz's mutator-flush will overwrite the
    /// title shortly after open; this is expected, with the document treated
    /// as the source of truth for window-title content.
    ///
    /// The returned `Window` carries the winit `WindowId` of the created
    /// window, which we use as the napi-side window identifier.
    #[napi]
    pub fn open_window(
        &mut self,
        doc: &mut NativeDoc,
        options: Option<&WindowOptions>,
    ) -> Result<NativeWindow> {
        if !doc.mark_attached() {
            return Err(Error::from_reason(
                "DocHandle has already been attached to a window".to_string(),
            ));
        }
        let window_doc = make_window_document(doc);
        let attributes = build_window_attributes(options)?;
        let config = WindowConfig::with_attributes(window_doc, CurrentRenderer::new(), attributes);
        self.inner.pending.push(config);
        self.inner.has_opened_window = true;
        self.inner.outstanding_windows += 1;

        // winit only assigns a WindowId while dispatching through an active
        // event loop. Run one non-blocking pump so the window is created, then
        // grab the Arc<dyn Window> and WindowId straight from the view.
        let before: Vec<WindowId> = self.inner.windows.keys().copied().collect();
        self.pump_app_events(0.0);
        let entry = self
            .inner
            .windows
            .iter()
            .find(|(id, _)| !before.contains(id))
            .map(|(_, entry)| entry as &WindowEntry)
            .ok_or_else(|| Error::from_reason("failed to create native window"))?;

        Ok(NativeWindow {
            window_id: entry.view.window.id(),
            inner: Rc::clone(&entry.inner),
        })
    }

    /// Synchronously close the given window. Removes it from the
    /// application's window map (or from our pending queue if it has not
    /// been initialised yet). The window stops painting and receiving
    /// events as soon as this call returns.
    ///
    /// This is intentionally not GC-driven: dropping the JS `Window` object
    /// does not close the OS window. Callers must invoke this explicitly.
    #[napi]
    pub fn close_window(&mut self, window: &mut NativeWindow) {
        let window_id = window.window_id;
        let mut inner = window.inner.borrow_mut();

        // Public JS API guarantee: close() is idempotent. Multiple calls are
        // common when listeners race with UI state updates, so only the first
        // one has side effects.
        if inner.closed || self.inner.closing_window_ids.contains(&window_id) {
            inner.closed = true;
            return;
        }

        let was_initialised = self.inner.windows.contains_key(&window_id);
        if was_initialised {
            self.inner.closing_window_ids.push(window_id);
        }

        inner.closed = true;
        inner.window = None;
        drop(inner);

        if was_initialised {
            self.inner.outstanding_windows = self.inner.outstanding_windows.saturating_sub(1);
        }

        // Live windows are notified from `flush_closing_windows`,
        // after any in-progress winit/blitz document event dispatch has fully
        // unwound.
    }

    // -- Per-window runtime configuration -----------------------------------
    //
    // The napi `Window` handle does not own a reference to the live winit
    // `Arc<dyn Window>`; the `BlitzApplication` does. So all per-window
    // setters/getters live on `BlitzApp` and look the view up by window_id.
    // The JS-side `Window` class delegates through these.

    /// List all available monitors with full metadata. Returns `[]` if
    /// no windows have been created yet.
    #[napi]
    pub fn available_monitors(&self) -> Vec<MonitorInfo> {
        let Some(entry) = self.inner.windows.values().next() else {
            return Vec::new();
        };
        entry
            .view
            .window
            .available_monitors()
            .map(monitor_to_info)
            .collect()
    }

    /// The primary monitor. Returns `None` if no windows have been
    /// created yet.
    #[napi]
    pub fn primary_monitor(&self) -> Option<MonitorInfo> {
        let entry = self.inner.windows.values().next()?;
        entry.view.window.primary_monitor().map(monitor_to_info)
    }

    /// Pump pending winit events for at most `millis` milliseconds.
    #[napi]
    pub fn pump_app_events(&mut self, millis: f64) -> PumpResult {
        self.pump_app_events_inner(millis)
    }

    /// Set the document zoom level. `1.0` is unzoomed. Combined with the
    /// system scale factor to produce the total viewport scale
    /// (`hidpi_scale * zoom`) that scales layout and CSS transforms.
    #[napi]
    pub fn set_zoom(&mut self, window: &NativeWindow, zoom: f64) -> Result<()> {
        let entry = self
            .inner
            .windows
            .get_mut(&window.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        entry.view.with_viewport(|v| v.set_zoom(zoom as f32));
        Ok(())
    }

    /// Get the current document zoom level.
    #[napi]
    pub fn get_zoom(&self, window: &NativeWindow) -> Result<f32> {
        let entry = self
            .inner
            .windows
            .get(&window.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        Ok(entry.view.doc.inner().viewport().zoom())
    }
}

impl NativeApp {
    fn poll_live_views(&mut self) {
        for entry in self.inner.windows.values_mut() {
            entry.view.poll();
        }
    }

    fn flush_closing_windows(&mut self) {
        if self.inner.closing_window_ids.is_empty() {
            return;
        }

        let closing_window_ids = std::mem::take(&mut self.inner.closing_window_ids);
        for window_id in closing_window_ids {
            if let Some(mut entry) = self.inner.windows.remove(&window_id) {
                entry.close();
                drop(entry);
            }

            if let Some(bridge) = self.inner.bridge.as_ref() {
                let _ = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSED.to_string(),
                    window_id: BigInt::from(window_id.into_raw() as u64),
                    cancelable: false,
                });
            }
        }
    }

    /// Pump pending winit events for at most `millis` milliseconds. JS should
    /// call this in a loop (typically once per animation frame) to drive the
    /// renderer and event handling.
    fn pump_app_events_inner(&mut self, millis: f64) -> PumpResult {
        // Give host-driven DOM mutations from the previous JS turn a chance to
        // flow through Blitz's normal `View::poll -> Document::poll ->
        // request_redraw` path before winit waits for more events.
        self.poll_live_views();

        // Pending windows are promoted to live Views by `JsAppHandler::drain_pending_windows`
        // during the pump. No need to hand them to an intermediate application layer.

        // A caller may invoke `window.close()` between pump ticks. In that
        // case no winit/blitz document dispatch is active, so it is safe and
        // necessary to drop the queued views before the synthetic-exit check
        // below observes `outstanding_windows == 0`.
        self.flush_closing_windows();

        // If at least one window has ever been opened and every
        // window has now been closed via JS, surface a synthetic
        // Exit. winit's `pump_app_events` mode never exits on its
        // own; the OS-initiated `CloseRequested` path already
        // triggers `event_loop.exit()` from inside
        // `BlitzApplication::window_event`, but JS-initiated
        // `BlitzApp::close_window` bypasses winit's pipeline entirely.
        if self.inner.has_opened_window && self.inner.outstanding_windows == 0 {
            return PumpResult {
                r#continue: false,
                exit: true,
                code: Some(0),
            };
        }

        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));

        let mut handler = AppHandler {
            inner: &mut self.inner,
        };
        let status = self.event_loop.pump_app_events(timeout, &mut handler);
        self.flush_closing_windows();
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
