//! `Window`: layer handle to one open OS window.
//!
//! The layer holds the winit `WindowId` and the shared
//! `Rc<RefCell<WindowState>>` directly (the same `Arc<dyn Window>` the
//! lifecycle's `WindowEntry` keeps), so `close()` drops the OS window for
//! both and runtime configuration calls reach the winit window without an
//! app-side lookup.
//!
//! Closing is async: `close()` marks the window closed immediately and
//! queues the `View` teardown for the next pump. The JS side must call
//! `window.close()` (or `app.closeWindow(window)`) explicitly.

pub(crate) mod handle;
pub(crate) mod monitor;
pub(crate) mod options;
pub(crate) mod util;

use self::{
    handle::WindowHandle,
    monitor::{MonitorInfo, VideoModeInfo},
};
use crate::{
    app::lifecycle::Lifecycle,
    dom::{
        WindowDocument,
        layers::html_document::HTMLDocumentLayer,
        shared::doc::{SharedDocument, build_shared_document},
        shared::wrap_node,
    },
    events::base::EventTargetLayer,
    window::util::{parse_dimension, parse_window_buttons},
};
use blitz::dom::DEFAULT_CSS;
use napi::{
    Env, Error, Result,
    bindgen_prelude::{BigInt, Object, PromiseRaw, Uint8Array, Undefined},
};
use napi_helpers::{
    discard_err,
    inherits::{Constructed, LayerRef, Super, proc::layer},
};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
    sync::Arc,
};
use winit::{
    dpi::PhysicalSize,
    icon::{Icon, RgbaIcon},
    monitor::Fullscreen,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window as WinitWindow, WindowId},
};

/// Shared inner state between the `Window` layer and the
/// Rust-side `WindowEntry`. Both hold the same `Rc<RefCell<WindowState>>`,
/// so `close()` on either side drops the `Arc<dyn Window>` for both.
pub(crate) struct WindowState {
    pub(crate) window: Option<Arc<dyn WinitWindow>>,
    pub(crate) closed: bool,
}

/// Own block of the `Window` class. Constructed by the open-flow
/// (`Lifecycle::drain_opening_windows`), never from JS directly.
#[layer]
pub struct WindowLayer {
    pub(crate) window_id: WindowId,
    pub(crate) state: Rc<RefCell<WindowState>>,
    pub(crate) shared_doc: Rc<SharedDocument>,
    pub(crate) lifecycle: Rc<Lifecycle>,
}

impl WindowLayer {
    #[inline]
    fn native_window(&self) -> Result<Ref<'_, dyn WinitWindow>> {
        let state = self.state.borrow();
        if state.closed || state.window.is_none() {
            return Err(Error::from_reason("window is closed"));
        }
        Ok(Ref::map(state, |i| i.window.as_deref().unwrap()))
    }
}

#[layer(js_name = "Window")]
impl WindowLayer {
    #[layer(parent)]
    type Parent = EventTargetLayer;

