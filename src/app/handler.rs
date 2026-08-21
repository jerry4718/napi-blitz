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
use napi::Error;
use std::{cell::RefCell, rc::Rc};
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId as WinitWindowId,
};

use crate::{
    app::{AppState, NativeWindow, PendingRequest, WindowEntry, shell_event::JsShellEventHandler},
    global,
};

pub struct AppHandler {
    pub state: Rc<RefCell<AppState>>,
}

impl AppHandler {
    /// Promote pending `WindowConfig`s into live `View`s. winit only
    /// fires `can_create_surfaces` on initial resume, so we must run
    /// this from every hook that has an `ActiveEventLoop`.
    ///
    /// For each pending open, the cancelable app-level `window:open`
    /// event is dispatched *before* the window is promoted (and before
    /// the `openWindow` promise resolves — JS has no Window object yet).
    /// `preventDefault()` cancels the open: the fresh view is dropped
    /// and the promise is rejected.
    fn drain_pending_windows(&mut self, event_loop: &dyn ActiveEventLoop) {
        let (proxy, opens) = {
            let mut state = self.state.borrow_mut();
            if state.pending_requests.is_empty() {
                return;
            }
            let proxy = state.proxy.clone();
            let all = std::mem::take(&mut state.pending_requests);
            let (opens, remaining): (Vec<_>, Vec<_>) = all
                .into_iter()
                .partition(|req| matches!(req, PendingRequest::Open { .. }));
            state.pending_requests.extend(remaining);
            (proxy, opens)
        };

        for req in opens {
            let PendingRequest::Open {
                config,
                state: win_state,
                shared_doc,
                deferred,
            } = req
            else {
                unreachable!()
            };
            let mut view = View::init(config, event_loop, &proxy);
            view.resume();
            let window_id = view.window_id();

            // Open confirmation, dispatched to the app from Rust. Must not
            // hold an AppState borrow: the dispatch re-enters JS and the
            // listener may call back into `NativeApp`.
            let js_app_ref = Rc::clone(&self.state.borrow().js_app_ref);
            let allowed = match global::env() {
                Ok(env) => match JsShellEventHandler::new(js_app_ref).open_sequence(&env) {
                    Ok(allowed) => allowed,
                    Err(e) => {
                        eprintln!(
                            "napi-blitz: drain_pending_windows: open_sequence failed, treating open as confirmed: {e}"
                        );
                        true
                    }
                },
                Err(e) => {
                    eprintln!(
                        "napi-blitz: drain_pending_windows: env not available, treating open as confirmed: {e}"
                    );
                    true
                }
            };
            if !allowed {
                drop(view);
                // The open was cancelled: roll back the outstanding-window
                // count so the synthetic-exit check can still fire once the
                // other windows close.
                {
                    let mut state = self.state.borrow_mut();
                    state.outstanding_windows = state.outstanding_windows.saturating_sub(1);
                }
                deferred.reject(Error::from_reason("window open prevented"));
                continue;
            }

            // Now that the OS window exists, the bare WindowState becomes
            // shared: wrap it and fill in the live OS window.
            let shared = Rc::new(RefCell::new(win_state));
            shared.borrow_mut().window = Some(view.window.clone());
            let native = NativeWindow {
                window_id,
                state: shared.clone(),
            };
            let entry = WindowEntry {
                view: Rc::new(RefCell::new(view)),
                state: shared.clone(),
                shared_doc,
            };
            self.state.borrow_mut().windows.insert(window_id, entry);

            // Resolve outside the `state` borrow: the resolver constructs a
            // NativeWindow JS object, which is a pure napi operation.
            deferred.resolve(Box::new(move |_env| Ok(native)));
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
                        state.outstanding_windows = state.outstanding_windows.saturating_sub(1);
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
            // Clone the shell-event dispatch pieces without holding an
            // AppState borrow: `close_sequence` re-enters JS (dispatchEvent
            // may call back into `NativeApp`), which must never see an
            // outstanding borrow.
            let (shared_doc, app_ref) = {
                let state = self.state.borrow();
                let Some(entry) = state.windows.get(&window_id) else {
                    return;
                };
                (Rc::clone(&entry.shared_doc), Rc::clone(&state.js_app_ref))
            };
            let env = match global::env() {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("napi-blitz: window_event CloseRequested: env not available: {e}");
                    return;
                }
            };
            let handler = JsShellEventHandler::new(app_ref);

            // Dispatch `close` (cancelable) -> `closed` (window) ->
            // `window:close` + `window:closed` (app). If `close` was
            // prevented, abort — the window stays open.
            match handler.close_sequence(&shared_doc, &env) {
                // Not prevented: proceed to tear down.
                Ok(true) => {}
                // A `close` / `window:close` listener prevented the close.
                Ok(false) => return,
                Err(e) => {
                    eprintln!(
                        "napi-blitz: window_event CloseRequested: close_sequence failed: {e}"
                    );
                    return;
                }
            }

            // Tear down the view.
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
