//! Monitor and video mode types for napi.

use winit::monitor::{MonitorHandle, VideoMode};

/// A fullscreen video mode of a monitor. Wraps winit's `VideoMode`.
#[napi]
pub struct VideoModeInfo {
    pub(crate) inner: VideoMode,
}

#[napi]
impl VideoModeInfo {
    #[napi(getter)]
    pub fn width(&self) -> u32 {
        self.inner.size().width
    }

    #[napi(getter)]
    pub fn height(&self) -> u32 {
        self.inner.size().height
    }

    #[napi(getter)]
    pub fn bit_depth(&self) -> Option<u16> {
        self.inner.bit_depth().map(std::num::NonZero::get)
    }

    #[napi(getter)]
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.inner
            .refresh_rate_millihertz()
            .map(std::num::NonZero::get)
    }
}

/// Information about a monitor. Wraps winit's `MonitorHandle`.
#[napi]
pub struct MonitorInfo {
    pub(crate) inner: MonitorHandle,
}

#[napi]
impl MonitorInfo {
    #[napi(getter)]
    pub fn id(&self) -> String {
        self.inner.id().to_string()
    }

    #[napi(getter)]
    pub fn name(&self) -> Option<String> {
        self.inner.name().map(|n| n.to_string())
    }

    #[napi(getter)]
    pub fn x(&self) -> i32 {
        self.inner.position().unwrap_or_default().x
    }

    #[napi(getter)]
    pub fn y(&self) -> i32 {
        self.inner.position().unwrap_or_default().y
    }

    #[napi(getter)]
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    #[napi(getter)]
    pub fn current_video_mode(&self) -> Option<VideoModeInfo> {
        self.inner
            .current_video_mode()
            .map(|vm| VideoModeInfo { inner: vm })
    }

    #[napi(getter)]
    pub fn video_modes(&self) -> Vec<VideoModeInfo> {
        self.inner
            .video_modes()
            .map(|inner| VideoModeInfo { inner })
            .collect()
    }
}

/// Convert a winit `MonitorHandle` to a napi `MonitorInfo`.
pub(crate) fn monitor_to_info(m: MonitorHandle) -> MonitorInfo {
    MonitorInfo { inner: m }
}
