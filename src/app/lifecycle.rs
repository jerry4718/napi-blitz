//! `Lifecycle`: how windows are born and die.
//!
//! The single owner of the open/close flow. Every entry point that
//! advances a window's lifecycle - a JS `openWindow`/`closeWindow` call,
//! a winit `CloseRequested`, a shell `CloseWindow` event, the pump's
//! synthetic-exit check - lands in a method on this type. It owns the
//! loop plumbing (the proxy, the shell-event receiver, the JS app ref
//! used for dispatch) and the `AppState` facts, and is itself shared as
//! an `Rc<Lifecycle>` between `NativeApp` and `AppHandler`.
//!
//! # Event matrix
//!
//! Every close follows request -> teardown -> notify, regardless of
//! entry point; the JS path simply splits the steps across pump ticks:
//!
//! | Entry               | Request (cancelable) | Teardown  | Notify (`closed` + `window:closed`) |
//! |---------------------|----------------------|-----------|-------------------------------------|
//! | JS `closeWindow`    | at call time         | next pump | after teardown, then the promise resolves |
//! | OS close button     | same tick            | same tick | after teardown |
//! | shell `CloseWindow` | -                    | same tick | - |
//!
//! Opens dispatch the cancelable `window:open` request before the
//! promotion and the `window:opened` notification after the promise
//! resolves.
//!
//! # Dispatch failure policy
//!
//! A failed dispatch leaves the window in its current state: a failed
//! `window:open` request is treated as confirmed, a failed close request
//! aborts the teardown, and failed notifications are logged and dropped.
//! Only the JS-facing `closeWindow` propagates the error to its promise.
//!
//! # Borrow discipline
//!
//! Winit callbacks and `NativeApp` methods re-enter JS, and re-entrant
//! JS may call back into this type. The rule that keeps the `RefCell`s
//! safe: every `state` borrow is dropped before any path that calls into
//! JS. Clone the dispatch pieces (`shared_doc`, `js_app_ref`) out under
//! a short borrow, drop the borrow, then dispatch.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::Receiver,
};

use blitz::shell::{BlitzShellEvent, BlitzShellProxy, View, WindowConfig};
use napi::{
    Env, Error, JsDeferred, JsValue, Result,
    bindgen_prelude::{Object, PromiseRaw, Undefined},
};
use winit::{event_loop::ActiveEventLoop, window::WindowId};

use crate::{
    app::{
        event_loop::EventLoopBox,
        state::{AppState, PendingRequest, WindowEntry},
    },
    dom::doc::{NativeDoc, SharedDoc},
    global,
    helpers::{JsWeakRef, dispatch_app_event, dispatch_window_event},
    renderer::CurrentRenderer,
    window::{
        NativeWindow, WindowState, make_window_document, options::WindowOptions,
        util::build_window_attributes,
    },
};

pub(crate) struct Lifecycle {
    /// Live windows and queued open/close requests - the facts of window
    /// life and death. Pure data; see `state.rs`.
    state: RefCell<AppState>,
    /// Proxy for sending events into the event loop (redraw, poll, etc.).
    proxy: BlitzShellProxy,
    /// Receiver for `BlitzShellEvent`s from the proxy channel.
    event_queue: Receiver<BlitzShellEvent>,
    /// Weak ref to the JS `BlitzApp` object, for dispatching app-level
    /// lifecycle events (`window:open`, `window:close`, `window:closed`).
    /// Set by `set_app_ref`; absent until JS opts in.
    js_app_ref: Rc<RefCell<Option<JsWeakRef>>>,
    /// True once at least one window has ever been opened. Without this,
    /// calling `pump_app_events` before any `open_window` would wrongly
    /// synthesize an exit on the very first pump.
    has_opened_window: Cell<bool>,
}

impl Lifecycle {
    pub(crate) fn new(proxy: BlitzShellProxy, event_queue: Receiver<BlitzShellEvent>) -> Self {
        Self {
            state: RefCell::new(AppState {
                windows: std::collections::HashMap::new(),
                pending_requests: Vec::new(),
            }),
            proxy,
            event_queue,
            js_app_ref: Rc::new(RefCell::new(None)),
            has_opened_window: Cell::new(false),
        }
    }

    /// Read access to the window facts, for `NativeApp`'s per-window
    /// queries (monitors, zoom) and `AppHandler`'s event routing. Writes
    /// go through the lifecycle methods above.
    pub(crate) fn state(&self) -> &RefCell<AppState> {
        &self.state
    }

