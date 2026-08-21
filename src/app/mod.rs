//! `BlitzApp`: the JS-facing wrapper around a winit event loop.
//!
//! `BlitzApp.create()` builds an event loop. Calling `openWindow(docHandle)`
//! produces a `Box<dyn Document>` from the handle and attaches a fresh window
//! to it. JS drives the loop synchronously via `pumpAppEvents(millis)` from
//! the main thread; this keeps event callbacks re-entrant on the napi env so
//! we can call back into JS without a ThreadsafeFunction.
//!
//! # Async `openWindow`
//!
//! `openWindow` is async: it returns a `Promise<NativeWindow>` and never
//! recursively drives the event loop. Window creation happens inside a
//! *later* `pump_app_events` (via `AppHandler::drain_pending_windows`), where
//! winit hands us an `ActiveEventLoop`. Two paths resolve the promise:
//!
//! - Outside a pump (initial setup): `open_window` runs one non-recursive
//!   pump itself, so the caller's `await` resolves immediately.
//! - Inside a pump (an event handler): `open_window` only queues the request;
//!   the current or next pump's `drain_pending_windows` creates the window and
//!   resolves the promise. This is what makes opening a window from a click
//!   handler safe — there is no nested event-loop recursion.

mod handler;
mod shell_event;

use crate::{
    app::{
        handler::AppHandler,
        shell_event::JsShellEventHandler,
    },
    dom::doc::{NativeDoc, SharedDoc},
    global,
    helpers::JsWeakRef,
    renderer::CurrentRenderer,
    window::{
        NativeWindow, WindowState, make_window_document,
        monitor::{MonitorInfo, monitor_to_info},
        options::WindowOptions,
        util::build_window_attributes,
    },
};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    sync::mpsc::Receiver,
    time::Duration,
};

use blitz::{
    shell::{
        BlitzShellEvent, BlitzShellProxy, EventLoop, View, WindowConfig, create_default_event_loop,
    },
    traits::shell::DummyShellProvider,
};
use napi::{
    Env, Error, JsDeferred, JsValue, Result,
    bindgen_prelude::{Object, PromiseRaw, Undefined},
};
use winit::{
    event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    window::WindowId,
};

/// Result of one `pumpAppEvents` call.
#[napi(object)]
pub struct PumpResult {
    /// The loop is still running. Caller should pump again later.
    pub r#continue: bool,
    /// The loop has exited (e.g. all windows closed).
    pub exit: bool,
    /// Exit code, if `exit`.
    pub code: Option<i32>,
}

#[napi]
pub struct NativeApp {
    pub(super) event_loop: RefCell<EventLoop>,
    pub(crate) state: Rc<RefCell<AppState>>,
}

/// A live window: the blitz `View` plus the JS-side `Window` handle
/// that holds an `Arc<dyn Window>`. Dropping the view alone does not
/// release the winit window if the JS `Window` still holds a clone,
/// so `WindowEntry::close` takes the Arc out before dropping the view.
///
/// `view` is `Rc<RefCell<...>>` so that `AppHandler::window_event` can clone
/// the Rc, drop the `AppState` borrow, and only then call `handle_winit_event`
/// (which re-enters JS). Re-entrant JS that calls back into `NativeApp`
/// methods never sees an outstanding `AppState` borrow, and the event
/// dispatch only ever mutably borrows its own window's view.
pub(crate) struct WindowEntry {
    pub(crate) view: Rc<RefCell<View<CurrentRenderer>>>,
    pub(crate) state: Rc<RefCell<WindowState>>,
    /// Shared doc, for dispatching shell events without downcasting
    /// `view.doc` (a `Box<dyn Document>`).
    pub(crate) shared_doc: Rc<SharedDoc>,
}

impl WindowEntry {
    fn close(&mut self) {
        let mut state = self.state.borrow_mut();
        state.window = None;
        state.closed = true;
        drop(state);
        self.view
            .borrow_mut()
            .doc
            .inner_mut()
            .set_shell_provider(Arc::new(DummyShellProvider));
    }
}

