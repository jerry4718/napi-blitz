//! `BlitzApp`: the JS-facing wrapper around blitz's winit-based application.
//!
//! `BlitzApp.create()` builds an event loop and a `BlitzApplication`. Calling
//! `openWindow(docHandle)` produces a `Box<dyn Document>` from the handle and
//! attaches a fresh window to it. JS drives the loop synchronously via
//! `pumpAppEvents(millis)` from the main thread; this keeps event callbacks
//! re-entrant on the napi env so we can call back into JS without a
//! ThreadsafeFunction.

use std::time::Duration;

use anyrender_vello::VelloWindowRenderer;
use blitz::shell::{
    BlitzApplication, BlitzShellProxy, EventLoop, WindowConfig, create_default_event_loop,
};
use napi::{Error, Result};
use napi_derive::napi;
use winit::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};

use crate::doc::{DocHandle, make_window_document};

/// Result of one `pumpAppEvents` call.
#[napi(object)]
pub struct PumpResult {
    /// The loop is still running. Caller should pump again later.
    pub running: bool,
    /// The loop has exited (e.g. all windows closed).
    pub exited: bool,
    /// Exit code, if `exited`.
    pub code: Option<i32>,
}

#[napi]
pub struct BlitzApp {
    event_loop: EventLoop,
    application: BlitzApplication<VelloWindowRenderer>,
}

#[napi]
impl BlitzApp {
    /// Build the winit event loop and underlying blitz application.
    #[napi(factory)]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        let application = BlitzApplication::new(proxy, receiver);
        Self {
            event_loop,
            application,
        }
    }

    /// Attach a new window to the given document handle. The same handle can
    /// only be attached to one window. The JS DocHandle keeps working after
    /// this call (it shares state with the window via Rc<RefCell<...>>), so
    /// JS can keep mutating the DOM after `openWindow`.
    #[napi]
    pub fn open_window(&mut self, doc: &mut DocHandle) -> Result<()> {
        if !doc.mark_attached() {
            return Err(Error::from_reason(
                "DocHandle has already been attached to a window".to_string(),
            ));
        }
        let window_doc = make_window_document(doc);
        self.application
            .add_window(WindowConfig::new(window_doc, VelloWindowRenderer::new()));
        Ok(())
    }

    /// Pump pending winit events for at most `millis` milliseconds. JS should
    /// call this in a loop (typically once per animation frame) to drive the
    /// renderer and event handling.
    #[napi]
    pub fn pump_app_events(&mut self, millis: f64) -> PumpResult {
        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));
        match self
            .event_loop
            .pump_app_events(timeout, &mut self.application)
        {
            PumpStatus::Continue => PumpResult {
                running: true,
                exited: false,
                code: None,
            },
            PumpStatus::Exit(code) => PumpResult {
                running: false,
                exited: true,
                code: Some(code),
            },
        }
    }
}
