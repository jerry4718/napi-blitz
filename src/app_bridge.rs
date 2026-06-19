//! Bridge between winit's `ApplicationHandler` events and the JS-side
//! `BlitzApp` / `Window` `EventTarget`s.
//!
//! Right now we only surface a tiny subset of winit's window-level
//! events:
//!
//! - `CloseRequested` (the OS-level "user clicked X") becomes a
//!   cancelable `close` event on the JS `Window`. JS code calling
//!   `event.preventDefault()` is reported back here so we can swallow
//!   the event before forwarding it to blitz's `BlitzApplication`
//!   (which would otherwise unconditionally drop the window).
//! - After a window has actually been removed (either because we
//!   forwarded a non-prevented `CloseRequested`, or because JS called
//!   `app.closeWindow()` synchronously), we dispatch a non-cancelable
//!   `closed` event so JS observers know the window is gone.
//!
//! The bridge holds a single JS callback (`onAppEvent`) that JS sets
//! via `BlitzApp.setAppEventHandler`. It receives an
//! [`AppEventPayload`] and returns an [`AppDispatchResult`] reporting
//! whether the JS side called `preventDefault()`.

use napi::{
    Env,
    bindgen_prelude::{Function, FunctionRef},
};
use napi_derive::napi;

/// Names of the events we currently surface. JS side compares against
/// these as plain strings; mirroring web `Event.type` shape.
pub const APP_EVENT_CLOSE: &str = "close";
pub const APP_EVENT_CLOSED: &str = "closed";

/// Payload handed to the JS-side app-event handler. One shape for all
/// of our app/window events; fields not relevant to a given event are
/// left at their defaults.
#[napi(object)]
pub struct AppEventPayload {
    /// `"close" | "closed"` for now.
    pub event_type: String,
    /// `BaseDocument::id` of the window the event refers to. JS uses
    /// this to map back to the right `Window` wrapper.
    pub window_doc_id: u32,
    /// Whether the JS `Event` constructed from this payload should be
    /// cancelable. Only `close` is cancelable; `closed` is not.
    pub cancelable: bool,
}

/// Result reported back from JS after dispatching an app event. A
/// missing call (handler not installed, or threw) acts as
/// `default_prevented = false`.
#[napi(object)]
pub struct AppDispatchResult {
    pub default_prevented: bool,
}

/// JS-side bridge for app/window events. Holds the napi callback we
/// invoke synchronously from inside `pump_app_events`.
pub struct JsAppBridge {
    pub env: Env,
    pub callback: FunctionRef<AppEventPayload, AppDispatchResult>,
}

impl JsAppBridge {
    pub fn new(env: Env, callback: FunctionRef<AppEventPayload, AppDispatchResult>) -> Self {
        Self { env, callback }
    }

    /// Dispatch an event to JS and return the resulting flags. Errors
    /// from the napi side (handler not callable, JS threw, ...) are
    /// printed and swallowed: we never want a JS-side glitch to crash
    /// the event loop.
    pub fn dispatch(&self, payload: AppEventPayload) -> AppDispatchResult {
        let cb: Function<AppEventPayload, AppDispatchResult> =
            match self.callback.borrow_back(&self.env) {
                Ok(cb) => cb,
                Err(err) => {
                    eprintln!("napi-blitz: failed to borrow app-event callback: {err}");
                    return AppDispatchResult {
                        default_prevented: false,
                    };
                }
            };
        match cb.call(payload) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("napi-blitz: app-event callback failed: {err}");
                AppDispatchResult {
                    default_prevented: false,
                }
            }
        }
    }
}
