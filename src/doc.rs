//! `DocHandle`: the JS-facing handle to a blitz `BaseDocument`.
//!
//! The handle owns an `Rc<RefCell<BaseDocument>>` plus the JS dispatch bridge.
//! When the document is moved into a window we wrap a clone of that `Rc` in a
//! [`WindowDocument`] (which implements blitz's `Document` trait) and hand
//! ownership of the `Box<dyn Document>` to the window. The `Rc` keeps the
//! same `BaseDocument` accessible from both sides, and the bridge state still
//! lives on the `DocHandle`.
//!
//! All DOM mutation methods on `DocHandle` are flat in JS: they take node ids
//! (numbers) as arguments. Element classes live in the JS layer.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    task::Context as TaskContext,
};

use blitz::{
    dom::{
        BaseDocument, BULLET_FONT, DEFAULT_CSS, DocGuard, DocGuardMut, Document as BlitzDocument,
        DocumentConfig, EventDriver, FontContext,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::UiEvent,
};
use napi::{
    Env, Result,
    bindgen_prelude::{Function, FunctionRef},
};
use napi_derive::napi;
use parley::fontique::Blob;

use crate::event::{JsBridge, JsEventHandler};
use crate::payload::{DispatchResult, EventPayload};

const DEFAULT_HTML: &str = "<!DOCTYPE html><html><head></head><body></body></html>";

/// Configuration passed to `DocHandle.create`.
#[napi(object)]
pub struct DocHandleConfig<'env> {
    /// Optional UA stylesheets. Defaults to blitz's DEFAULT_CSS.
    pub ua_stylesheets: Option<Vec<String>>,
    /// Optional initial HTML. Defaults to a blank document.
    pub base_html: Option<String>,
    /// Required: a JS callback `(payload: EventPayload) => DispatchResult` that
    /// the Rust side calls every time blitz produces a DomEvent. The JS layer
    /// uses it to drive its own `EventTarget` chain.
    pub on_dispatch: Function<'env, EventPayload, DispatchResult>,
}

/// The shared state between [`DocHandle`] (JS-owned) and [`WindowDocument`]
/// (window-owned, when a window is opened on this document).
pub struct DocState {
    pub base: BaseDocument,
    pub bridge: JsBridge,
}

/// Newtype for `Rc<RefCell<DocState>>`. Both `DocHandle` and `WindowDocument`
/// hold one of these, sharing the same underlying state.
#[derive(Clone)]
pub struct SharedDocState(pub Rc<RefCell<DocState>>);

impl SharedDocState {
    pub fn new(state: DocState) -> Self {
        Self(Rc::new(RefCell::new(state)))
    }
}

/// Adapter that implements blitz's `Document` trait around our shared state.
/// `WindowConfig::new` takes `Box<dyn Document>`, and the window owns that box
/// for its lifetime; we keep the same data accessible from JS by sharing the
/// `Rc<RefCell<...>>`.
pub struct WindowDocument {
    pub state: SharedDocState,
}

impl WindowDocument {
    pub fn new(state: SharedDocState) -> Self {
        Self { state }
    }
}

impl BlitzDocument for WindowDocument {
    fn inner(&self) -> DocGuard<'_> {
        // We can't return a `RefCell` guard directly without leaking `Ref`s,
        // so we use the `RefCell` variant of `DocGuard`. We borrow the
        // outer `RefCell<DocState>`, then project to the `BaseDocument`.
        // `DocGuard::RefCell` expects `Ref<'_, BaseDocument>`; we use
        // `Ref::map` to project from `DocState` to `BaseDocument`.
        let borrow = self.state.0.borrow();
        let projected = std::cell::Ref::map(borrow, |s| &s.base);
        DocGuard::RefCell(projected)
    }

    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        let borrow = self.state.0.borrow_mut();
        let projected = std::cell::RefMut::map(borrow, |s| &mut s.base);
        DocGuardMut::RefCell(projected)
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        // We do the dispatch in two phases so we don't collide on the
        // RefCell: first take a mut borrow of state to set up the driver,
        // then run the driver. Since `JsEventHandler` only borrows
        // `bridge` (and not `base`), and the driver only borrows `base`
        // through the `Document` trait, we have to split them carefully.
        //
        // Strategy: call into base via `DocGuardMut`, but pass the bridge
        // separately. We achieve this by using a scoped block.
        let mut state = self.state.0.borrow_mut();
        let DocState { base, bridge } = &mut *state;

        let handler = JsEventHandler { bridge };
        let mut driver = EventDriver::new(base, handler);
        driver.handle_ui_event(event);
    }

    fn poll(&mut self, _task_context: Option<TaskContext>) -> bool {
        false
    }

    fn id(&self) -> usize {
        self.state.0.borrow().base.id()
    }
}