    /// Store a weak ref to the JS `BlitzApp` object so Rust can dispatch
    /// app-level lifecycle events (`window:open`, `window:close`,
    /// `window:closed`) to it. Mirrors `NativeDoc::set_window_ref`.
    pub(crate) fn set_app_ref(&self, env: Env, app: Object) -> Result<()> {
        *self.js_app_ref.borrow_mut() = Some(JsWeakRef::new(&app, &env)?);
        Ok(())
    }

    // ── Lifecycle event dispatch ──────────────────────────────────────
    //
    // The whole event matrix (see the module docs). All of them re-enter
    // JS: call with no outstanding `state` borrow.

    /// Dispatch the cancelable `window:open` request to the app. JS holds
    /// no Window object yet, so the app is the only receiver. Returns
    /// `true` if the open should proceed.
    fn dispatch_open_request(&self, env: &Env) -> Result<bool> {
        Ok(!dispatch_app_event(
            &self.js_app_ref,
            "window:open",
            true,
            env,
        )?)
    }

    /// Dispatch the non-cancelable `window:opened` notification to the
    /// app, after the `openWindow` promise has resolved.
    fn notify_opened(&self, env: &Env) -> Result<()> {
        dispatch_app_event(&self.js_app_ref, "window:opened", false, env).map(|_| ())
    }

    /// Dispatch the cancelable close request: `close` to the window and,
    /// independently, `window:close` to the app. A `preventDefault()` at
    /// either level vetoes the close. Returns `true` if it should proceed.
    fn dispatch_close_request(&self, doc: &Rc<SharedDoc>, env: &Env) -> Result<bool> {
        let window_prevented = dispatch_window_event(doc, "close", true, env)?;
        let app_prevented = dispatch_app_event(&self.js_app_ref, "window:close", true, env)?;
        Ok(!window_prevented && !app_prevented)
    }

    /// Dispatch the non-cancelable `closed` (window) and `window:closed`
    /// (app) notifications, after the teardown has completed.
    fn notify_closed(&self, doc: &Rc<SharedDoc>, env: &Env) -> Result<()> {
        dispatch_window_event(doc, "closed", false, env)?;
        dispatch_app_event(&self.js_app_ref, "window:closed", false, env).map(|_| ())
    }

    /// Build the pieces for a pending open from the JS `DocHandle`, so
    /// `open_window` only has to create the promise and queue it.
    fn build_open_request(
        doc: &mut NativeDoc,
        options: Option<&WindowOptions>,
    ) -> Result<(WindowConfig<CurrentRenderer>, WindowState, Rc<SharedDoc>)> {
        if !doc.mark_attached() {
            return Err(Error::from_reason(
                "DocHandle has already been attached to a window".to_string(),
            ));
        }
        let shared_doc = doc.doc.clone();
        let window_doc = make_window_document(doc);
        let attributes = build_window_attributes(options)?;
        let config = WindowConfig::with_attributes(window_doc, CurrentRenderer::new(), attributes);

        let win_state = WindowState {
            window: None,
            closed: false,
        };
        Ok((config, win_state, shared_doc))
    }

    fn queue_open(
        &self,
        config: WindowConfig<CurrentRenderer>,
        state: WindowState,
        shared_doc: Rc<SharedDoc>,
        deferred: JsDeferred<NativeWindow, Box<dyn FnOnce(Env) -> Result<NativeWindow>>>,
    ) {
        self.state
            .borrow_mut()
            .pending_requests
            .push(PendingRequest::Open {
                config: Box::new(config),
                state,
                shared_doc,
                deferred,
            });
        self.has_opened_window.set(true);
    }

    /// Attach a new window to the given document handle. The same handle
    /// can only be attached to one window. The JS DocHandle keeps working
    /// after this call (it shares state with the window via
    /// `Rc<RefCell<...>>`), so JS can keep mutating the DOM afterward.
    ///
    /// Returns a `Promise<NativeWindow>` that resolves once a
    /// `pump_app_events` call promotes the pending config to a live
    /// window (see `drain_opening_windows`). This method never drives
    /// the event loop itself: it only queues the request, which makes it
    /// safe to invoke from inside an event handler - the in-flight pump
    /// creates the window, with no nested event-loop recursion.
    ///
    /// Because creation is deferred to the caller's pump, the caller
    /// must ensure a pump is (or will be) running before `await`-ing
    /// the result; otherwise the promise never resolves.
    pub(crate) fn open_window(
        &self,
        env: Env,
        doc: &mut NativeDoc,
        options: Option<&WindowOptions>,
    ) -> Result<PromiseRaw<'_, NativeWindow>> {
        let (config, win_state, shared_doc) = Self::build_open_request(doc, options)?;
        let (deferred, promise_obj) =
            env.create_deferred::<NativeWindow, Box<dyn FnOnce(Env) -> Result<NativeWindow>>>()?;
        let promise = PromiseRaw::new(env.raw(), JsValue::raw(&promise_obj));