    #[layer(constructor)]
    fn build(_sup: Super<EventTargetLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "construct a window via BlitzApp.openWindow",
        ))
    }

    /// The HTMLDocument painted in this window. Resolved through the
    /// shared document's two-state reference: strong while the window is
    /// live, weak after teardown, so no strong edge is parked on the
    /// window wrapper itself.
    #[layer(getter)]
    fn document(&self, env: &Env) -> Result<LayerRef<HTMLDocumentLayer>> {
        let obj = self
            .shared_doc
            .document_ref()
            .as_ref()
            .and_then(|r| r.get_value(env))
            .ok_or_else(|| Error::from_reason("document is gone"))?;
        LayerRef::new(&obj, env)
    }

    /// Whether `close()` has run for this window.
    #[layer(getter)]
    pub fn closed(&self) -> bool {
        self.state.borrow().closed
    }

    /// Opaque window identifier. The lifecycle routes winit events back to
    /// the right window by it.
    #[layer(getter)]
    pub fn window_id(&self) -> BigInt {
        BigInt::from(self.window_id.into_raw() as u64)
    }

    /// Get the raw window handle for this window.
    ///
    /// The returned `RawWindowHandle` can be passed to `WindowOptions.parentWindow()`
    /// to create child windows, or to `rfd` dialogs that need a parent window.
    #[layer]
    pub fn window_handle(&self) -> Result<WindowHandle> {
        let window = self.native_window()?;
        let wh = window
            .window_handle()
            .map_err(|e| Error::from_reason(format!("failed to get raw window handle: {e}")))?;
        let dh = window
            .display_handle()
            .map_err(|e| Error::from_reason(format!("failed to get raw display handle: {e}")))?;
        Ok(WindowHandle {
            window: wh.as_raw(),
            display: dh.as_raw(),
        })
    }

    #[layer]
    pub fn set_title(&self, title: String) -> Result<()> {
        self.native_window()?.set_title(&title);
        Ok(())
    }

    /// Replace the window's document the way assigning `location.href`
    /// would: a fresh document object is built and swapped in, and the
    /// old document — wrappers, caches and all — is retired. The swap is
    /// the cycle-breaking switch: the old document drops every native
    /// strong edge (`detach_window`) and the new one gains them
    /// (`attach_window`) in the same step. Returns the fresh document,
    /// like `DOMParser.parseFromString` does for its parsed document.
    /// This is a blitz-specific navigation API, not a DOM-standard one,
    /// so it lives on the window rather than on `Document`.
    #[layer]
    pub fn load_html(
        &mut self,
        this: &Object,
        env: &Env,
        html: String,
    ) -> Result<LayerRef<HTMLDocumentLayer>> {
        if self.state.borrow().closed {
            return Err(Error::from_reason("window is closed"));
        }
        let new_shared = build_shared_document(env, &html, vec![DEFAULT_CSS.to_string()])?;

        // Rebind the window entry to the new document, carrying over the
        // live viewport. Pure Rust, so the state borrow is not held
        // across any JS work.
        {
            let mut state = self.lifecycle.state_mut();
            let entry = state
                .windows
                .get_mut(&self.window_id)
                .ok_or_else(|| Error::from_reason("window is not open"))?;
            let viewport = entry.shared_doc.base().viewport().clone();
            new_shared.base_mut().set_viewport(viewport);
            entry.shared_doc = Rc::clone(&new_shared);
            entry.view.borrow_mut().doc = make_window_document(&new_shared);
        }

        // Retire the old document: its JS Document reference and whole
        // cached tree go weak at once.
        discard_err!(self.shared_doc.detach_window(env), "detach old document");

        // Point the window layer at the new document and pin it for the
        // window's lifetime.
        let node_id = new_shared.base().root_node().id;
        let document = wrap_node(&new_shared, env, node_id)?;
        let fresh_document = LayerRef::new(&document, env)?;
        self.shared_doc = Rc::clone(&new_shared);
        new_shared.set_window_ref(env, this)?;
        new_shared.attach_window(env)?;
        new_shared.mark_host_dirty();
        Ok(fresh_document)
    }

    #[layer]
    pub fn set_size(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("width", width)?;
        let height = parse_dimension("height", height)?;
        let _ = self
            .native_window()?
            .request_surface_size(PhysicalSize::new(width, height).into());
        Ok(())
    }

    #[layer]
    pub fn resize(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("width", width)?;
        let height = parse_dimension("height", height)?;
        let _ = self
            .native_window()?
            .request_surface_size(PhysicalSize::new(width, height).into());
        Ok(())
    }

    #[layer]
    pub fn get_size(&self) -> Result<Vec<u32>> {
        let size = self.native_window()?.surface_size();
        Ok(vec![size.width, size.height])
    }

    #[layer]
    pub fn get_resizable(&self) -> Result<bool> {
        Ok(self.native_window()?.is_resizable())
    }

    #[layer]
    pub fn current_monitor(&self) -> Option<MonitorInfo> {
        self.native_window()
            .ok()?
            .current_monitor()
            .map(|inner| MonitorInfo { inner })
    }

    #[layer]
    pub fn set_min_size(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("minWidth", width)?;
        let height = parse_dimension("minHeight", height)?;
        self.native_window()?
            .set_min_surface_size(Some(PhysicalSize::new(width, height).into()));
        Ok(())
    }

    #[layer]
    pub fn set_max_size(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("maxWidth", width)?;
        let height = parse_dimension("maxHeight", height)?;
        self.native_window()?
            .set_max_surface_size(Some(PhysicalSize::new(width, height).into()));
        Ok(())
    }

    #[layer]
    pub fn set_resizable(&self, value: bool) -> Result<()> {
        self.native_window()?.set_resizable(value);
        Ok(())
    }

    #[layer]
    pub fn set_maximized(&self, value: bool) -> Result<()> {
        self.native_window()?.set_maximized(value);
        Ok(())
    }

    #[layer]
    pub fn set_visible(&self, value: bool) -> Result<()> {
        self.native_window()?.set_visible(value);
        Ok(())
    }

    #[layer]
    pub fn set_transparent(&self, value: bool) -> Result<()> {
        self.native_window()?.set_transparent(value);
        Ok(())
    }

    #[layer]
    pub fn set_blur(&self, value: bool) -> Result<()> {
        self.native_window()?.set_blur(value);
        Ok(())
    }

    #[layer]
    pub fn set_decorations(&self, value: bool) -> Result<()> {
        self.native_window()?.set_decorations(value);
        Ok(())
    }

    #[layer]
    pub fn set_fullscreen_borderless(&self, monitor: &MonitorInfo) -> Result<()> {
        self.native_window()?
            .set_fullscreen(Some(Fullscreen::Borderless(Some(monitor.inner.clone()))));
        Ok(())
    }

    #[layer]
    pub fn set_fullscreen_exclusive(
        &self,
        monitor: &MonitorInfo,
        video_mode: &VideoModeInfo,
    ) -> Result<()> {
        self.native_window()?
            .set_fullscreen(Some(Fullscreen::Exclusive(
                monitor.inner.clone(),
                video_mode.inner,
            )));
        Ok(())
    }

    #[layer]
    pub fn set_fullscreen_none(&self) -> Result<()> {
        self.native_window()?.set_fullscreen(None);
        Ok(())
    }

    #[layer]
    pub fn set_enabled_buttons(&self, buttons: Vec<String>) -> Result<()> {
        let flags = parse_window_buttons(&buttons)?;
        self.native_window()?.set_enabled_buttons(flags);
        Ok(())
    }

    #[layer]
    pub fn set_window_icon(&self, data: Uint8Array) -> Result<()> {
        let bytes = data.as_ref();
        if bytes.len() < 8 {
            return Err(Error::from_reason(
                "windowIcon: data too short, expected 8-byte header + RGBA pixels",
            ));
        }
        let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| Error::from_reason("windowIcon: width*height*4 overflows usize"))?;
        let pixels = &bytes[8..];
        if pixels.len() != expected {
            return Err(Error::from_reason(format!(
                "windowIcon: pixel data is {} bytes, expected {expected}",
                pixels.len()
            )));
        }
        let icon = RgbaIcon::new(pixels.to_vec(), width, height)
            .map(Icon::from)
            .map_err(|e| Error::from_reason(format!("windowIcon: {e}")))?;
        self.native_window()?.set_window_icon(Some(icon));
        Ok(())
    }

    /// Set the document zoom level. `1.0` is unzoomed. Combined with the
    /// system scale factor to produce the total viewport scale
    /// (`hidpi_scale * zoom`) that scales layout and CSS transforms.
    #[layer]
    pub fn set_zoom(&self, zoom: f64) -> Result<()> {
        let state = self.lifecycle.state();
        let entry = state
            .windows
            .get(&self.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        entry
            .view
            .borrow_mut()
            .with_viewport(|v| v.set_zoom(zoom as f32));
        Ok(())
    }

    /// Get the current document zoom level.
    #[layer]
    pub fn get_zoom(&self) -> Result<f32> {
        let state = self.lifecycle.state();
        let entry = state
            .windows
            .get(&self.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        Ok(entry.view.borrow().doc.inner().viewport().zoom())
    }

    /// Queue this window for closure and return a promise that resolves
    /// once the native `View` has actually been torn down (during the next
    /// pump), or rejects if a `close` listener calls `preventDefault()`.
    /// `close()` is idempotent.
    #[layer]
    pub fn close(&self) -> Result<PromiseRaw<'static, Undefined>> {
        self.lifecycle.request_close(self)
    }
}

/// Internal helper: build a WindowDocument from a document's shared state.
#[cfg(feature = "native-window")]
pub(crate) fn make_window_document(shared_doc: &Rc<SharedDocument>) -> Box<WindowDocument> {
    Box::new(WindowDocument::new(Rc::clone(shared_doc)))
}
