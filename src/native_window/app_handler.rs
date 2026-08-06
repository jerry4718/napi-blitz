//! `JsAppHandler`: our own `winit::ApplicationHandler` that manages
//! `View` lifecycle and event dispatch directly, without going through
//! blitz-shell's `BlitzApplication`.
//!
//! Holds a `&mut BlitzApp` for the duration of one `pumpAppEvents` call.

use std::sync::Arc;

use blitz::shell::{BlitzShellEvent, View};
use blitz::traits::shell::DummyShellProvider;
use napi::bindgen_prelude::BigInt;
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId,
};

use crate::native_window::app::BlitzApp;
use crate::native_window::app_bridge::{
    APP_EVENT_CLOSE, APP_EVENT_CLOSED, AppEventPayload, JsAppBridge,
};

pub struct JsAppHandler<'a> {
    pub app: &'a mut BlitzApp,
}

impl<'a> JsAppHandler<'a> {
    /// Look up the doc id for a given winit `WindowId`.
    fn doc_id_for(&self, window_id: WindowId) -> Option<usize> {
        self.app.windows.get(&window_id).map(|view| view.doc.id())
    }

    /// Promote pending `WindowConfig`s into live `View`s. winit only
    /// fires `can_create_surfaces` on initial resume, so we must run
    /// this from every hook that has an `ActiveEventLoop`.
    fn drain_pending_windows(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.app.pending.is_empty() {
            return;
        }
        let proxy = self.app.proxy.clone();
        let configs = std::mem::take(&mut self.app.pending);
        for (_doc_id, config) in configs {
            let mut view = View::init(config, event_loop, &proxy);
            view.resume();
            self.app.windows.insert(view.window_id(), view);
        }
    }

    /// Process queued `BlitzShellEvent`s from the proxy channel.
    fn drain_shell_events(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(event) = self.app.event_queue.try_recv() {
            match event {
                BlitzShellEvent::Poll { window_id } => {
                    if let Some(window) = self.app.windows.get_mut(&window_id) {
                        window.poll();
                    }
                }
                BlitzShellEvent::ResumeReady { window_id } => {
                    if let Some(window) = self.app.windows.get_mut(&window_id) {
                        let ok = window.complete_resume();
                        debug_assert!(ok, "ResumeReady received but renderer not ready");
                    }
                }
                BlitzShellEvent::RequestRedraw { doc_id } => {
                    let view = self.app.windows.values_mut().find(|v| v.doc.id() == doc_id);
                    if let Some(view) = view {
                        view.request_redraw();
                    }
                }
                BlitzShellEvent::CloseWindow { window_id } => {
                    if let Some(mut view) = self.app.windows.remove(&window_id) {
                        view.doc
                            .inner_mut()
                            .set_shell_provider(Arc::new(DummyShellProvider));
                        drop(view);
                        if self.app.windows.is_empty() {
                            event_loop.exit();
                        }
                        self.app.outstanding_windows =
                            self.app.outstanding_windows.saturating_sub(1);
                    }
                }
                // Embedder / Navigate / NavigationLoad: no-op
                _ => {}
            }
        }
    }
}

impl<'a> ApplicationHandler for JsAppHandler<'a> {
    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        for view in self.app.windows.values_mut() {
            view.resume();
        }
        self.drain_pending_windows(event_loop);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_pending_windows(event_loop);
        self.drain_shell_events(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            let doc_id = match self.doc_id_for(window_id) {
                Some(id) => id,
                None => return,
            };

            // Phase 1: dispatch a cancelable `close` event to JS.
            if let Some(bridge) = self.app.bridge.as_ref() {
                let result = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSE.to_string(),
                    window_doc_id: BigInt::from(doc_id as u64),
                    cancelable: true,
                });
                if result.default_prevented {
                    return;
                }
            }

            // Phase 2: tear down the view.
            let Some(mut view) = self.app.windows.remove(&window_id) else {
                return;
            };
            view.doc
                .inner_mut()
                .set_shell_provider(Arc::new(DummyShellProvider));
            drop(view);
            if self.app.windows.is_empty() {
                event_loop.exit();
            }
            self.app.outstanding_windows = self.app.outstanding_windows.saturating_sub(1);

            // Phase 3: notify JS that the window is gone.
            if let Some(bridge) = self.app.bridge.as_ref() {
                let _ = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSED.to_string(),
                    window_doc_id: BigInt::from(doc_id as u64),
                    cancelable: false,
                });
            }
            return;
        }

        // Non-close events: forward to the View's event handler.
        if let Some(view) = self.app.windows.get_mut(&window_id) {
            view.handle_winit_event(event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_pending_windows(event_loop);
        self.drain_shell_events(event_loop);
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for view in self.app.windows.values_mut() {
            view.suspend();
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for view in self.app.windows.values_mut() {
            view.suspend();
        }
    }
}
