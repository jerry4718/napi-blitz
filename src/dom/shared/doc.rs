//! Per-document shared state, the blitz↔JS document adapter, and node
//! wrapping.
//!
//! `SharedDocument` holds the `BaseDocument`, the switchable NodeCache, weak
//! refs to the JS Document/Window, and the napi env captured at creation.
//! `wrap_node` materializes a JS wrapper for a blitz node id by building
//! the matching `#[layer]` chain (`new_from_chain`) and caching it; the
//! GC finalizer weak-references `SharedDocument`.

// 4. Build the node chain by blitz node type + tag name.
use crate::dom::{
    DocumentLayer, dispatch::JsEventHandler, shared::node_cache::NodeCache, wrap_node,
};
use blitz::{
    dom::{
        BULLET_FONT, BaseDocument, DEFAULT_CSS, DocGuard, DocGuardMut, Document as BlitzDocument,
        DocumentConfig, EventDriver, FontContext, NodeId,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::UiEvent,
};
use fontique::Blob;
use napi::{Env, Result, bindgen_prelude::Object};
use napi_derive::napi;
use napi_helpers::{JsWeakRef, inherits::with_own};
use std::{
    cell::{Cell, Ref, RefCell, RefMut},
    rc::Rc,
    sync::Arc,
    task::Context as TaskContext,
};

const DEFAULT_HTML: &str = "<!DOCTYPE html><html><head></head><body></body></html>";

/// Configuration passed to `createDocument`.
#[napi(object)]
pub struct DocHandleConfig {
    pub ua_stylesheets: Option<Vec<String>>,
    pub base_html: Option<String>,
}

// ── SharedDocument: per-document shared state ─────────────────────────────

/// Per-document shared state. Held inside `Rc` so that the window adapter
/// and node wrappers can share it. The GC finalizer uses `Weak`.
pub struct SharedDocument {
    /// The document tree.
    base: RefCell<BaseDocument>,
    /// Switchable-reference cache: blitz_node_id -> SwitchableRef.
    /// In-document nodes are strong (prevent GC); detached nodes are weak.
    node_cache: RefCell<NodeCache>,
    /// Host-dirty flag: JS mutated the DOM, window needs redraw.
    host_dirty: Cell<bool>,
    /// Weak ref to the JS Document object
    js_weak: RefCell<Option<JsWeakRef>>,
    /// Weak ref to the JS Window object, for lifecycle dispatch.
    js_window_ref: RefCell<Option<JsWeakRef>>,
    /// The napi env captured at document creation; blitz's
    /// `EventHandler` callbacks do not carry an `Env`.
    env: Cell<Option<Env>>,
    /// Whether the document has already been attached to a window.
    /// One document attaches to at most one window.
    attached: Cell<bool>,
}

impl SharedDocument {
    pub fn new(base: BaseDocument) -> Self {
        Self {
            base: RefCell::new(base),
            host_dirty: Cell::new(false),
            node_cache: RefCell::new(NodeCache::new()),
            js_weak: RefCell::new(None),
            js_window_ref: RefCell::new(None),
            env: Cell::new(None),
            attached: Cell::new(false),
        }
    }

    /// Claim the document for a window attach. `false` when it is already
    /// attached; the same document can only back one window.
    pub fn mark_attached(&self) -> bool {
        if self.attached.get() {
            false
        } else {
            self.attached.set(true);
            true
        }
    }

    pub fn base(&self) -> Ref<'_, BaseDocument> {
        self.base.borrow()
    }

    pub fn base_mut(&self) -> RefMut<'_, BaseDocument> {
        self.base.borrow_mut()
    }

    pub fn node_cache(&self) -> Ref<'_, NodeCache> {
        self.node_cache.borrow()
    }

    pub fn node_cache_mut(&self) -> RefMut<'_, NodeCache> {
        self.node_cache.borrow_mut()
    }

    pub fn mark_host_dirty(&self) {
        self.host_dirty.set(true);
    }

    pub fn take_host_dirty(&self) -> bool {
        self.host_dirty.replace(false)
    }

    pub fn set_env(&self, env: Env) {
        self.env.set(Some(env));
    }

    pub fn env(&self) -> Option<Env> {
        self.env.get()
    }

    /// Register the JS Document object, retained weakly.
    pub fn set_js_weak(&self, env: &Env, document: &Object) -> Result<()> {
        *self.js_weak.borrow_mut() = Some(JsWeakRef::new(document, env)?);
        Ok(())
    }

    /// Register the JS Document object, retained weakly.
    pub fn js_weak(&self) -> Ref<'_, Option<JsWeakRef>> {
        self.js_weak.borrow()
    }

    /// Register the JS Window object, retained weakly; the lifecycle
    /// dispatch resolves the window through this.
    pub fn set_window_ref(&self, env: &Env, window: &Object) -> Result<()> {
        *self.js_window_ref.borrow_mut() = Some(JsWeakRef::new(window, env)?);
        Ok(())
    }

    /// Read the JS Window object's weak ref.
    pub fn js_window_ref(&self) -> Ref<'_, Option<JsWeakRef>> {
        self.js_window_ref.borrow()
    }
}

