//! `BlitzApp`: the JS-facing wrapper around a winit event loop.
//!
//! `BlitzApp.create()` builds an event loop. Calling `openWindow(docHandle)`
//! produces a `Box<dyn Document>` from the handle and attaches a fresh window
//! to it. JS drives the loop synchronously via `pumpAppEvents(millis)` from
//! the main thread; this keeps event callbacks re-entrant on the napi env so
//! we can call back into JS without a ThreadsafeFunction.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use blitz::shell::{
    BlitzShellEvent, BlitzShellProxy, EventLoop, View, WindowConfig, create_default_event_loop,
};
use blitz::traits::shell::DummyShellProvider;
use napi::{
    Env, Error, Result,
    bindgen_prelude::{BigInt, Function, FunctionRef, Uint8Array},
};
use napi_derive::napi;
use std::sync::Arc;
use winit::{
    dpi::PhysicalSize,
    event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    window::{WindowAttributes, WindowButtons},
};

use crate::{
    dom::doc::{DocHandle, make_window_document},
    native_window::{
        app_bridge::{APP_EVENT_CLOSED, AppDispatchResult, AppEventPayload, JsAppBridge},
        app_handler::JsAppHandler,
        monitor::MonitorInfo,
        window::{Window, WindowOptions},
    },
    renderer::CurrentRenderer,
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
pub struct BlitzApp {
    event_loop: Option<EventLoop>,
    /// Live windows keyed by winit `WindowId`.
    pub(crate) windows: HashMap<winit::window::WindowId, View<CurrentRenderer>>,
    /// Window configs requested via `openWindow` but not yet promoted to live `View`s.
    pub(crate) pending: Vec<(usize, WindowConfig<CurrentRenderer>)>,
    /// Proxy for sending events into the event loop (redraw, poll, etc.).
    pub(crate) proxy: BlitzShellProxy,
    /// Receiver for `BlitzShellEvent`s from the proxy channel.
    pub(crate) event_queue: Receiver<BlitzShellEvent>,
    /// Doc ids requested to close from JS. We intentionally defer live
    /// `View` removal until after the current `pumpAppEvents` call has
    /// returned from winit event dispatch. This makes `window.close()`
    /// safe to call from within that same window's click handler.
    pub(crate) closing_doc_ids: Vec<usize>,
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
impl BlitzApp {
    /// Build the winit event loop.
    #[napi(factory)]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        Self {
            event_loop: Some(event_loop),
            windows: HashMap::new(),
            pending: Vec::new(),
            proxy,
            event_queue: receiver,
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
        options: Option<&WindowOptions>,
    ) -> Result<Window> {
        if !doc.mark_attached() {
            return Err(Error::from_reason(
                "DocHandle has already been attached to a window".to_string(),
            ));
        }
        let doc_id = doc.doc_id();
        let window_doc = make_window_document(doc);
        let attributes = build_window_attributes(options)?;
        let config = WindowConfig::with_attributes(window_doc, CurrentRenderer::new(), attributes);
        self.pending.push((doc_id, config));
        self.has_opened_window = true;
        self.outstanding_windows += 1;

        // winit only assigns a WindowId while dispatching through an active
        // event loop. Run one non-blocking pump so the window is created, then
        // grab the Arc<dyn Window> straight from the view.
        self.pump_app_events(0.0);
        let native_window = self
            .windows
            .iter()
            .find_map(|(_, view)| (view.doc.id() == doc_id).then_some(view.window.clone()))
            .ok_or_else(|| Error::from_reason("failed to create native window"))?;

        Ok(Window {
            doc_id,
            window: Some(native_window),
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
        window.window = None;
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

    /// List all available monitors with full metadata. Returns `[]` if
    /// no windows have been created yet.
    #[napi]
    pub fn available_monitors(&self) -> Vec<MonitorInfo> {
        let Some(view) = self.windows.values().next() else {
            return Vec::new();
        };
        view.window
            .available_monitors()
            .map(monitor_to_info)
            .collect()
    }

    /// The primary monitor. Returns `None` if no windows have been
    /// created yet.
    #[napi]
    pub fn primary_monitor(&self) -> Option<MonitorInfo> {
        let view = self.windows.values().next()?;
        view.window.primary_monitor().map(monitor_to_info)
    }

    /// Pump pending winit events for at most `millis` milliseconds.
    #[napi]
    pub fn pump_app_events(&mut self, millis: f64) -> PumpResult {
        self.pump_app_events_inner(millis)
    }
}

impl BlitzApp {
    fn has_initialised_window(&self, doc_id: usize) -> bool {
        self.windows.values().any(|view| view.doc.id() == doc_id)
    }

    fn poll_live_views(&mut self) {
        for view in self.windows.values_mut() {
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
                .windows
                .iter()
                .find_map(|(window_id, view)| (view.doc.id() == doc_id).then_some(*window_id))
            else {
                continue;
            };

            if let Some(mut view) = self.windows.remove(&window_id) {
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
        if self.has_opened_window && self.outstanding_windows == 0 {
            return PumpResult {
                r#continue: false,
                exit: true,
                code: Some(0),
            };
        }

        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));

        // Take event_loop out so the handler can borrow the rest of `self`.
        let mut event_loop = self.event_loop.take().expect("event_loop taken");
        let mut handler = JsAppHandler { app: self };
        let status = event_loop.pump_app_events(timeout, &mut handler);
        self.event_loop = Some(event_loop);
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
fn build_window_attributes(options: Option<&WindowOptions>) -> Result<WindowAttributes> {
    let mut attrs = WindowAttributes::default();
    let Some(options) = options else {
        return Ok(attrs);
    };

    if let Some(title) = options.title.as_ref() {
        attrs = attrs.with_title(title.clone());
    }
    if let Some((w, h)) = options.size {
        let w = parse_surface_dimension("width", w)?;
        let h = parse_surface_dimension("height", h)?;
        attrs = attrs.with_surface_size(PhysicalSize::new(w, h));
    }
    if let Some(resizable) = options.resizable {
        attrs = attrs.with_resizable(resizable);
    }
    if let Some((w, h)) = options.min_size {
        let w = parse_surface_dimension("minWidth", w)?;
        let h = parse_surface_dimension("minHeight", h)?;
        attrs = attrs.with_min_surface_size(PhysicalSize::new(w, h));
    }
    if let Some((w, h)) = options.max_size {
        let w = parse_surface_dimension("maxWidth", w)?;
        let h = parse_surface_dimension("maxHeight", h)?;
        attrs = attrs.with_max_surface_size(PhysicalSize::new(w, h));
    }
    if let Some(maximized) = options.maximized {
        attrs = attrs.with_maximized(maximized);
    }
    if let Some(visible) = options.visible {
        attrs = attrs.with_visible(visible);
    }
    if let Some(transparent) = options.transparent {
        attrs = attrs.with_transparent(transparent);
    }
    if let Some(blur) = options.blur {
        attrs = attrs.with_blur(blur);
    }
    if let Some(decorations) = options.decorations {
        attrs = attrs.with_decorations(decorations);
    }
    if let Some(fullscreen) = options.fullscreen.as_ref() {
        attrs = attrs.with_fullscreen(Some(fullscreen.clone()));
    }
    if let Some(buttons) = options.enabled_buttons.as_ref() {
        attrs = attrs.with_enabled_buttons(parse_window_buttons(buttons)?);
    }
    if let Some(icon_data) = options.window_icon.as_ref() {
        attrs = attrs.with_window_icon(Some(parse_window_icon(icon_data)?));
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

/// Convert a winit `MonitorHandle` to a napi `MonitorInfo`.
fn monitor_to_info(m: winit::monitor::MonitorHandle) -> MonitorInfo {
    MonitorInfo { inner: m }
}

/// Parse JS string array into winit `WindowButtons` bitflags.
/// Accepted values: `"close"`, `"minimize"`, `"maximize"`.
fn parse_window_buttons(buttons: &[String]) -> Result<WindowButtons> {
    let mut flags = WindowButtons::empty();
    for btn in buttons {
        match btn.as_str() {
            "close" => flags |= WindowButtons::CLOSE,
            "minimize" => flags |= WindowButtons::MINIMIZE,
            "maximize" => flags |= WindowButtons::MAXIMIZE,
            other => {
                return Err(Error::from_reason(format!(
                    "enabledButtons: unknown button \"{other}\", expected close/minimize/maximize"
                )));
            }
        }
    }
    Ok(flags)
}

/// Parse window icon from raw bytes. Expected layout:
/// `[width_u32_le, height_u32_le, ...rgba8_pixels]` (8 byte header + w*h*4 bytes).
fn parse_window_icon(data: &Uint8Array) -> Result<winit::icon::Icon> {
    let bytes = data.as_ref();
    if bytes.len() < 8 {
        return Err(Error::from_reason(
            "windowIcon: data too short, expected 8-byte header (width, height) + RGBA pixels",
        ));
    }
    let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let pixels = &bytes[8..];
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| Error::from_reason("windowIcon: width*height*4 overflows usize"))?;
    if pixels.len() < expected {
        return Err(Error::from_reason(format!(
            "windowIcon: pixel data is {} bytes, expected {expected} ({}x{}x4)",
            pixels.len(),
            width,
            height
        )));
    }
    winit::icon::RgbaIcon::new(pixels[..expected].to_vec(), width, height)
        .map(winit::icon::Icon::from)
        .map_err(|e| Error::from_reason(format!("windowIcon: failed to create icon: {e}")))
}