/// JS-facing handle. Holds the shared document state and exposes the flat
/// nodeId-based DOM API.
#[napi]
pub struct DocHandle {
    pub(crate) state: SharedDocState,
    /// Whether ownership of the document has been moved into a window.
    /// After this we still keep the `Rc` so the JS side can keep mutating
    /// the DOM, but we refuse to attach it to a second window.
    pub(crate) moved_into_window: bool,
}

impl DocHandle {
    pub(crate) fn share_state(&self) -> SharedDocState {
        self.state.clone()
    }
}

#[napi]
impl DocHandle {
    /// Create a new document.
    #[napi(factory)]
    pub fn create(env: Env, config: DocHandleConfig<'_>) -> Result<Self> {
        // Register the bullet font for list-item bullets.
        let mut font_ctx = FontContext::new();
        font_ctx
            .collection
            .register_fonts(Blob::new(Arc::new(BULLET_FONT) as _), None);

        let ua_stylesheets = config
            .ua_stylesheets
            .unwrap_or_else(|| vec![DEFAULT_CSS.to_string()]);
        let base_html = config
            .base_html
            .unwrap_or_else(|| DEFAULT_HTML.to_string());

        let doc_config = DocumentConfig {
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ua_stylesheets: Some(ua_stylesheets),
            ..DocumentConfig::default()
        };

        let mut base = BaseDocument::new(doc_config);

        // Parse the initial HTML into the base document.
        {
            let mut mutator = base.mutate();
            DocumentHtmlParser::parse_into_mutator(&mut mutator, &base_html);
        }
        base.resolve(0.0);

        let callback_ref: FunctionRef<EventPayload, DispatchResult> = config
            .on_dispatch
            .create_ref()?;
        let bridge = JsBridge::new(env, callback_ref);

        let state = DocState { base, bridge };
        let shared = SharedDocState::new(state);

        Ok(Self {
            state: shared,
            moved_into_window: false,
        })
    }

    /// Mark this document as moved into a window. Internal use by `BlitzApp`.
    /// Returns `true` if it was a fresh attach, `false` if already attached.
    pub(crate) fn mark_attached(&mut self) -> bool {
        if self.moved_into_window {
            false
        } else {
            self.moved_into_window = true;
            true
        }
    }

    /// Recompute style + layout. Called from JS after batches of mutations or
    /// before painting. `time_ms` drives CSS animations.
    #[napi]
    pub fn resolve(&mut self, time_ms: f64) {
        self.state.0.borrow_mut().base.resolve(time_ms);
    }

    /// The id of the root node (always 0 for blitz, but expose it for JS).
    #[napi]
    pub fn root_node_id(&self) -> u32 {
        self.state.0.borrow().base.root_node().id as u32
    }

    /// The id of `<html>` (the root *element*).
    #[napi]
    pub fn root_element_id(&self) -> u32 {
        self.state.0.borrow().base.root_element().id as u32
    }

    /// Update the set of node ids JS currently has live wrappers for. Rust
    /// uses this to short-circuit dispatch when no listener could exist.
    #[napi]
    pub fn set_listened_nodes(&mut self, ids: Vec<u32>) {
        let mut state = self.state.0.borrow_mut();
        state.bridge.listened_nodes = ids.into_iter().collect();
    }

    /// Add a single node id to the listened set. Cheaper than calling
    /// `set_listened_nodes` for incremental subscription updates.
    #[napi]
    pub fn add_listened_node(&mut self, id: u32) {
        self.state.0.borrow_mut().bridge.listened_nodes.insert(id);
    }

    /// Remove a node id from the listened set.
    #[napi]
    pub fn remove_listened_node(&mut self, id: u32) {
        self.state.0.borrow_mut().bridge.listened_nodes.remove(&id);
    }
}

/// Internal helper: build a [`WindowDocument`] from a [`DocHandle`] without
/// transferring the underlying `Rc` away from the handle. The window will
/// receive `Box<WindowDocument>`; the handle keeps its own clone of the `Rc`.
pub(crate) fn make_window_document(handle: &DocHandle) -> Box<WindowDocument> {
    Box::new(WindowDocument::new(handle.share_state()))
}
