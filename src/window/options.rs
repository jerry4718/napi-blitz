use crate::window::{
    handle::WindowHandle,
    monitor::{MonitorInfo, VideoModeInfo},
};
use napi::bindgen_prelude::Uint8Array;
use winit::monitor::Fullscreen;

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
    pub(crate) parent_window: Option<WindowHandle>,
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
            parent_window: None,
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

    /// Set the parent window for this window.
    ///
    /// Pass a `RawWindowHandle` obtained from `NativeWindow.windowHandle()`.
    #[napi]
    pub fn parent_window(&mut self, handle: &WindowHandle) -> &Self {
        self.parent_window = Some(WindowHandle {
            window: handle.window,
            display: handle.display,
        });
        self
    }
}
