//! `AppHandler`: our own `winit::ApplicationHandler` that manages
//! `View` lifecycle and event dispatch directly, without going through
//! blitz-shell's `BlitzApplication`.
//!
//! Shares ownership of `AppState` via `Rc<RefCell<...>>` rather than borrowing
//! `&mut AppState`. This decouples the handler's lifetime from a single
//! `pumpAppEvents` call stack and — crucially — means winit callbacks that
//! re-enter JS no longer conflict with a `&mut self` borrow on `NativeApp`.
//! The rule that keeps `RefCell` safe: drop every `AppState` borrow before any
//! path that calls into JS.

use blitz::shell::{BlitzShellEvent, View};
use napi::bindgen_prelude::BigInt;
use std::{cell::RefCell, rc::Rc};
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId as WinitWindowId,
};

use crate::{
    app::{
        AppState, NativeWindow, WindowEntry,
        bridge::{APP_EVENT_CLOSE, APP_EVENT_CLOSED, AppEventPayload},
    },
};

pub struct AppHandler {
    pub state: Rc<RefCell<AppState>>,
}

impl AppHandler {
    /// Promote pending `WindowConfig`s into live `View`s. winit only
    /// fires `can_create_surfaces` on initial resume, so we must run
    /// this from every hook that has an `ActiveEventLoop`.
    fn drain_pending_windows(&mut self, event_loop: &dyn ActiveEventLoop) {
        let mut state = self.state.borrow_mut();
        if state.pending_windows.is_empty() {
            return;
        }
        let proxy = state.proxy.clone();
        let pending_windows = std::mem::take(&mut state.pending_windows);
        for pending in pending_windows {
            let mut view = View::init(pending.config, event_loop, &proxy);
            view.resume();
            let window_id = view.window_id();

            // Fill the shared WindowState so the JS-side NativeWindow sees a
            // live OS window, then build the value that resolves the promise
            // `open_window` handed back to JS.
            pending.state.borrow_mut().window = Some(view.window.clone());
            let native = NativeWindow {
                window_id,
                state: pending.state,
            };
            let entry = WindowEntry {
                view: Rc::new(RefCell::new(view)),
                state: native.state.clone(),
            };
            state.windows.insert(window_id, entry);

            // Resolve outside the `state` borrow: the resolver constructs a
            // NativeWindow JS object, which is a pure napi operation.
            pending
                .deferred
                .resolve(Box::new(move |_env| Ok(native)));
        }
    }

    /// Process queued `BlitzShellEvent`s from the proxy channel.
    fn drain_shell_events(&mut self, event_loop: &dyn ActiveEventLoop) {
        let mut state = self.state.borrow_mut();
        while let Ok(event) = state.event_queue.try_recv() {
            match event {
                BlitzShellEvent::Poll { window_id } => {
                    if let Some(window) = state.windows.get(&window_id) {
                        window.view.borrow_mut().poll();
                    }
                }
                BlitzShellEvent::ResumeReady { window_id } => {
                    if let Some(window) = state.windows.get(&window_id) {
                        let ok = window.view.borrow_mut().complete_resume();
                        debug_assert!(ok, "ResumeReady received but renderer not ready");
                    }
                }
                BlitzShellEvent::RequestRedraw { doc_id } => {
                    let entry = state
                        .windows
                        .values()
                        .find(|e| e.view.borrow().doc.id() == doc_id);
                    if let Some(entry) = entry {
                        entry.view.borrow_mut().request_redraw();
                    }
                }
                BlitzShellEvent::CloseWindow { window_id } => {
                    if let Some(mut entry) = state.windows.remove(&window_id) {
                        entry.close();
                        drop(entry);
                        if state.windows.is_empty() {
                            event_loop.exit();
                        }
                        state.outstanding_windows =
                            state.outstanding_windows.saturating_sub(1);
                    }
                }
                // Embedder / Navigate / NavigationLoad: no-op
                _ => {}
            }
        }
    }
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        {
            let state = self.state.borrow();
            for entry in state.windows.values() {
                entry.view.borrow_mut().resume();
            }
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
            // Phase 1: dispatch a cancelable `close` event to JS. We must not
            // hold an AppState borrow across `bridge.dispatch` (it re-enters JS).
            {
                let state = self.state.borrow();
                if !state.windows.contains_key(&window_id) {
                    return;
                }
            }

            let wid = BigInt::from(window_id.into_raw() as u64);

            let default_prevented = {
                let state = self.state.borrow();
                if let Some(bridge) = state.bridge.as_ref() {
                    bridge
                        .dispatch(AppEventPayload {
                            event_type: APP_EVENT_CLOSE.to_string(),
                            window_id: wid.clone(),
                            cancelable: true,
                        })
                        .default_prevented
                } else {
                    false
                }
            };
            if default_prevented {
                return;
            }

            // Phase 2: tear down the view.
            {
                let mut state = self.state.borrow_mut();
                let Some(mut entry) = state.windows.remove(&window_id) else {
                    return;
                };
                entry.close();
                drop(entry);
                if state.windows.is_empty() {
                    event_loop.exit();
                }
                state.outstanding_windows = state.outstanding_windows.saturating_sub(1);
            }

            // Phase 3: notify JS that the window is gone. No outstanding
            // borrow, so JS re-entry is safe.
            let state = self.state.borrow();
            if let Some(bridge) = state.bridge.as_ref() {
                let _ = bridge.dispatch(AppEventPayload {
                    event_type: APP_EVENT_CLOSED.to_string(),
                    window_id: wid,
                    cancelable: false,
                });
            }
            return;
        }

        // Non-close events: forward to the View's event handler.
        //
        // `handle_winit_event` may re-enter JS (click -> spawn -> openWindow).
        // During that re-entry JS may call `NativeApp::open_window` etc., which
        // do `self.state.borrow_mut()`. So we must not hold the outer AppState
        // borrow across `handle_winit_event`.
        //
        // `view` is `Rc<RefCell<View>>`: we clone the Rc out while holding a
        // short `state.borrow()`, drop the state borrow, then call into the
        // view. The view's own RefCell borrow is held across the JS callback,
        // but re-entrant JS never touches *this* view except through a fresh
        // `AppState` borrow (which no longer conflicts), so this is safe.
        let view_rc = {
            let state = self.state.borrow();
            state.windows.get(&window_id).map(|e| Rc::clone(&e.view))
        };
        if let Some(view_rc) = view_rc {
            view_rc.borrow_mut().handle_winit_event(event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_pending_windows(event_loop);
        self.drain_shell_events(event_loop);
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        let state = self.state.borrow();
        for entry in state.windows.values() {
            entry.view.borrow_mut().suspend();
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        let state = self.state.borrow();
        for entry in state.windows.values() {
            entry.view.borrow_mut().suspend();
        }
    }
}
