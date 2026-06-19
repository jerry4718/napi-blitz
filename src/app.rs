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
use napi::{Error, Result};
use napi_derive::napi;
use winit::dpi::PhysicalSize;
use winit::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::WindowAttributes;

use crate::doc::{DocHandle, make_window_document};
use crate::window::{Window, WindowOptions};

/// Result of one `pumpAppEvents` call.
#[napi(object)]
pub struct PumpResult {
    /// The loop is still running. Caller should pump again later.
    pub running: bool,
    /// The loop has exited (e.g. all windows closed).
    pub exited: bool,
    /// Exit code, if `exited`.
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
        }
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
        let attributes = build_window_attributes(options);
        let config =
            WindowConfig::with_attributes(window_doc, VelloWindowRenderer::new(), attributes);
        self.pending.push((doc_id, config));
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
        if window.closed {
            return;
        }
        let doc_id = window.doc_id;

        // Drop matching pending config (window opened but never pumped).
        self.pending.retain(|(id, _)| *id != doc_id);

        // Remove from initialised windows.
        self.application
            .windows
            .retain(|_, view| view.doc.id() != doc_id);

        window.closed = true;
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
    #[napi]
    pub fn set_window_inner_size(&mut self, window: &Window, width: u32, height: u32) {
        if let Some(view) = self.window_view(window) {
            let _ = view
                .window
                .request_surface_size(PhysicalSize::new(width, height).into());
        }
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

    /// Pump pending winit events for at most `millis` milliseconds. JS should
    /// call this in a loop (typically once per animation frame) to drive the
    /// renderer and event handling.
    #[napi]
    pub fn pump_app_events(&mut self, millis: f64) -> PumpResult {
        // Hand any windows that survived `closeWindow` over to the
        // BlitzApplication. After this they live in `application.windows`
        // (after the next `can_create_surfaces`) and synchronous close is
        // routed through `application.windows.retain(...)`.
        for (_doc_id, config) in self.pending.drain(..) {
            self.application.add_window(config);
        }

        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));
        match self
            .event_loop
            .pump_app_events(timeout, &mut self.application)
        {
            PumpStatus::Continue => PumpResult {
                running: true,
                exited: false,
                code: None,
            },
            PumpStatus::Exit(code) => PumpResult {
                running: false,
                exited: true,
                code: Some(code),
            },
        }
    }
}

/// Translate `WindowOptions` into a winit `WindowAttributes`. Skipped
/// fields fall back to winit's platform default.
fn build_window_attributes(options: Option<WindowOptions>) -> WindowAttributes {
    let mut attrs = WindowAttributes::default();
    let Some(options) = options else { return attrs };

    if let Some(title) = options.title {
        attrs = attrs.with_title(title);
    }
    if let (Some(w), Some(h)) = (options.width, options.height) {
        attrs = attrs.with_surface_size(PhysicalSize::new(w, h));
    }
    if let Some(resizable) = options.resizable {
        attrs = attrs.with_resizable(resizable);
    }
    attrs
}
