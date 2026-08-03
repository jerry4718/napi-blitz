//! `BlitzApp`: the JS-facing wrapper around blitz's winit-based application.
//!
//! `BlitzApp.create()` builds an event loop and a `BlitzApplication`. Calling
//! `openWindow(docHandle)` produces a `Box<dyn Document>` from the handle and
//! attaches a fresh window to it. JS drives the loop synchronously via
//! `pumpAppEvents(millis)` from the main thread; this keeps event callbacks
//! re-entrant on the napi env so we can call back into JS without a
//! ThreadsafeFunction.

use std::time::Duration;

use anyrender_vello::VelloWindowRenderer;
use blitz::shell::{
    BlitzApplication, BlitzShellProxy, EventLoop, WindowConfig, create_default_event_loop,
};
use blitz::traits::shell::DummyShellProvider;
use napi::{
    Env, Error, Result,
    bindgen_prelude::{BigInt, Function, FunctionRef},
};
use napi_derive::napi;
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::WindowAttributes;

use crate::dom::doc::{DocHandle, make_window_document};
use crate::native_window::app_bridge::{
    APP_EVENT_CLOSED, AppDispatchResult, AppEventPayload, JsAppBridge,
};
use crate::native_window::app_handler::JsAppHandler;
use crate::native_window::window::{Window, WindowOptions};

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
pub struct BlitzApp {
    event_loop: EventLoop,
    application: BlitzApplication<VelloWindowRenderer>,
    /// Window configs that have been requested via `openWindow` but not yet
    /// handed to the underlying `BlitzApplication`. We hold them ourselves so
    /// `closeWindow` can synchronously cancel a window that has not been
    /// initialised yet (winit's `can_create_surfaces` only runs during
    /// `pumpAppEvents`). Each entry is `(doc_id, config)`.
    pending: Vec<(usize, WindowConfig<VelloWindowRenderer>)>,
    /// Doc ids requested to close from JS. We intentionally defer live
    /// `View` removal until after the current `pump_app_events` call has
    /// returned from winit/blitz event dispatch. This makes `window.close()`
    /// safe to call from within that same window's click handler: blitz's
    /// `EventDriver` may still need to borrow the document after JS listeners
    /// return to run default actions.
    closing_doc_ids: Vec<usize>,
    /// JS-side bridge for app/window events (close / closed). Set
    /// lazily by `setAppEventHandler`; absent until JS opts in.
    bridge: Option<JsAppBridge>,
    /// Number of windows currently considered "alive". Incremented
    /// on `openWindow`, decremented in the `close_window` path when we
    /// successfully remove a window from `application.windows` and in
    /// the native `CloseRequested` path via
    /// `JsAppHandler::outstanding`.
    outstanding_windows: usize,
    /// True once at least one window has ever been opened. Without
    /// this, calling `pump_app_events` before any `open_window` would
    /// wrongly synthesise an exit on the very first pump.
    has_opened_window: bool,
}

