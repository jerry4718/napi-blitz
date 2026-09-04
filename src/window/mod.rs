//! `Window`: napi-facing handle to one open OS window.
//!
//! A `Window` is identified internally by the document id of its attached
//! document (blitz's `BaseDocument::id()`). The doc id is allocated at
//! `DocHandle` creation time, so we know it before winit has minted a real
//! `WindowId` (winit only assigns one inside `can_create_surfaces`, which runs
//! during the next `pump_app_events`).
//!
//! Closing is async: `BlitzApp.close_window` marks the window closed
//! immediately and queues the `View` teardown for the next pump. The JS
//! side must call `window.close()` (or `app.closeWindow(window)`)
//! explicitly.
//!
//! Runtime configuration (size, resizable, ...) lives on `BlitzApp` rather
//! than `Window` itself, because the napi `Window` handle does not own a
//! reference back to the live winit `Arc<dyn Window>` - the application
//! does. The JS layer's `Window` class delegates these calls to the app.

pub(crate) mod handle;
pub(crate) mod monitor;
pub(crate) mod options;
pub(crate) mod util;

use self::{
    handle::WindowHandle,
    monitor::{MonitorInfo, VideoModeInfo},
};
use crate::{
    dom::{WindowDocument, shared::doc::SharedDocument},
    window::util::{parse_dimension, parse_window_buttons},
};
use napi::{
    Error, Result,
    bindgen_prelude::{BigInt, Uint8Array},
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

/// Shared inner state between the JS-side `Window` handle and the
/// Rust-side `WindowEntry`. Both hold the same `Rc<RefCell<WindowState>>`,
/// so `close()` on either side drops the `Arc<dyn Window>` for both.
pub(crate) struct WindowState {
    pub(crate) window: Option<Arc<dyn WinitWindow>>,
    pub(crate) closed: bool,
}

/// Handle to an open window. Construct via `BlitzApp.openWindow`.
///
/// Shares a `Rc<RefCell<WindowState>>` with the `WindowEntry` stored in
/// `BlitzApp`. `close_window` takes the `Arc<dyn Window>` out of the inner
/// cell, which releases the OS window even if this JS handle is still alive.
#[napi]
pub struct NativeWindow {
    /// winit `WindowId`; uniquely identifies the window for as long as
    /// it is open. Internal-only - the JS layer does not need to see this.
    pub(crate) window_id: WindowId,
    pub(crate) state: Rc<RefCell<WindowState>>,
}

impl NativeWindow {
    #[inline]
    fn native_window(&self) -> Result<Ref<'_, dyn WinitWindow>> {
        let state = self.state.borrow();
        if state.closed || state.window.is_none() {
            return Err(Error::from_reason("window is closed"));
        }
        Ok(Ref::map(state, |i| i.window.as_deref().unwrap()))
    }
}

#[napi]
impl NativeWindow {
    /// Whether `closeWindow` has run for this handle.
    #[napi(getter)]
    pub fn closed(&self) -> bool {
        self.state.borrow().closed
    }

    /// Opaque window identifier. JS uses this to map app-event payloads
    /// back to the right `Window` wrapper.
    #[napi(getter)]
    pub fn window_id(&self) -> BigInt {
        BigInt::from(self.window_id.into_raw() as u64)
    }

    /// Get the raw window handle for this window.
    ///
    /// The returned `RawWindowHandle` can be passed to `WindowOptions.parentWindow()`
    /// to create child windows, or to `rfd` dialogs that need a parent window.
    #[napi]
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

    #[napi]
    pub fn set_title(&self, title: String) -> Result<()> {
        self.native_window()?.set_title(&title);
        Ok(())
    }

    #[napi]
    pub fn set_size(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("width", width)?;
        let height = parse_dimension("height", height)?;
        let _ = self
            .native_window()?
            .request_surface_size(PhysicalSize::new(width, height).into());
        Ok(())
    }

    #[napi]
    pub fn get_size(&self) -> Result<Vec<u32>> {
        let size = self.native_window()?.surface_size();
        Ok(vec![size.width, size.height])
    }

    #[napi]
    pub fn get_resizable(&self) -> Result<bool> {
        Ok(self.native_window()?.is_resizable())
    }

    #[napi]
    pub fn current_monitor(&self) -> Option<MonitorInfo> {
        self.native_window()
            .ok()?
            .current_monitor()
            .map(|inner| MonitorInfo { inner })
    }

    #[napi]
    pub fn set_min_size(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("minWidth", width)?;
        let height = parse_dimension("minHeight", height)?;
        self.native_window()?
            .set_min_surface_size(Some(PhysicalSize::new(width, height).into()));
        Ok(())
    }

    #[napi]
    pub fn set_max_size(&self, width: f64, height: f64) -> Result<()> {
        let width = parse_dimension("maxWidth", width)?;
        let height = parse_dimension("maxHeight", height)?;
        self.native_window()?
            .set_max_surface_size(Some(PhysicalSize::new(width, height).into()));
        Ok(())
    }

    #[napi]
    pub fn set_resizable(&self, value: bool) -> Result<()> {
        self.native_window()?.set_resizable(value);
        Ok(())
    }

    #[napi]
    pub fn set_maximized(&self, value: bool) -> Result<()> {
        self.native_window()?.set_maximized(value);
        Ok(())
    }

    #[napi]
    pub fn set_visible(&self, value: bool) -> Result<()> {
        self.native_window()?.set_visible(value);
        Ok(())
    }

    #[napi]
    pub fn set_transparent(&self, value: bool) -> Result<()> {
        self.native_window()?.set_transparent(value);
        Ok(())
    }

    #[napi]
    pub fn set_blur(&self, value: bool) -> Result<()> {
        self.native_window()?.set_blur(value);
        Ok(())
    }

    #[napi]
    pub fn set_decorations(&self, value: bool) -> Result<()> {
        self.native_window()?.set_decorations(value);
        Ok(())
    }

    #[napi]
    pub fn set_fullscreen_borderless(&self, monitor: &MonitorInfo) -> Result<()> {
        self.native_window()?
            .set_fullscreen(Some(Fullscreen::Borderless(Some(monitor.inner.clone()))));
        Ok(())
    }

    #[napi]
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

    #[napi]
    pub fn set_fullscreen_none(&self) -> Result<()> {
        self.native_window()?.set_fullscreen(None);
        Ok(())
    }

    #[napi]
    pub fn set_enabled_buttons(&self, buttons: Vec<String>) -> Result<()> {
        let flags = parse_window_buttons(&buttons)?;
        self.native_window()?.set_enabled_buttons(flags);
        Ok(())
    }

    #[napi]
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
}

/// Internal helper: build a WindowDocument from a document's shared state.
#[cfg(feature = "native-window")]
pub(crate) fn make_window_document(shared_doc: &Rc<SharedDocument>) -> Box<WindowDocument> {
    Box::new(WindowDocument::new(Rc::clone(shared_doc)))
}
