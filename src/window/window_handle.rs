//! Napi wrapper around `rwh_06::RawWindowHandle`.
//!
//! JS cannot construct this directly - it can only obtain a reference from
//! Rust-side methods (e.g. `NativeWindow.windowHandle()`). This keeps the
//! unsafe pointer construction on the Rust side.

use winit::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle as OriginWindowHandle,
};

/// Opaque wrapper around a platform-specific raw window handle.
///
/// Obtained from native objects (e.g. `NativeWindow.windowHandle()`).
/// Pass it to APIs that need a parent window, such as `WindowOptions.parentWindow()`,
/// or to `rfd` dialog calls.
#[napi]
pub struct WindowHandle {
    pub(crate) window: RawWindowHandle,
    pub(crate) display: RawDisplayHandle,
}

impl HasWindowHandle for WindowHandle {
    fn window_handle(&self) -> Result<OriginWindowHandle<'_>, HandleError> {
        // SAFETY: the handle was obtained from a live window.
        unsafe { Ok(OriginWindowHandle::borrow_raw(self.window)) }
    }
}

impl HasDisplayHandle for WindowHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: the handle was obtained from a live window.
        unsafe { Ok(DisplayHandle::borrow_raw(self.display)) }
    }
}

// SAFETY: WindowHandle holds only Copy raw handle values. They are plain
// integers/pointers that are safe to share across threads for reading.
unsafe impl Send for WindowHandle {}
unsafe impl Sync for WindowHandle {}
