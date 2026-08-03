//! `JsAppHandler`: a thin `winit::ApplicationHandler` wrapper that
//! forwards every event to the underlying [`blitz::shell::BlitzApplication`]
//! after giving JS a chance to observe (and, for `CloseRequested`,
//! cancel) it.
//!
//! The wrapper is intentionally constructed *per* `pump_app_events`
//! call: it borrows the inner application and the optional JS bridge
//! out of [`crate::app::BlitzApp`] for the duration of one pump. This
//! lets us add the JS hook without growing `BlitzApp`'s lifetime
//! contract — the bridge stays owned by `BlitzApp` and can be swapped
//! at runtime via `setAppEventHandler`.

use anyrender_vello::VelloWindowRenderer;
use blitz::shell::{BlitzApplication, View};
use blitz::traits::shell::DummyShellProvider;
use napi::bindgen_prelude::BigInt;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId,
};

use crate::native_window::app_bridge::{
    APP_EVENT_CLOSE, APP_EVENT_CLOSED, AppEventPayload, JsAppBridge,
};

pub struct JsAppHandler<'a> {
    pub inner: &'a mut BlitzApplication<VelloWindowRenderer>,
    pub bridge: Option<&'a JsAppBridge>,
    /// Mirrors `BlitzApp::outstanding_windows` — decremented when the
    /// OS-initiated `CloseRequested` path successfully tears down a
    /// window. Keeping it in sync means `BlitzApp::pump_app_events`
    /// can still detect "all windows gone" if the user mixes OS and
    /// JS-initiated closes.
    pub outstanding: &'a mut usize,
}

impl<'a> JsAppHandler<'a> {
    /// Look up the doc id for a given winit `WindowId` by walking the
    /// inner application's `windows` map. Returns `None` if the window
    /// is no longer alive (e.g. JS already closed it synchronously).
    fn doc_id_for(&self, window_id: WindowId) -> Option<usize> {
        self.inner.windows.get(&window_id).map(|view| view.doc.id())
    }

    /// Drain `BlitzApplication::pending_windows` and turn each entry
    /// into a live `View` ourselves. blitz's own promotion happens in
    /// `can_create_surfaces`, but winit only fires that on initial
    /// resume / surface re-creation — not when JS pushes a new
    /// window config at runtime. So we run the same logic from any
    /// hook that has an `ActiveEventLoop` (about_to_wait,
    /// window_event, proxy_wake_up). This is intentionally idempotent
    /// and a no-op when pending_windows is empty.
    ///
    /// We keep the order of `View::init` -> `windows.insert` matched
    /// to blitz's own implementation so behaviour stays identical to
    /// the startup path.
    fn drain_pending_windows(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.inner.pending_windows.is_empty() {
            return;
        }
        let proxy = self.inner.proxy.clone();
        let configs = std::mem::take(&mut self.inner.pending_windows);
        for window_config in configs {
            let mut view = View::init(window_config, event_loop, &proxy);
            view.resume();
            self.inner.windows.insert(view.window_id(), view);
        }
    }
}

impl<'a> ApplicationHandler for JsAppHandler<'a> {
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.resumed(event_loop);
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.can_create_surfaces(event_loop);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        // Promote any JS-queued windows before forwarding so blitz can
        // route subsequent shell events to live `View`s rather than
        // dropping them.
        self.drain_pending_windows(event_loop);
        self.inner.proxy_wake_up(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            // Resolve the doc id *before* forwarding: blitz removes
            // the window from `windows` synchronously inside
            // `window_event`, so a post-forward lookup would return
            // None.
            let doc_id = match self.doc_id_for(window_id) {
                Some(id) => id,
                None => {
                    // Window already gone — nothing to ask JS about,
                    // and forwarding is a no-op too.
                    return;
                }
            };

            // Phase 1: dispatch a cancelable `close` event.
            if let Some(bridge) = self.bridge {
                let result = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSE.to_string(),
                    window_doc_id: BigInt::from(doc_id as u64),
                    cancelable: true,
                });
                if result.default_prevented {
                    // JS asked us to keep the window. Drop the event
                    // on the floor; the inner `BlitzApplication` never
                    // sees it.
                    return;
                }
            }

            // Phase 2: remove the view ourselves instead of forwarding
            // to blitz. blitz would remove/drop the `View`, but the
            // document's `ShellProvider` also owns an Arc to the winit
            // window. Reset it first so the OS window is actually
            // released when the View goes away.
            let Some(mut view) = self.inner.windows.remove(&window_id) else {
                return;
            };
            view.doc
                .inner_mut()
                .set_shell_provider(Arc::new(DummyShellProvider));
            drop(view);
            if self.inner.windows.is_empty() {
                event_loop.exit();
            }
            *self.outstanding = self.outstanding.saturating_sub(1);

            // Phase 3: notify JS that the window is gone. Always
            // non-cancelable.
            if let Some(bridge) = self.bridge {
                let _ = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSED.to_string(),
                    window_doc_id: BigInt::from(doc_id as u64),
                    cancelable: false,
                });
            }
            return;
        }

        // Non-close events: pass through unchanged.
        self.inner.window_event(event_loop, window_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        // Last-chance promotion before winit goes back to sleep. After
        // initial `resumed`/`can_create_surfaces`, this is the only
        // hook reliably fired every pump, so it has to handle the
        // common case of "JS just called openWindow synchronously
        // and we need the View ready before we yield".
        self.drain_pending_windows(event_loop);
        self.inner.about_to_wait(event_loop);
    }

    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.destroy_surfaces(event_loop);
    }
}