impl SharedDocument {
    // ── Reference switching ───────────────────────────────────────────

    /// Check if a node is in the document using the blitz internal flag.
    pub fn is_in_document(&self, node_id: NodeId) -> bool {
        self.base()
            .get_node(node_id)
            .is_some_and(|node| node.flags.is_in_document())
    }

    /// Recursively switch all cached nodes in a subtree to strong refs.
    ///
    /// **Caller must ensure `node_id` is in the document.**
    fn make_subtree_strong(&self, node_id: NodeId, env: &Env) -> Result<()> {
        self.node_cache_mut().make_strong(node_id, env)?;
        let child_ids: Vec<NodeId> = self
            .base()
            .get_node(node_id)
            .map(|n| n.children.to_vec())
            .unwrap_or_default();
        for child_id in child_ids {
            self.make_subtree_strong(child_id, env)?;
        }
        Ok(())
    }

    /// Recursively switch all cached nodes in a subtree to weak refs.
    fn make_subtree_weak(&self, node_id: NodeId, env: &Env) -> Result<()> {
        self.node_cache_mut().make_weak(node_id, env)?;
        let child_ids: Vec<NodeId> = self
            .base()
            .get_node(node_id)
            .map(|n| n.children.to_vec())
            .unwrap_or_default();
        for child_id in child_ids {
            self.make_subtree_weak(child_id, env)?;
        }
        Ok(())
    }

    /// Switch a subtree to strong refs if the parent is in the document.
    pub fn make_in_document_subtree_strong(
        &self,
        parent_id: NodeId,
        child_id: NodeId,
        env: &Env,
    ) -> Result<()> {
        if self.is_in_document(parent_id) {
            self.make_subtree_strong(child_id, env)?;
        }
        Ok(())
    }

    /// Switch a subtree to weak refs if the node is in the document.
    /// If the node is already detached, no-op.
    ///
    /// **Must be called before `remove_node`**, while the node still has its
    /// parent chain so `is_in_document` can be evaluated.
    pub fn make_in_document_subtree_weak(&self, node_id: NodeId, env: &Env) -> Result<()> {
        if self.is_in_document(node_id) {
            self.make_subtree_weak(node_id, env)?;
        }
        Ok(())
    }

    /// Collect, weaken, and detach all children of `node_id`.
    pub fn detach_children(&self, node_id: NodeId, env: &Env) -> Result<()> {
        let children: Vec<NodeId> = {
            let base = self.base();
            base.get_node(node_id)
                .map(|n| n.children.iter().copied().collect())
                .unwrap_or_default()
        };
        for child_id in &children {
            self.make_in_document_subtree_weak(*child_id, env)?;
        }
        let mut base = self.base_mut();
        let mut mutator = base.mutate();
        for child_id in &children {
            mutator.remove_node(*child_id);
        }
        drop(mutator);
        drop(base);
        Ok(())
    }

