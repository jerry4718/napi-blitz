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
    dom::{WindowDocument, layers::html_document::HTMLDocumentLayer, shared::doc::SharedDocument},
    events::base::EventTargetLayer,
    window::util::{parse_dimension, parse_window_buttons},
};
use napi::{
    Error, Result,
    bindgen_prelude::{BigInt, PromiseRaw, Uint8Array, Undefined},
};
use napi_helpers::inherits::{Constructed, LayerRef, Super, proc::layer};
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
    #[allow(dead_code)]
    pub(crate) shared_doc: Rc<SharedDocument>,
    pub(crate) lifecycle: Rc<Lifecycle>,
    pub(crate) document: LayerRef<HTMLDocumentLayer>,
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

    /// The HTMLDocument painted in this window.
    #[layer(getter)]
    fn document(&self) -> LayerRef<HTMLDocumentLayer> {
        self.document.clone()
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
        let state = self.lifecycle.state().borrow();
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
        let state = self.lifecycle.state().borrow();
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
