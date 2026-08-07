//! `Window`: napi-facing handle to one open OS window.
//!
//! A `Window` is identified internally by the document id of its attached
//! document (blitz's `BaseDocument::id()`). The doc id is allocated at
//! `DocHandle` creation time, so we know it before winit has minted a real
//! `WindowId` (winit only assigns one inside `can_create_surfaces`, which runs
//! during the next `pump_app_events`).
//!
//! Closing is synchronous: `BlitzApp.close_window` mutates the application's
//! `windows` map directly. We do NOT rely on JS GC to drop windows - the JS
//! side must call `window.close()` (or `app.closeWindow(window)`) explicitly.
//!
//! Runtime configuration (size, resizable, ...) lives on `BlitzApp` rather
//! than `Window` itself, because the napi `Window` handle does not own a
//! reference back to the live winit `Arc<dyn Window>` - the application
//! does. The JS layer's `Window` class delegates these calls to the app.

pub mod monitor;

use monitor::{MonitorInfo, VideoModeInfo};
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
    window::{Window as WinitWindow, WindowAttributes, WindowButtons, WindowId},
};

/// Options accepted by `BlitzApp.openWindow`. Construct via
/// `WindowOptions.builder()`.
#[napi]
pub struct WindowOptions {
    pub(crate) title: Option<String>,
    pub(crate) size: Option<(f64, f64)>,
    pub(crate) resizable: Option<bool>,
    pub(crate) min_size: Option<(f64, f64)>,
    pub(crate) max_size: Option<(f64, f64)>,
    pub(crate) maximized: Option<bool>,
    pub(crate) visible: Option<bool>,
    pub(crate) transparent: Option<bool>,
    pub(crate) blur: Option<bool>,
    pub(crate) decorations: Option<bool>,
    pub(crate) fullscreen: Option<Fullscreen>,
    pub(crate) enabled_buttons: Option<Vec<String>>,
    pub(crate) window_icon: Option<Uint8Array>,
}

#[napi]
impl WindowOptions {
    /// Create a new builder with all fields unset.
    #[napi(factory)]
    pub fn builder() -> Self {
        Self {
            title: None,
            size: None,
            resizable: None,
            min_size: None,
            max_size: None,
            maximized: None,
            visible: None,
            transparent: None,
            blur: None,
            decorations: None,
            fullscreen: None,
            enabled_buttons: None,
            window_icon: None,
        }
    }

    #[napi]
    pub fn title(&mut self, value: String) -> &Self {
        self.title = Some(value);
        self
    }

    #[napi]
    pub fn size(&mut self, width: f64, height: f64) -> &Self {
        self.size = Some((width, height));
        self
    }

    #[napi]
    pub fn resizable(&mut self, value: bool) -> &Self {
        self.resizable = Some(value);
        self
    }

    #[napi]
    pub fn min_size(&mut self, width: f64, height: f64) -> &Self {
        self.min_size = Some((width, height));
        self
    }

    #[napi]
    pub fn max_size(&mut self, width: f64, height: f64) -> &Self {
        self.max_size = Some((width, height));
        self
    }

    #[napi]
    pub fn maximized(&mut self, value: bool) -> &Self {
        self.maximized = Some(value);
        self
    }

    #[napi]
    pub fn visible(&mut self, value: bool) -> &Self {
        self.visible = Some(value);
        self
    }

    #[napi]
    pub fn transparent(&mut self, value: bool) -> &Self {
        self.transparent = Some(value);
        self
    }

    #[napi]
    pub fn blur(&mut self, value: bool) -> &Self {
        self.blur = Some(value);
        self
    }

    #[napi]
    pub fn decorations(&mut self, value: bool) -> &Self {
        self.decorations = Some(value);
        self
    }

    /// Set borderless fullscreen on the specified monitor.
    #[napi]
    pub fn fullscreen_borderless(&mut self, monitor: &MonitorInfo) -> &Self {
        self.fullscreen = Some(Fullscreen::Borderless(Some(monitor.inner.clone())));
        self
    }

