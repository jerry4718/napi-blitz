//! `Window`: napi-facing handle to one open OS window.
//!
//! A `Window` is identified internally by the document id of its attached
//! document (blitz's `BaseDocument::id()`). The doc id is allocated at
//! `DocHandle` creation time, so we know it before winit has minted a real
//! `WindowId` (winit only assigns one inside `can_create_surfaces`, which runs
//! during the next `pump_app_events`).
//!
//! Closing is synchronous: `BlitzApp.close_window` mutates the application's
//! `windows` map directly. We do NOT rely on JS GC to drop windows — the JS
//! side must call `window.close()` (or `app.closeWindow(window)`) explicitly.
//!
//! Runtime configuration (size, resizable, ...) lives on `BlitzApp` rather
//! than `Window` itself, because the napi `Window` handle does not own a
//! reference back to the live winit `Arc<dyn Window>` — the application
//! does. The JS layer's `Window` class delegates these calls to the app.

use napi_derive::napi;

/// Options accepted by `BlitzApp.openWindow`. All fields are optional and
/// map directly to winit `WindowAttributes` (0.31). Naming follows winit
/// where it diverges from web (e.g. `surface_size` rather than
/// `inner_size` — winit 0.31 renamed inner -> surface).
#[napi(object)]
pub struct WindowOptions {
    /// Initial window title. May be transient: if the document carries a
    /// `<title>` element, blitz will overwrite the title on the next
    /// mutator flush. Without a `<title>` element this title persists.
    pub title: Option<String>,
    /// Initial surface width in physical pixels.
    pub width: Option<u32>,
    /// Initial surface height in physical pixels.
    pub height: Option<u32>,
    /// Whether the window is initially resizable. Defaults to winit's
    /// platform default (typically `true`).
    pub resizable: Option<bool>,
}

/// Handle to an open window. Construct via `BlitzApp.openWindow`.
#[napi]
pub struct Window {
    /// blitz `BaseDocument` id; uniquely identifies the window for as long as
    /// it is open. Internal-only — the JS layer does not need to see this.
    pub(crate) doc_id: usize,
    /// Set to true once `BlitzApp.close_window` has run for this window.
    pub(crate) closed: bool,
}

#[napi]
impl Window {
    /// Whether `closeWindow` has run for this handle.
    #[napi(getter)]
    pub fn closed(&self) -> bool {
        self.closed
    }
}