/// One deferred operation that runs at the next pump. `Open` and `Close`
/// share a single queue keyed by *processing time* (next pump) rather than
/// one queue per operation, keeping the request path uniform.
pub(crate) enum PendingRequest {
    /// Promote a window config to a live `View` (needs the `ActiveEventLoop`
    /// a pump frame provides). Resolving `deferred` fulfils the JS-side
    /// `Promise` returned by `openWindow`.
    Open {
        config: WindowConfig<CurrentRenderer>,
        /// Bare `WindowState` — while pending, this is the *only* owner (the
        /// `NativeWindow` can't be built until the OS window id exists). It's
        /// wrapped in `Rc<RefCell>` at promotion time, when it becomes shared
        /// between the `NativeWindow` and the `WindowEntry`.
        state: WindowState,
        /// Shared doc, so the promoted `WindowEntry` can dispatch shell
        /// events to the JS `Window` object.
        shared_doc: Rc<SharedDoc>,
        deferred: JsDeferred<NativeWindow, Box<dyn FnOnce(Env) -> Result<NativeWindow>>>,
    },
    /// Tear down a requested closure (deferred past in-flight winit dispatch
    /// so `window.close()` is safe from inside a click handler). Resolving
    /// `deferred` fulfils the `Promise` `close_window` returned to JS.
    Close {
        window_id: WindowId,
        deferred: JsDeferred<Undefined, Box<dyn FnOnce(Env) -> Result<Undefined>>>,
    },
}

pub(crate) struct AppState {
    /// Live windows keyed by winit `WindowId`.
    pub(crate) windows: HashMap<WindowId, WindowEntry>,
    /// Requests queued for the next pump: promote a pending config to a live
    /// `View` (`Open` — needs the `ActiveEventLoop` a pump frame provides) or
    /// tear down a requested closure (`Close` — deferred past in-flight winit
    /// dispatch so `window.close()` is safe from inside a click handler).
    pub(crate) pending_requests: Vec<PendingRequest>,
    /// Proxy for sending events into the event loop (redraw, poll, etc.).
    pub(crate) proxy: BlitzShellProxy,
    /// Receiver for `BlitzShellEvent`s from the proxy channel.
    pub(crate) event_queue: Receiver<BlitzShellEvent>,
    /// Weak ref to the JS `BlitzApp` object, for dispatching app-level
    /// lifecycle events (`window:open`, `window:close`, `window:closed`).
    /// Set lazily by `set_app_ref`; absent until JS opts in.
    pub(crate) js_app_ref: Rc<RefCell<Option<JsWeakRef>>>,
    /// Number of windows currently considered "alive". Incremented
    /// on `openWindow`, decremented in the `close_window` path when we
    /// successfully remove a window from `windows` and in the native
    /// `CloseRequested` path via `JsAppHandler::outstanding`.
    pub(crate) outstanding_windows: usize,
    /// True once at least one window has ever been opened. Without
    /// this, calling `pump_app_events` before any `open_window` would
    /// wrongly synthesise an exit on the very first pump.
    pub(crate) has_opened_window: bool,
}