    /// Set exclusive fullscreen using the specified monitor and video mode.
    #[napi]
    pub fn fullscreen_exclusive(
        &mut self,
        monitor: &MonitorInfo,
        video_mode: &VideoModeInfo,
    ) -> &Self {
        self.fullscreen = Some(Fullscreen::Exclusive(
            monitor.inner.clone(),
            video_mode.inner,
        ));
        self
    }

    #[napi]
    pub fn enabled_buttons(&mut self, value: Vec<String>) -> &Self {
        self.enabled_buttons = Some(value);
        self
    }

    #[napi]
    pub fn window_icon(&mut self, value: Uint8Array) -> &Self {
        self.window_icon = Some(value);
        self
    }
}

/// Shared inner state between the JS-side `Window` handle and the
/// Rust-side `WindowEntry`. Both hold the same `Rc<RefCell<WindowInner>>`,
/// so `close()` on either side drops the `Arc<dyn Window>` for both.
pub(crate) struct WindowInner {
    pub(crate) window: Option<Arc<dyn WinitWindow>>,
    pub(crate) closed: bool,
}

/// Handle to an open window. Construct via `BlitzApp.openWindow`.
///
/// Shares a `Rc<RefCell<WindowInner>>` with the `WindowEntry` stored in
/// `BlitzApp`. `close_window` takes the `Arc<dyn Window>` out of the inner
/// cell, which releases the OS window even if this JS handle is still alive.
#[napi]
pub struct Window {
    /// winit `WindowId`; uniquely identifies the window for as long as
    /// it is open. Internal-only - the JS layer does not need to see this.
    pub(crate) window_id: WindowId,
    pub(crate) inner: Rc<RefCell<WindowInner>>,
}

impl Window {
    #[inline]
    fn native_window(&self) -> Result<Ref<'_, dyn WinitWindow>> {
        let inner = self.inner.borrow();
        if inner.closed || inner.window.is_none() {
            return Err(Error::from_reason("window is closed"));
        }
        Ok(Ref::map(inner, |i| i.window.as_deref().unwrap()))
    }
}

#[napi]
impl Window {
    /// Whether `closeWindow` has run for this handle.
    #[napi(getter)]
    pub fn closed(&self) -> bool {
        self.inner.borrow().closed
    }

    /// Opaque window identifier. JS uses this to map app-event payloads
    /// back to the right `Window` wrapper.
    #[napi(getter)]
    pub fn window_id(&self) -> BigInt {
        BigInt::from(self.window_id.into_raw() as u64)
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

pub(crate) fn parse_dimension(name: &str, value: f64) -> Result<u32> {
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

/// Translate `WindowOptions` into a winit `WindowAttributes`. Skipped
/// fields fall back to winit's platform default.
pub(crate) fn build_window_attributes(options: Option<&WindowOptions>) -> Result<WindowAttributes> {
    let mut attrs = WindowAttributes::default();
    let Some(options) = options else {
        return Ok(attrs);
    };

    if let Some(title) = options.title.as_ref() {
        attrs = attrs.with_title(title.clone());
    }
    if let Some((w, h)) = options.size {
        let w = parse_dimension("width", w)?;
        let h = parse_dimension("height", h)?;
        attrs = attrs.with_surface_size(PhysicalSize::new(w, h));
    }
    if let Some(resizable) = options.resizable {
        attrs = attrs.with_resizable(resizable);
    }
    if let Some((w, h)) = options.min_size {
        let w = parse_dimension("minWidth", w)?;
        let h = parse_dimension("minHeight", h)?;
        attrs = attrs.with_min_surface_size(PhysicalSize::new(w, h));
    }
    if let Some((w, h)) = options.max_size {
        let w = parse_dimension("maxWidth", w)?;
        let h = parse_dimension("maxHeight", h)?;
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

/// Parse JS string array into winit `WindowButtons` bitflags.
/// Accepted values: `"close"`, `"minimize"`, `"maximize"`.
pub(crate) fn parse_window_buttons(buttons: &[String]) -> Result<WindowButtons> {
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
pub(crate) fn parse_window_icon(data: &Uint8Array) -> Result<Icon> {
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
    RgbaIcon::new(pixels[..expected].to_vec(), width, height)
        .map(Icon::from)
        .map_err(|e| Error::from_reason(format!("windowIcon: failed to create icon: {e}")))
}
