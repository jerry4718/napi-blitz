//! `AppHandler`: our own `winit::ApplicationHandler` that manages
//! `View` lifecycle and event dispatch directly, without going through
//! blitz-shell's `BlitzApplication`.
//!
//! Holds a `&mut BlitzApp` for the duration of one `pumpAppEvents` call.

use blitz::shell::{BlitzShellEvent, View};
use napi::bindgen_prelude::BigInt;
use std::{cell::RefCell, rc::Rc};
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId as WinitWindowId,
};

use crate::{
    app::{
        AppState, WindowEntry,
        bridge::{APP_EVENT_CLOSE, APP_EVENT_CLOSED, AppEventPayload},
    },
    window::WindowState,
};

pub struct AppHandler<'a> {
    pub state: &'a mut AppState,
}

impl<'a> AppHandler<'a> {
    /// Promote pending `WindowConfig`s into live `View`s. winit only
    /// fires `can_create_surfaces` on initial resume, so we must run
    /// this from every hook that has an `ActiveEventLoop`.
    fn drain_pending_windows(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.state.pending.is_empty() {
            return;
        }
        let proxy = self.state.proxy.clone();
        let configs = std::mem::take(&mut self.state.pending);
        for config in configs {
            let mut view = View::init(config, event_loop, &proxy);
            view.resume();
            let window_id = view.window_id();
            let inner = Rc::new(RefCell::new(WindowState {
                window: Some(view.window.clone()),
                closed: false,
            }));
            self.state
                .windows
                .insert(window_id, WindowEntry { view, state: inner });
        }
    }

    /// Process queued `BlitzShellEvent`s from the proxy channel.
    fn drain_shell_events(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(event) = self.state.event_queue.try_recv() {
            match event {
                BlitzShellEvent::Poll { window_id } => {
                    if let Some(window) = self.state.windows.get_mut(&window_id) {
                        window.view.poll();
                    }
                }
                BlitzShellEvent::ResumeReady { window_id } => {
                    if let Some(window) = self.state.windows.get_mut(&window_id) {
                        let ok = window.view.complete_resume();
                        debug_assert!(ok, "ResumeReady received but renderer not ready");
                    }
                }
                BlitzShellEvent::RequestRedraw { doc_id } => {
                    let entry = self
                        .state
                        .windows
                        .values_mut()
                        .find(|e| e.view.doc.id() == doc_id);
                    if let Some(entry) = entry {
                        entry.view.request_redraw();
                    }
                }
                BlitzShellEvent::CloseWindow { window_id } => {
                    if let Some(mut entry) = self.state.windows.remove(&window_id) {
                        entry.close();
                        drop(entry);
                        if self.state.windows.is_empty() {
                            event_loop.exit();
                        }
                        self.state.outstanding_windows =
                            self.state.outstanding_windows.saturating_sub(1);
                    }
                }
                // Embedder / Navigate / NavigationLoad: no-op
                _ => {}
            }
        }
    }
}

impl<'a> ApplicationHandler for AppHandler<'a> {
    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        for entry in self.state.windows.values_mut() {
            entry.view.resume();
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
        window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            if !self.state.windows.contains_key(&window_id) {
                return;
            }

            let wid = BigInt::from(window_id.into_raw() as u64);

            // Phase 1: dispatch a cancelable `close` event to JS.
            if let Some(bridge) = self.state.bridge.as_ref() {
                let result = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSE.to_string(),
                    window_id: wid.clone(),
                    cancelable: true,
                });
                if result.default_prevented {
                    return;
                }
            }

            // Phase 2: tear down the view.
            let Some(mut entry) = self.state.windows.remove(&window_id) else {
                return;
            };
            entry.close();
            drop(entry);
            if self.state.windows.is_empty() {
                event_loop.exit();
            }
            self.state.outstanding_windows = self.state.outstanding_windows.saturating_sub(1);

            // Phase 3: notify JS that the window is gone.
            if let Some(bridge) = self.state.bridge.as_ref() {
                let _ = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSED.to_string(),
                    window_id: wid,
                    cancelable: false,
                });
            }
            return;
        }

        // Non-close events: forward to the View's event handler.
        if let Some(entry) = self.state.windows.get_mut(&window_id) {
            entry.view.handle_winit_event(event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_pending_windows(event_loop);
        self.drain_shell_events(event_loop);
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for entry in self.state.windows.values_mut() {
            entry.view.suspend();
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for entry in self.state.windows.values_mut() {
            entry.view.suspend();
        }
    }
}
