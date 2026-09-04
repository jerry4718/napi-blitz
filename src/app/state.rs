//! `AppState`: what exists right now - the live-window table and the
//! pending-request queue. Pure data: everything that *does* something
//! with these facts (opens, closes, teardowns, dispatches) lives in
//! `Lifecycle`.

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use blitz::{
    shell::{View, WindowConfig},
    traits::shell::DummyShellProvider,
};
use napi_helpers::deferred::Deferred;
use winit::window::WindowId;

use crate::{
    dom::shared::doc::SharedDocument,
    renderer::CurrentRenderer,
    window::{WindowState, raf::RafQueue},
};

/// A live window: the blitz `View` plus the JS-side `Window` handle
/// that holds an `Arc<dyn Window>`. Dropping the view alone does not
/// release the winit window if the JS `Window` still holds a clone,
/// so `WindowEntry::close` takes the Arc out before dropping the view.
///
/// `view` is `Rc<RefCell<...>>` so that `AppHandler::window_event` can clone
/// the Rc, drop the state borrow, and only then call `handle_winit_event`
/// (which re-enters JS). Re-entrant JS that calls back into `NativeApp`
/// methods never sees an outstanding state borrow, and the event
/// dispatch only ever mutably borrows its own window's view.
pub(crate) struct WindowEntry {
    pub(crate) view: Rc<RefCell<View<CurrentRenderer>>>,
    pub(crate) state: Rc<RefCell<WindowState>>,
    /// Shared doc, for dispatching shell events without downcasting
    /// `view.doc` (a `Box<dyn Document>`).
    pub(crate) shared_doc: Rc<SharedDocument>,
    /// Pending `requestAnimationFrame` callbacks for this window's next
    /// redraw frame. `Rc` so the event handler can clone the queue out,
    /// drop the state borrow, and only then run callbacks (which re-enter
    /// JS). `RafQueue` is itself interior-mutable (methods take `&self`),
    /// so no `RefCell` layer is needed here.
    pub(crate) raf: Rc<RafQueue>,
}

impl WindowEntry {
    pub(crate) fn close(&mut self) {
        // The window is gone; no frame will run the queued callbacks, so
        // release their strong references now (window close is the
        // mandatory cleanup point for queued animation callbacks).
        self.raf.clear();
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
        config: Box<WindowConfig<CurrentRenderer>>,
        /// Bare `WindowState` - while pending, this is the *only* owner (the
        /// `Window` layer can't be built until the OS window id exists). It's
        /// wrapped in `Rc<RefCell>` at promotion time, when it becomes shared
        /// between the `WindowLayer` and the `WindowEntry`.
        state: WindowState,
        /// Shared doc, so the promoted `WindowEntry` can dispatch shell
        /// events to the JS `Window` object.
        shared_doc: Rc<SharedDocument>,
        deferred: Deferred,
    },
    /// Tear down a requested closure (deferred past in-flight winit dispatch
    /// so `window.close()` is safe from inside a click handler). Resolving
    /// `deferred` fulfils the `Promise` `close_window` returned to JS.
    Close {
        window_id: WindowId,
        deferred: Deferred,
    },
}

pub(crate) struct AppState {
    /// Live windows keyed by winit `WindowId`.
    pub(crate) windows: HashMap<WindowId, WindowEntry>,
    /// Requests queued for the next pump: promote a pending config to a live
    /// `View` (`Open` - needs the `ActiveEventLoop` a pump frame provides) or
    /// tear down a requested closure (`Close` - deferred past in-flight winit
    /// dispatch so `window.close()` is safe from inside a click handler).
    pub(crate) pending_requests: Vec<PendingRequest>,
}