    /// Pre-order DFS over the document tree, starting from the given node
    /// (inclusive). `pred` decides which node ids are collected.
    pub fn dfs<F>(&self, root: NodeId, pred: F) -> Vec<NodeId>
    where
        F: Fn(&blitz::dom::Node) -> bool,
    {
        let state = self.base();
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = state.get_node(id) else {
                continue;
            };
            if pred(node) {
                out.push(id);
            }
            for &child in node.children.iter().rev() {
                stack.push(child);
            }
        }
        out
    }

    pub fn find_first<F>(&self, pred: F) -> Option<NodeId>
    where
        F: Fn(&blitz::dom::Node) -> bool,
    {
        let state = self.base();
        let mut stack: Vec<NodeId> = vec![state.root_node().id];
        while let Some(id) = stack.pop() {
            let node = state.get_node(id)?;
            if pred(node) {
                return Some(id);
            }
            for &child in node.children.iter().rev() {
                stack.push(child);
            }
        }
        None
    }
}

// ── WindowDocument: blitz Document adapter ────────────────────────────

pub struct WindowDocument {
    pub shared_doc: Rc<SharedDocument>,
}

impl WindowDocument {
    pub fn new(doc: Rc<SharedDocument>) -> Self {
        Self { shared_doc: doc }
    }
}

impl BlitzDocument for WindowDocument {
    fn inner(&self) -> DocGuard<'_> {
        let borrow = self.shared_doc.base();
        DocGuard::RefCell(borrow)
    }

    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        let borrow = self.shared_doc.base_mut();
        DocGuardMut::RefCell(borrow)
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        let handler = JsEventHandler {
            doc: Rc::downgrade(&self.shared_doc),
        };
        let mut driver = EventDriver::new(self, handler);
        driver.handle_ui_event(event);
    }

    fn poll(&mut self, _task_context: Option<TaskContext>) -> bool {
        self.shared_doc.take_host_dirty()
    }

    fn id(&self) -> usize {
        self.shared_doc.base().id()
    }
}

// ── Document creation ─────────────────────────────────────────────────

/// Create a new document from Rust, populate it with the default HTML,
/// and return the JS Document object (an `HTMLDocument` layer chain).
#[napi]
pub fn create_document<'env>(
    env: &'env Env,
    config: Option<DocHandleConfig>,
) -> Result<Object<'env>> {
    let mut font_ctx = FontContext::new();
    font_ctx
        .collection
        .register_fonts(Blob::new(Arc::new(BULLET_FONT) as _), None);
    font_ctx.collection.make_shared();
    font_ctx.source_cache.make_shared();

    let ua_stylesheets = config
        .as_ref()
        .and_then(|c| c.ua_stylesheets.clone())
        .unwrap_or_else(|| vec![DEFAULT_CSS.to_string()]);
    let base_html = config
        .as_ref()
        .and_then(|c| c.base_html.clone())
        .unwrap_or_else(|| DEFAULT_HTML.to_string());

    let doc_config = DocumentConfig {
        html_parser_provider: Some(Arc::new(HtmlProvider) as _),
        ua_stylesheets: Some(ua_stylesheets),
        font_ctx: Some(font_ctx),
        ..DocumentConfig::default()
    };

    let mut base = BaseDocument::new(doc_config);
    {
        let mut mutator = base.mutate();
        DocumentHtmlParser::parse_into_mutator(&mut mutator, &base_html);
    }
    base.resolve(0.0);

    let shared_doc = Rc::new(SharedDocument::new(base));
    shared_doc.set_env(env.clone());

    let node_id = shared_doc.base().root_node().id;
    wrap_node(&shared_doc, &env, node_id)
}

/// Register the JS `Window` object for the document, retained weakly, so
/// the Rust side can dispatch lifecycle events to it. Called from the JS
/// `Window` constructor.
#[napi]
pub fn set_window_ref(env: Env, doc: Object, window: Object) -> Result<()> {
    let shared_doc = with_own::<DocumentLayer, _>(&doc, |d| d.shared.clone())?;
    shared_doc.set_window_ref(&env, &window)
}