#[napi]
impl BlitzApp {
    /// Build the winit event loop and underlying blitz application.
    #[napi(factory)]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        let application = BlitzApplication::new(proxy, receiver);
        Self {
            event_loop,
            application,
            pending: Vec::new(),
            closing_doc_ids: Vec::new(),
            bridge: None,
            outstanding_windows: 0,
            has_opened_window: false,
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
        self.bridge = Some(JsAppBridge::new(env, callback_ref));
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
    /// The returned `Window` carries the `doc_id` of the attached document,
    /// which we use as the napi-side window identifier. Note that winit's
    /// real `WindowId` is only minted on the next `pump_app_events` call,
    /// so the doc_id is what we key on for synchronous open/close.
    #[napi]
    pub fn open_window(
        &mut self,
        doc: &mut DocHandle,
        options: Option<WindowOptions>,
    ) -> Result<Window> {
        if !doc.mark_attached() {
            return Err(Error::from_reason(
                "DocHandle has already been attached to a window".to_string(),
            ));
        }
        let doc_id = doc.doc_id();
        let window_doc = make_window_document(doc);
        let attributes = build_window_attributes(options)?;
        let config =
            WindowConfig::with_attributes(window_doc, VelloWindowRenderer::new(), attributes);
        self.pending.push((doc_id, config));
        self.has_opened_window = true;
        self.outstanding_windows += 1;
        Ok(Window {
            doc_id,
            closed: false,
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
    pub fn close_window(&mut self, window: &mut Window) {
        let doc_id = window.doc_id;

        // Public JS API guarantee: close() is idempotent. Multiple calls are
        // common when listeners race with UI state updates, so only the first
        // one has side effects.
        if window.closed || self.closing_doc_ids.contains(&doc_id) {
            window.closed = true;
            return;
        }

        // Drop matching pending config (window opened but not yet pumped).
        // After `pump_app_events`, the config has been handed to the
        // `JsAppHandler` which promotes it to a live `View` inside
        // `application.windows` via `View::init`, so the
        // `application.windows.retain` below catches the
        // post-pump case.
        let was_pending = self.pending.iter().any(|(id, _)| *id == doc_id);
        self.pending.retain(|(id, _)| *id != doc_id);

        let was_initialised = self.has_initialised_window(doc_id);
        if was_initialised {
            self.closing_doc_ids.push(doc_id);
        }

        let removed = was_pending || was_initialised;

        window.closed = true;
        if removed {
            self.outstanding_windows = self.outstanding_windows.saturating_sub(1);
        }

        // Pending windows never enter the event loop, so it is safe to notify
        // immediately. Live windows are notified from `flush_closing_windows`,
        // after any in-progress winit/blitz document event dispatch has fully
        // unwound.
        if was_pending
            && !was_initialised
            && let Some(bridge) = self.bridge.as_ref()
        {
            let _ = bridge.dispatch(AppEventPayload {
                event_type: APP_EVENT_CLOSED.to_string(),
                window_doc_id: BigInt::from(doc_id as u64),
                cancelable: false,
            });
        }
    }

    // -- Per-window runtime configuration -----------------------------------
    //
    // The napi `Window` handle does not own a reference to the live winit
    // `Arc<dyn Window>`; the `BlitzApplication` does. So all per-window
    // setters/getters live on `BlitzApp` and look the view up by doc_id.
    // The JS-side `Window` class delegates through these.

    /// winit `Window::request_surface_size`. The actual size that the
    /// platform settles on can differ from the request; callers should
    /// rely on the `surface-resize` events (driven by winit) to reflect
    /// the truth.
    ///
    /// The JS-facing boundary intentionally accepts `f64`, matching JS
    /// `number`, instead of `u32`: napi's unsigned integer conversion would
    /// silently apply ToUint32 semantics to negatives/fractions. We validate
    /// the double ourselves and only then pass a `PhysicalSize<u32>` to winit.
    #[napi]
    pub fn set_window_inner_size(
        &mut self,
        window: &Window,
        width: f64,
        height: f64,
    ) -> Result<()> {
        let width = parse_surface_dimension("width", width)?;
        let height = parse_surface_dimension("height", height)?;

        if let Some(view) = self.window_view(window) {
            let _ = view
                .window
                .request_surface_size(PhysicalSize::new(width, height).into());
        }
        Ok(())
    }

    /// winit `Window::surface_size`. Returns `[width, height]` in
    /// physical pixels, or `None` if the window has not been created
    /// yet or has been closed.
    #[napi]
    pub fn get_window_inner_size(&self, window: &Window) -> Option<Vec<u32>> {
        let view = self.window_view_ref(window)?;
        let size = view.window.surface_size();
        Some(vec![size.width, size.height])
    }

    /// winit `Window::set_resizable`.
    #[napi]
    pub fn set_window_resizable(&mut self, window: &Window, resizable: bool) {
        if let Some(view) = self.window_view(window) {
            view.window.set_resizable(resizable);
        }
    }

    /// winit `Window::is_resizable`. Returns `None` if the window has
    /// not been created yet or has been closed.
    #[napi]
    pub fn get_window_resizable(&self, window: &Window) -> Option<bool> {
        Some(self.window_view_ref(window)?.window.is_resizable())
    }

    /// Look up the live `View` for a `Window` handle, by doc id.
    fn window_view(
        &mut self,
        window: &Window,
    ) -> Option<&mut blitz::shell::View<VelloWindowRenderer>> {
        self.application
            .windows
            .values_mut()
            .find(|v| v.doc.id() == window.doc_id)
    }

    /// Read-only counterpart to `window_view`.
    fn window_view_ref(&self, window: &Window) -> Option<&blitz::shell::View<VelloWindowRenderer>> {
        self.application
            .windows
            .values()
            .find(|v| v.doc.id() == window.doc_id)
    }

    fn has_initialised_window(&self, doc_id: usize) -> bool {
        self.application
            .windows
            .values()
            .any(|view| view.doc.id() == doc_id)
    }

    fn poll_live_views(&mut self) {
        for view in self.application.windows.values_mut() {
            view.poll();
        }
    }

    fn flush_closing_windows(&mut self) {
        if self.closing_doc_ids.is_empty() {
            return;
        }

        let closing_doc_ids = std::mem::take(&mut self.closing_doc_ids);
        for doc_id in closing_doc_ids {
            let Some(window_id) = self
                .application
                .windows
                .iter()
                .find_map(|(window_id, view)| (view.doc.id() == doc_id).then_some(*window_id))
            else {
                continue;
            };

            if let Some(mut view) = self.application.windows.remove(&window_id) {
                // `View::init` stores a `BlitzShellProvider` in the document.
                // That provider owns an `Arc<dyn winit::Window>`, so simply
                // dropping `View` is not enough to make the OS window go away.
                // Swap the provider back to the dummy implementation first so
                // the winit window Arc can actually reach zero.
                view.doc
                    .inner_mut()
                    .set_shell_provider(Arc::new(DummyShellProvider));
                drop(view);
            }

            if let Some(bridge) = self.bridge.as_ref() {
                let _ = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSED.to_string(),
                    window_doc_id: BigInt::from(doc_id as u64),
                    cancelable: false,
                });
            }
        }
    }

    /// Pump pending winit events for at most `millis` milliseconds. JS should
    /// call this in a loop (typically once per animation frame) to drive the
    /// renderer and event handling.
    #[napi]
    pub fn pump_app_events(&mut self, millis: f64) -> PumpResult {
        // Give host-driven DOM mutations from the previous JS turn a chance to
        // flow through Blitz's normal `View::poll -> Document::poll ->
        // request_redraw` path before winit waits for more events.
        self.poll_live_views();

        // Hand any windows that survived `closeWindow` over to the
        // BlitzApplication. After this they live in
        // `application.pending_windows` until the next handler hook
        // promotes them. blitz's own `can_create_surfaces` only fires
        // on initial resume, so the JS-runtime case is handled by
        // `JsAppHandler::drain_pending_windows`, which calls
        // `View::init` from `about_to_wait` / `proxy_wake_up`. By the
        // time `pump_app_events` returns, every entry pushed here has
        // either become a `View` in `application.windows` or been
        // explicitly cancelled via `close_window`.
        for (_doc_id, config) in self.pending.drain(..) {
            self.application.add_window(config);
        }

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
        if self.has_opened_window && self.outstanding_windows == 0 {
            return PumpResult {
                r#continue: false,
                exit: true,
                code: Some(0),
            };
        }

        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));
        // Build a fresh per-pump handler that wraps the inner blitz
        // application and lets JS observe close/closed events.
        let mut handler = JsAppHandler {
            inner: &mut self.application,
            bridge: self.bridge.as_ref(),
            outstanding: &mut self.outstanding_windows,
        };
        let status = self.event_loop.pump_app_events(timeout, &mut handler);
        self.flush_closing_windows();
        // Also catch synchronous mutations that happened inside native event
        // callbacks before returning to JS.
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

/// Translate `WindowOptions` into a winit `WindowAttributes`. Skipped
/// fields fall back to winit's platform default.
fn build_window_attributes(options: Option<WindowOptions>) -> Result<WindowAttributes> {
    let mut attrs = WindowAttributes::default();
    let Some(options) = options else {
        return Ok(attrs);
    };

    if let Some(title) = options.title {
        attrs = attrs.with_title(title);
    }
    match (options.width, options.height) {
        (Some(w), Some(h)) => {
            let w = parse_surface_dimension("width", w)?;
            let h = parse_surface_dimension("height", h)?;
            attrs = attrs.with_surface_size(PhysicalSize::new(w, h));
        }
        (None, None) => {}
        _ => {
            return Err(Error::from_reason(
                "width and height must be provided together".to_string(),
            ));
        }
    }
    if let Some(resizable) = options.resizable {
        attrs = attrs.with_resizable(resizable);
    }
    Ok(attrs)
}

fn parse_surface_dimension(name: &str, value: f64) -> Result<u32> {
    if !value.is_finite() {
        return Err(Error::from_reason(format!("{name} must be finite")));
    }
    if value.fract() != 0.0 {
        return Err(Error::from_reason(format!("{name} must be an integer")));
    }
    if value < 1.0 {
        return Err(Error::from_reason(format!("{name} must be >= 1")));
    }
    if value > u32::MAX as f64 {
        return Err(Error::from_reason(format!("{name} exceeds u32::MAX")));
    }
    Ok(value as u32)
}
