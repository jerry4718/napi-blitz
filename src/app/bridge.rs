//! Bridge between winit's `ApplicationHandler` events and the JS-side
//! `BlitzApp` / `Window` `EventTarget`s.

use napi::{
    Env,
    bindgen_prelude::{BigInt, Function, FunctionRef},
};

/// Names of the events we currently surface. JS side compares against
/// these as plain strings; mirroring web `Event.type` shape.
pub const APP_EVENT_CLOSE: &str = "close";
pub const APP_EVENT_CLOSED: &str = "closed";

/// Payload handed to the JS-side app-event handler.
#[napi(object)]
pub struct AppEventPayload {
    /// `"close" | "closed"` for now.
    #[napi(js_name = "type")]
    pub event_type: String,
    /// Opaque window identifier. JS uses this to look up the
    /// matching `Window` wrapper.
    pub window_id: BigInt,
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
        cb.call(payload).unwrap_or_else(|err| {
            eprintln!("napi-blitz: app-event callback failed: {err}");
            AppDispatchResult {
                default_prevented: false,
            }
        })
    }
}