        self.queue_open(config, win_state, shared_doc, deferred);

        Ok(promise)
    }

    /// Take the pending requests matching `take` out of the queue and put
    /// every other request back for a later pump. `None` when nothing is
    /// queued, so callers can bail out with a bare `return`.
    fn drain_pending(&self, take: impl Fn(&PendingRequest) -> bool) -> Option<Vec<PendingRequest>> {
        let mut state = self.state.borrow_mut();
        if state.pending_requests.is_empty() {
            return None;
        }
        let all = std::mem::take(&mut state.pending_requests);
        let (matched, remaining): (Vec<_>, Vec<_>) = all.into_iter().partition(take);
        state.pending_requests.extend(remaining);
        Some(matched)
    }

    /// Promote pending `WindowConfig`s into live `View`s. winit only
    /// fires `can_create_surfaces` on initial resume, so this must run
    /// from every hook that has an `ActiveEventLoop`.
    ///
    /// For each pending open, the cancelable app-level `window:open`
    /// event is dispatched *before* the window is promoted (and before
    /// the `openWindow` promise resolves - JS has no Window object yet).
    /// `preventDefault()` cancels the open: the fresh view is dropped
    /// and the promise is rejected. After a successful promotion the
    /// non-cancelable `window:opened` notification is dispatched once
    /// the promise resolves.
    pub(crate) fn drain_opening_windows(&self, event_loop: &EventLoopBox) {
        let Some(opens) = self.drain_pending(|req| matches!(req, PendingRequest::Open { .. }))
        else {
            return;
        };

        let proxy = self.proxy.clone();

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
            let mut view = View::init(*config, event_loop, &proxy);
            view.resume();
            let window_id = view.window_id();

            // Open confirmation, dispatched to the app from Rust. Must not
            // hold a state borrow: the dispatch re-enters JS and the
            // listener may call back into `NativeApp`.
            let allowed = match global::env() {
                Ok(env) => self.dispatch_open_request(&env).unwrap_or_else(|e| {
                    eprintln!(
                        "napi-blitz: drain_opening_windows: open request failed, treating open as confirmed: {e}"
                    );
                    true
                }),
                Err(e) => {
                    eprintln!(
                        "napi-blitz: drain_opening_windows: env not available, treating open as confirmed: {e}"
                    );
                    true
                }
            };
            if !allowed {
                drop(view);
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

            // Resolve outside the state borrow: the resolver constructs a
            // NativeWindow JS object, which is a pure napi operation.
            deferred.resolve(Box::new(move |_env| Ok(native)));

            // Post-open notification after the `openWindow` promise
            // resolves, dispatched to the app from Rust. No outstanding
            // state borrow: JS re-entry into `open_window` /
            // `request_close` is safe.
            let env = match global::env() {
                Ok(e) => e,
                Err(e) => {
                    eprintln!(
                        "napi-blitz: drain_opening_windows: env not available, skipping window:opened: {e}"
                    );
                    continue;
                }
            };
            if let Err(e) = self.notify_opened(&env) {
                eprintln!("napi-blitz: drain_opening_windows: notify_opened failed: {e}");
            }
        }
    }

    /// Queue the given window for closure and return a promise that
    /// resolves once the native `View` has actually been torn down
    /// (during the next pump, in `drain_closing_windows`), or rejects
    /// if a `close` listener calls `preventDefault()`.
    ///
    /// The cancelable `close` event is dispatched here, from Rust, at
    /// the moment the close is requested. If a listener prevents the
    /// default, the window stays open and the promise rejects.
    /// Otherwise, the JS-side `closed` flag is set immediately - only
    /// the physical teardown is async, mirroring `open_window`'s
    /// create-then-resolve.
    ///
    /// This is intentionally not GC-driven: dropping the JS `Window`
    /// object does not close the OS window. Callers must invoke this
    /// explicitly.
    pub(crate) fn request_close(
        &self,
        env: Env,
        window: &NativeWindow,
    ) -> Result<PromiseRaw<'_, Undefined>> {
        let window_id = window.window_id;

        // Decide under one short borrow: idempotency, the
        // already-torn-down shortcut, and cloning the dispatch pieces
        // (so no state borrow is held across the JS dispatch below).
        let shared_doc = {
            let state = self.state.borrow();
            // Public JS API guarantee: close() is idempotent. Multiple
            // calls are common when listeners race with UI state
            // updates, so only the first one queues a teardown; later
            // ones resolve immediately.
            if window.state.borrow().closed
                || state.pending_requests.iter().any(
                    |req| matches!(req, PendingRequest::Close { window_id: id, .. } if *id == window_id),
                )
            {
                None
            } else if !state.windows.contains_key(&window_id) {
                // The window is no longer live - e.g. torn down from the
                // OS side before this late JS close arrived. There is no
                // `View` to tear down and no window-level `close` event
                // to dispatch: just mark the handle closed.
                let mut ws = window.state.borrow_mut();
                ws.closed = true;
                ws.window = None;
                None
            } else {
                Some(Rc::clone(&state.windows.get(&window_id).expect("checked above").shared_doc))
            }
        };
        let Some(shared_doc) = shared_doc else {
            return PromiseRaw::resolve(&env, ());
        };

        // Dispatch the cancelable `close` event to the window from Rust.
        if !self.dispatch_close_request(&shared_doc, &env)? {
            // A `close` listener on the window or a `window:close`
            // listener on the app prevented the close: the window stays
            // open and the caller's promise rejects.
            let (deferred, promise_obj) =
                env.create_deferred::<Undefined, Box<dyn FnOnce(Env) -> Result<Undefined>>>()?;
            let promise = PromiseRaw::new(env.raw(), JsValue::raw(&promise_obj));
            deferred.reject(Error::from_reason("close prevented"));
            return Ok(promise);
        }

        let (deferred, promise_obj) =
            env.create_deferred::<Undefined, Box<dyn FnOnce(Env) -> Result<Undefined>>>()?;
        let promise = PromiseRaw::new(env.raw(), JsValue::raw(&promise_obj));

        // Mark the handle closed and queue the teardown. Pure state
        // mutation: no JS call, so the borrow is safe.
        {
            let mut ws = window.state.borrow_mut();
            ws.closed = true;
            ws.window = None;
        }
        self.state
            .borrow_mut()
            .pending_requests
            .push(PendingRequest::Close {
                window_id,
                deferred,
            });

        Ok(promise)
    }

    /// Remove a live window and release its OS window. The single
    /// teardown path behind `drain_closing_windows`, `close_from_os`,
    /// and the shell `CloseWindow` event. Returns the window's
    /// `SharedDoc` when a live window was removed, for the post-teardown
    /// notification.
    fn teardown_window(&self, window_id: WindowId) -> Option<Rc<SharedDoc>> {
        let mut state = self.state.borrow_mut();
        state.windows.remove(&window_id).map(|mut entry| {
            entry.close();
            Rc::clone(&entry.shared_doc)
        })
    }

    /// Exit the event loop once the last live window is gone. Only the
    /// OS-initiated paths call this; JS-initiated closes surface their
    /// exit through the synthetic check in `pump_app_events`.
    fn exit_if_empty(&self, event_loop: &EventLoopBox) {
        if self.state.borrow().windows.is_empty() {
            event_loop.exit();
        }
    }

    /// Handle a winit `CloseRequested`: dispatch the cancelable close
    /// request to the window and the app, and - unless prevented -
    /// tear the window down, notify, and exit the loop when it was the
    /// last one. Same request -> teardown -> notify order as the JS
    /// path (`request_close` + `drain_closing_windows`), all in one
    /// tick.
    pub(crate) fn close_from_os(&self, window_id: WindowId, event_loop: &EventLoopBox) {
        // Clone the dispatch pieces without holding a state borrow:
        // the dispatch re-enters JS (dispatchEvent may call back into
        // `NativeApp`), which must never see an outstanding borrow.
        let shared_doc = {
            let state = self.state.borrow();
            let Some(entry) = state.windows.get(&window_id) else {
                return;
            };
            Rc::clone(&entry.shared_doc)
        };
        let env = match global::env() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("napi-blitz: close_from_os: env not available: {e}");
                return;
            }
        };

        // Dispatch `close` (window) + `window:close` (app), both
        // cancelable. If either was prevented, abort - the window stays
        // open.
        match self.dispatch_close_request(&shared_doc, &env) {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                eprintln!("napi-blitz: close_from_os: close request failed: {e}");
                return;
            }
        }

        if self.teardown_window(window_id).is_some() {
            self.exit_if_empty(event_loop);
        }

        if let Err(e) = self.notify_closed(&shared_doc, &env) {
            eprintln!("napi-blitz: close_from_os: notify_closed failed: {e}");
        }
    }

    /// Tear down windows whose close was requested between pump ticks.
    /// `window.close()` may be invoked at any time; the queued `Close`
    /// request is fulfilled here so the teardown and the `window:closed`
    /// notification land before the synthetic-exit check observes zero
    /// outstanding windows.
    pub(crate) fn drain_closing_windows(&self) {
        let Some(closing) = self.drain_pending(|req| matches!(req, PendingRequest::Close { .. }))
        else {
            return;
        };

        for req in closing {
            let PendingRequest::Close {
                window_id,
                deferred,
            } = req
            else {
                unreachable!()
            };

            // 1. Remove + close the entry (needs &mut state), cloning the
            //    dispatch pieces so no borrow is held across the JS
            //    dispatch.
            let shared_doc = self.teardown_window(window_id);

            // 2. Notify from Rust: `window:closed` on the window,
            //    propagated up to the app. No outstanding state borrow,
            //    so JS re-entry into `open_window` / `request_close` is
            //    safe. The cancelable close request (`window:close`)
            //    was already dispatched by `request_close`; here only
            //    the post-teardown notification remains.
            if let Some(shared_doc) = shared_doc {
                let env = match global::env() {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("napi-blitz: drain_closing_windows: env not available: {e}");
                        return;
                    }
                };
                if let Err(e) = self.notify_closed(&shared_doc, &env) {
                    eprintln!("napi-blitz: drain_closing_windows: notify_closed failed: {e}");
                }
            }

            // 3. Fulfil the `close_window` promise after the
            //    notifications, so JS-side await sees the teardown
            //    fully complete.
            deferred.resolve(Box::new(move |_env| Ok(())));
        }
    }

    /// Process queued `BlitzShellEvent`s from the proxy channel.
    pub(crate) fn drain_shell_events(&self, event_loop: &EventLoopBox) {
        while let Ok(event) = self.event_queue.try_recv() {
            match event {
                BlitzShellEvent::Poll { window_id } => {
                    if let Some(window) = self.state.borrow().windows.get(&window_id) {
                        window.view.borrow_mut().poll();
                    }
                }
                BlitzShellEvent::ResumeReady { window_id } => {
                    if let Some(window) = self.state.borrow().windows.get(&window_id) {
                        let ok = window.view.borrow_mut().complete_resume();
                        debug_assert!(ok, "ResumeReady received but renderer not ready");
                    }
                }
                BlitzShellEvent::RequestRedraw { doc_id } => {
                    let view = self
                        .state
                        .borrow()
                        .windows
                        .values()
                        .find(|e| e.view.borrow().doc.id() == doc_id)
                        .map(|e| Rc::clone(&e.view));
                    if let Some(view) = view {
                        view.borrow_mut().request_redraw();
                    }
                }
                BlitzShellEvent::CloseWindow { window_id }
                    if self.teardown_window(window_id).is_some() =>
                {
                    self.exit_if_empty(event_loop);
                }
                // Embedder / Navigate / NavigationLoad: no-op
                _ => {}
            }
        }
    }

    /// Number of windows still logically alive: live views, plus opens
    /// queued but not yet promoted, minus closes already queued. A
    /// re-entry between the idempotency check and the push can queue a
    /// second `Close` for a window that exists once in `windows`, so the
    /// count may go negative; the drain consumes the surplus `Close`
    /// idempotently, which is why clamping to zero is sound.
    fn outstanding_windows(&self) -> usize {
        let state = self.state.borrow();
        let outstanding =
            state
                .pending_requests
                .iter()
                .fold(state.windows.len() as isize, |n, req| match req {
                    PendingRequest::Open { .. } => n + 1,
                    PendingRequest::Close { .. } => n - 1,
                });
        if outstanding < 0 {
            eprintln!(
                "napi-blitz: outstanding_windows: negative count ({outstanding}), clamping to zero"
            );
            0
        } else {
            outstanding as usize
        }
    }

    /// Whether the pump should surface a synthetic `Exit`: at least one
    /// window has ever been opened, and none is logically alive anymore.
    /// winit's `pump_app_events` mode never exits on its own; the
    /// OS-initiated path already calls `event_loop.exit()` from
    /// `close_from_os`, but JS-initiated closes bypass winit's pipeline
    /// entirely, so the pump syntheses the exit here.
    pub(crate) fn should_synthetic_exit(&self) -> bool {
        self.has_opened_window.get() && self.outstanding_windows() == 0
    }
}