#[napi]
impl NativeApp {
    /// Build the winit event loop.
    #[napi(factory)]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        Self {
            event_loop: RefCell::new(event_loop),
            state: Rc::new(RefCell::new(AppState {
                windows: HashMap::new(),
                pending_requests: Vec::new(),
                proxy,
                event_queue: receiver,
                js_app_ref: Rc::new(RefCell::new(None)),
                outstanding_windows: 0,
                has_opened_window: false,
            })),
        }
    }

    /// Store a weak ref to the JS `BlitzApp` object so Rust can
    /// dispatch app-level lifecycle events (`window:open`,
    /// `window:close`, `window:closed`) to it. Mirrors
    /// `NativeDoc::set_window_ref`.
    #[napi]
    pub fn set_app_ref(&self, env: Env, app: Object) -> Result<()> {
        *self.state.borrow_mut().js_app_ref.borrow_mut() = Some(JsWeakRef::new(&app, &env)?);
        Ok(())
    }

    /// Attach a new window to the given document handle. The same handle can
    /// only be attached to one window. The JS DocHandle keeps working after
    /// this call (it shares state with the window via Rc<RefCell<...>>), so
    /// JS can keep mutating the DOM after `openWindow`.
    ///
    /// `options` maps directly to a winit `WindowAttributes`. If the document
    /// carries a `<title>` element, blitz's mutator-flush will overwrite the
    /// title shortly after open; this is expected, with the document treated
    /// as the source of truth for window-title content.
    ///
    /// Returns a `Promise<NativeWindow>`. This method never drives the event
    /// loop itself: it only queues the window request and returns a promise
    /// that resolves once a `pump_app_events` call promotes the pending config
    /// to a live window (see `AppHandler::drain_pending_windows`). That makes
    /// it safe to invoke from inside an event handler — the in-flight pump
    /// creates the window, with no nested event-loop recursion.
    ///
    /// Because creation is deferred to the caller's pump, the caller must
    /// ensure a pump is (or will be) running before `await`-ing the result;
    /// otherwise the promise never resolves. Typical setup drives at least one
    /// `pump_app_events` before awaiting.
    #[napi]
    pub fn open_window(
        &self,
        env: Env,
        doc: &mut NativeDoc,
        options: Option<&WindowOptions>,
    ) -> Result<PromiseRaw<'_, NativeWindow>> {
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
        let (deferred, promise_obj) = env
            .create_deferred::<NativeWindow, Box<dyn FnOnce(Env) -> Result<NativeWindow>>>()?;
        let promise = PromiseRaw::new(env.raw(), JsValue::raw(&promise_obj));

        {
            let mut app_state = self.state.borrow_mut();
            app_state
                .pending_requests
                .push(PendingRequest::Open {
                    config,
                    state: win_state,
                    shared_doc,
                    deferred,
                });
            app_state.has_opened_window = true;
            app_state.outstanding_windows += 1;
        }

        Ok(promise)
    }

    /// Queue the given window for closure and return a promise that resolves
    /// once the native `View` has actually been torn down (during the next
    /// pump, in `flush_closing_windows`), or rejects if a `close` listener
    /// calls `preventDefault()`.
    ///
    /// The cancelable `close` event is dispatched here, from Rust, at the
    /// moment the close is requested. If a listener prevents the default,
    /// the window stays open and the promise rejects. Otherwise the JS-side
    /// `closed` flag is set immediately — only the physical teardown is
    /// async, mirroring `open_window`'s create-then-resolve.
    ///
    /// This is intentionally not GC-driven: dropping the JS `Window` object
    /// does not close the OS window. Callers must invoke this explicitly.
    #[napi]
    pub fn close_window(
        &self,
        env: Env,
        window: &NativeWindow,
    ) -> Result<PromiseRaw<'_, Undefined>> {
        let window_id = window.window_id;

        // Public JS API guarantee: close() is idempotent. Multiple calls are
        // common when listeners race with UI state updates, so only the first
        // one queues a teardown; later ones resolve immediately.
        {
            let state = window.state.borrow();
            let app_state = self.state.borrow();
            if state.closed
                || app_state
                    .pending_requests
                    .iter()
                    .any(|req| matches!(req, PendingRequest::Close { window_id: id, .. } if *id == window_id))
            {
                drop(state);
                drop(app_state);
                return PromiseRaw::resolve(&env, ());
            }
        }

        // Never became a live window (still pending): just mark closed and
        // resolve immediately — there is no `View` to tear down and no
        // window-level `close` event to dispatch.
        let was_initialised = self.state.borrow().windows.contains_key(&window_id);
        if !was_initialised {
            let mut state = window.state.borrow_mut();
            state.closed = true;
            state.window = None;
            drop(state);
            return PromiseRaw::resolve(&env, ());
        }

        // Dispatch the cancelable `close` event to the window from Rust.
        // Clone the dispatch pieces first so no AppState borrow is held
        // across the re-entrant JS call.
        let (shared_doc, app_ref) = {
            let state = self.state.borrow();
            let entry = state.windows.get(&window_id).expect("checked above");
            (Rc::clone(&entry.shared_doc), Rc::clone(&state.js_app_ref))
        };
        let handler = JsShellEventHandler::new(app_ref);
        if !handler.close_request(&shared_doc, &env) {
            // A `close` listener on the window or a `window:close`
            // listener on the app prevented the close: the window stays
            // open and the caller's promise rejects.
            let (deferred, promise_obj) = env
                .create_deferred::<Undefined, Box<dyn FnOnce(Env) -> Result<Undefined>>>()?;
            let promise = PromiseRaw::new(env.raw(), JsValue::raw(&promise_obj));
            deferred.reject(Error::from_reason("close prevented"));
            return Ok(promise);
        }

        let (deferred, promise_obj) = env
            .create_deferred::<Undefined, Box<dyn FnOnce(Env) -> Result<Undefined>>>()?;
        let promise = PromiseRaw::new(env.raw(), JsValue::raw(&promise_obj));

        {
            let mut state = window.state.borrow_mut();
            state.closed = true;
            state.window = None;
        }
        {
            let mut app_state = self.state.borrow_mut();
            app_state
                .pending_requests
                .push(PendingRequest::Close { window_id, deferred });
            app_state.outstanding_windows = app_state.outstanding_windows.saturating_sub(1);
        }

        Ok(promise)
    }

    // -- Per-window runtime configuration -----------------------------------
    //
    // The napi `Window` handle does not own a reference to the live winit
    // `Arc<dyn Window>`; the `BlitzApplication` does. So all per-window
    // setters/getters live on `BlitzApp` and look the view up by window_id.
    // The JS-side `Window` class delegates through these.

    /// List all available monitors with full metadata. Returns `[]` if
    /// no windows have been created yet.
    #[napi]
    pub fn available_monitors(&self) -> Vec<MonitorInfo> {
        let state = self.state.borrow();
        let Some(entry) = state.windows.values().next() else {
            return Vec::new();
        };
        entry
            .view
            .borrow()
            .window
            .available_monitors()
            .map(monitor_to_info)
            .collect()
    }

    /// The primary monitor. Returns `None` if no windows have been
    /// created yet.
    #[napi]
    pub fn primary_monitor(&self) -> Option<MonitorInfo> {
        let state = self.state.borrow();
        let entry = state.windows.values().next()?;
        entry.view.borrow().window.primary_monitor().map(monitor_to_info)
    }

    /// Pump pending winit events for at most `millis` milliseconds.
    #[napi]
    pub fn pump_app_events(&self, millis: f64) -> PumpResult {
        self.pump_app_events_inner(millis)
    }

    /// Set the document zoom level. `1.0` is unzoomed. Combined with the
    /// system scale factor to produce the total viewport scale
    /// (`hidpi_scale * zoom`) that scales layout and CSS transforms.
    #[napi]
    pub fn set_zoom(&self, window: &NativeWindow, zoom: f64) -> Result<()> {
        let state = self.state.borrow();
        let entry = state
            .windows
            .get(&window.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        entry
            .view
            .borrow_mut()
            .with_viewport(|v| v.set_zoom(zoom as f32));
        Ok(())
    }

    /// Get the current document zoom level.
    #[napi]
    pub fn get_zoom(&self, window: &NativeWindow) -> Result<f32> {
        let state = self.state.borrow();
        let entry = state
            .windows
            .get(&window.window_id)
            .ok_or_else(|| Error::from_reason("window not found"))?;
        Ok(entry.view.borrow().doc.inner().viewport().zoom())
    }
}

