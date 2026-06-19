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

use napi_derive::napi;

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