impl NativeApp {
    fn poll_live_views(&self) {
        let state = self.state.borrow();
        for entry in state.windows.values() {
            entry.view.borrow_mut().poll();
        }
    }

    fn flush_closing_windows(&self) {
        let closing = {
            let mut state = self.state.borrow_mut();
            if state.pending_requests.is_empty() {
                return;
            }
            let all = std::mem::take(&mut state.pending_requests);
            let (closing, remaining): (Vec<_>, Vec<_>) = all
                .into_iter()
                .partition(|req| matches!(req, PendingRequest::Close { .. }));
            state.pending_requests.extend(remaining);
            closing
        };

        for req in closing {
            let PendingRequest::Close { window_id, deferred } = req else {
                unreachable!()
            };

            // 1. Remove + close the entry (needs &mut AppState), cloning the
            //    dispatch pieces so no borrow is held across the JS dispatch.
            let (shared_doc, app_ref) = {
                let mut state = self.state.borrow_mut();
                match state.windows.remove(&window_id) {
                    Some(mut entry) => {
                        entry.close();
                        let doc = Some(Rc::clone(&entry.shared_doc));
                        drop(entry);
                        (doc, Rc::clone(&state.js_app_ref))
                    }
                    None => (None, Rc::clone(&state.js_app_ref)),
                }
            };

            // 2. Notify from Rust: `window:closed` on the window, propagated
            //    up to the app. No outstanding AppState borrow, so JS
            //    re-entry into `open_window` / `close_window` is safe.
            if let Some(shared_doc) = shared_doc {
                let env = match global::env() {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("napi-blitz: flush_closing_windows: env not available: {e}");
                        return;
                    }
                };
                let handler = JsShellEventHandler::new(app_ref);
                // The cancelable close request (`window:close`) was already
                // dispatched by `close_window`; here only the post-teardown
                // notification remains, propagated window → app.
                handler.notify_closed(&shared_doc, &env);
            }

            // 3. Fulfil the `close_window` promise after the notifications,
            //    so JS-side await sees the teardown fully complete.
            deferred.resolve(Box::new(move |_env| Ok(())));
        }
    }

    /// Pump pending winit events for at most `millis` milliseconds. JS should
    /// call this in a loop (typically once per animation frame) to drive the
    /// renderer and event handling.
    fn pump_app_events_inner(&self, millis: f64) -> PumpResult {
        // Give host-driven DOM mutations from the previous JS turn a chance to
        // flow through Blitz's normal `View::poll -> Document::poll ->
        // request_redraw` path before winit waits for more events.
        self.poll_live_views();

        // Pending windows are promoted to live Views by `AppHandler::drain_pending_windows`
        // during the pump. No need to hand them to an intermediate application layer.

        // A caller may invoke `window.close()` between pump ticks. In that
        // case no winit/blitz document dispatch is active, so it is safe and
        // necessary to drop the queued views before the synthetic-exit check
        // below observes `outstanding_windows == 0`.
        self.flush_closing_windows();

        // If at least one window has ever been opened and every
        // window has now been closed via JS, surface a synthetic
        // Exit. winit's `pump_app_events` mode never exits on its
        // own; the OS-initiated `CloseRequested` path already
        // triggers `event_loop.exit()` from inside
        // `BlitzApplication::window_event`, but JS-initiated
        // `BlitzApp::close_window` bypasses winit's pipeline entirely.
        let (has_opened, outstanding) = {
            let state = self.state.borrow();
            (state.has_opened_window, state.outstanding_windows)
        };
        if has_opened && outstanding == 0 {
            return PumpResult {
                r#continue: false,
                exit: true,
                code: Some(0),
            };
        }

        let timeout = Some(Duration::from_millis(millis.max(0.0).round() as u64));

        let mut handler = AppHandler {
            state: Rc::clone(&self.state),
        };
        let status = self.event_loop.borrow_mut().pump_app_events(timeout, &mut handler);
        self.flush_closing_windows();
        self.poll_live_views();

        match status {
            PumpStatus::Continue => PumpResult {
                r#continue: true,
                exit: false,
                code: None,
            },
            PumpStatus::Exit(code) => PumpResult {
                r#continue: false,
                exit: true,
                code: Some(code),
            },
        }
    }
}
