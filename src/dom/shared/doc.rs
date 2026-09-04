//! Per-document shared state, the blitz↔JS document adapter, and node
//! wrapping.
//!
//! `SharedDoc` holds the `BaseDocument`, the switchable NodeCache, weak
//! refs to the JS Document/Window, and the napi env captured at creation.
//! `wrap_node` materializes a JS wrapper for a blitz node id by building
//! the matching `#[layer]` chain (`new_from_chain`) and caching it; the
//! GC finalizer weak-references `SharedDoc`.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    task::Context as TaskContext,
};

use crate::{
    dispatch::JsEventHandler,
    layers::{
        comment::CommentLayer,
        document::DocumentLayer,
        element::ElementLayer,
        html_document::HTMLDocumentLayer,
        html_element::HTMLElementLayer,
        html_input_element::HTMLInputElementLayer,
        html_textarea_element::HTMLTextAreaElementLayer,
        node::{NodeLayer, NodeState},
        text::TextLayer,
    },
    shared::node_cache::NodeCache,
};
use blitz::{
    dom::{
        BULLET_FONT, BaseDocument, DEFAULT_CSS, DocGuard, DocGuardMut, Document as BlitzDocument,
        DocumentConfig, EventDriver, FontContext, NodeData, NodeId, local_name, node::NodeKind,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::UiEvent,
};
use napi::{Env, Error, Result, Status, bindgen_prelude::Object};
use napi_derive::napi;
// 4. Build the node chain by blitz node type + tag name.
use napi_helpers::{
    JsWeakRef,
    inherits::{layer_chain, new_from_chain},
};
use parley::fontique::Blob;
use wintertc_events::event_target::EventTargetLayer;

const DEFAULT_HTML: &str = "<!DOCTYPE html><html><head></head><body></body></html>";

/// Configuration passed to `createDocument`.
#[napi(object)]
pub struct DocHandleConfig {
    pub ua_stylesheets: Option<Vec<String>>,
    pub base_html: Option<String>,
}

// ── SharedDoc: per-document shared state ─────────────────────────────

/// Per-document shared state. Held inside `Rc` so that the window adapter
/// and node wrappers can share it. The GC finalizer uses `Weak`.
pub struct SharedDoc {
    /// The document tree.
    pub base: RefCell<BaseDocument>,
    /// Host-dirty flag: JS mutated the DOM, window needs redraw.
    host_dirty: Cell<bool>,
    /// Switchable-reference cache: blitz_node_id -> SwitchableRef.
    /// In-document nodes are strong (prevent GC); detached nodes are weak.
    pub node_cache: RefCell<NodeCache>,
    /// Weak ref to the JS Document object
    pub js_document_ref: RefCell<Option<JsWeakRef>>,
    /// Weak ref to the JS Window object, for forwarding pointer events.
    pub js_window_ref: RefCell<Option<JsWeakRef>>,
    /// The napi env captured at document creation; blitz's
    /// `EventHandler` callbacks do not carry an `Env`.
    env: Cell<Option<Env>>,
}

impl SharedDoc {
    pub fn new(base: BaseDocument) -> Self {
        Self {
            base: RefCell::new(base),
            host_dirty: Cell::new(false),
            node_cache: RefCell::new(NodeCache::new()),
            js_document_ref: RefCell::new(None),
            js_window_ref: RefCell::new(None),
            env: Cell::new(None),
        }
    }

    pub fn set_env(&self, env: Env) {
        self.env.set(Some(env));
    }

    pub fn env(&self) -> Option<Env> {
        self.env.get()
    }

    pub fn mark_host_dirty(&self) {
        self.host_dirty.set(true);
    }

    pub fn take_host_dirty(&self) -> bool {
        self.host_dirty.replace(false)
    }

    // ── Reference switching ───────────────────────────────────────────

    /// Check if a node is in the document using the blitz internal flag.
    pub fn is_in_document(&self, node_id: NodeId) -> bool {
        self.base
            .borrow()
            .get_node(node_id)
            .is_some_and(|node| node.flags.is_in_document())
    }

    /// Recursively switch all cached nodes in a subtree to strong refs.
    ///
    /// **Caller must ensure `node_id` is in the document.**
    fn make_subtree_strong(&self, node_id: NodeId, env: &Env) -> Result<()> {
        self.node_cache.borrow_mut().make_strong(node_id, env)?;
        let child_ids: Vec<NodeId> = self
            .base
            .borrow()
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
        self.node_cache.borrow_mut().make_weak(node_id, env)?;
        let child_ids: Vec<NodeId> = self
            .base
            .borrow()
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
            let base = self.base.borrow();
            base.get_node(node_id)
                .map(|n| n.children.iter().copied().collect())
                .unwrap_or_default()
        };
        for child_id in &children {
            self.make_in_document_subtree_weak(*child_id, env)?;
        }
        let mut base = self.base.borrow_mut();
        let mut mutator = base.mutate();
        for child_id in &children {
            mutator.remove_node(*child_id);
        }
        drop(mutator);
        drop(base);
        Ok(())
    }

    /// Register the JS Document object, retained weakly.
    pub fn set_document_ref(&self, env: &Env, document: &Object) -> Result<()> {
        *self.js_document_ref.borrow_mut() = Some(JsWeakRef::new(document, env)?);
        Ok(())
    }
}

// ── wrap_node: materialize a JS wrapper for a blitz node ─────────────

/// Return the cached JS wrapper for `node_id`, or build the matching
/// `#[layer]` chain via `new_from_chain` and cache it.
///
/// Document nodes resolve to (and register) the JS Document object.
pub fn wrap_node<'a>(doc: &Rc<SharedDoc>, env: &'a Env, node_id: NodeId) -> Result<Object<'a>> {
    // 1. Return an existing JS wrapper only after confirming that the
    //    underlying DOM node still exists.
    if let Some(cached) = doc.node_cache.borrow().get(node_id, env) {
        return Ok(cached);
    }

    // 2. Read the node metadata. Invalid or stale ids must not fall
    //    through to chain building as a made-up nodeType.
    let (node_kind, qual_name) = {
        let base = doc.base.borrow();
        let node = base.get_node(node_id).ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                format!("No DOM node found for node_id={node_id}"),
            )
        })?;

        match &node.data {
            NodeData::Document(_) => (NodeKind::Document, None),
            NodeData::Element(el) => (NodeKind::Element, Some(el.name.clone())),
            NodeData::Text(_) => (NodeKind::Text, None),
            NodeData::Comment { .. } => (NodeKind::Comment, None),
            _ => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Unsupported DOM node type for node_id={node_id}"),
                ));
            }
        }
    };

    // 3. Document node: resolve to the JS Document object, creating and
    //    registering it on first access.
    if let NodeKind::Document = node_kind {
        if let Some(existing) = doc
            .js_document_ref
            .borrow()
            .as_ref()
            .and_then(|weak| weak.get_value(env))
        {
            doc.node_cache.borrow_mut().insert(
                node_id,
                &existing,
                env,
                true,
                Rc::downgrade(doc),
            )?;
            return Ok(existing);
        }
        let chain = layer_chain!(
            EventTargetLayer::fresh(),
            NodeLayer {
                node_id,
                doc: doc.clone(),
                state: NodeState::default(),
            },
            DocumentLayer { doc: doc.clone() },
            HTMLDocumentLayer {},
        );
        let document = new_from_chain::<HTMLDocumentLayer>(env, chain)?;
        doc.set_document_ref(env, &document)?;
        doc.node_cache
            .borrow_mut()
            .insert(node_id, &document, env, true, Rc::downgrade(doc))?;
        return Ok(document);
    }

    let js_node = match node_kind {
        NodeKind::Element => match qual_name.map(|qn| qn.local) {
            Some(local_name!("input")) => new_from_chain::<HTMLInputElementLayer>(
                env,
                layer_chain!(
                    EventTargetLayer::fresh(),
                    NodeLayer {
                        node_id,
                        doc: doc.clone(),
                        state: NodeState::default(),
                    },
                    ElementLayer {
                        node_id,
                        doc: doc.clone(),
                        state: crate::layers::element::ElementState::default(),
                    },
                    HTMLElementLayer {},
                    HTMLInputElementLayer {
                        node_id,
                        doc: doc.clone(),
                    },
                ),
            )?,
            Some(local_name!("textarea")) => new_from_chain::<HTMLTextAreaElementLayer>(
                env,
                layer_chain!(
                    EventTargetLayer::fresh(),
                    NodeLayer {
                        node_id,
                        doc: doc.clone(),
                        state: NodeState::default(),
                    },
                    ElementLayer {
                        node_id,
                        doc: doc.clone(),
                        state: crate::layers::element::ElementState::default(),
                    },
                    HTMLElementLayer {},
                    HTMLTextAreaElementLayer {
                        node_id,
                        doc: doc.clone(),
                    },
                ),
            )?,
            _ => new_from_chain::<HTMLElementLayer>(
                env,
                layer_chain!(
                    EventTargetLayer::fresh(),
                    NodeLayer {
                        node_id,
                        doc: doc.clone(),
                        state: NodeState::default(),
                    },
                    ElementLayer {
                        node_id,
                        doc: doc.clone(),
                        state: crate::layers::element::ElementState::default(),
                    },
                    HTMLElementLayer {},
                ),
            )?,
        },
        NodeKind::Text => new_from_chain::<TextLayer>(
            env,
            layer_chain!(
                EventTargetLayer::fresh(),
                NodeLayer {
                    node_id,
                    doc: doc.clone(),
                    state: NodeState::default(),
                },
                TextLayer {},
            ),
        )?,
        NodeKind::Comment => new_from_chain::<CommentLayer>(
            env,
            layer_chain!(
                EventTargetLayer::fresh(),
                NodeLayer {
                    node_id,
                    doc: doc.clone(),
                    state: NodeState::default(),
                },
                CommentLayer {},
            ),
        )?,
        _ => {
            return Err(Error::new(
                Status::GenericFailure,
                format!("No layer for node_kind {node_kind:?} (node_id={node_id})"),
            ));
        }
    };

    // 5. Determine initial reference strength: strong if the node is
    //    currently in the document tree, weak otherwise.
    let strong = doc.is_in_document(node_id);

    // 6. Cache the JS wrapper with the determined strength.
    doc.node_cache
        .borrow_mut()
        .insert(node_id, &js_node, env, strong, Rc::downgrade(doc))?;

    Ok(js_node)
}

// ── WindowDocument: blitz Document adapter ────────────────────────────

pub struct WindowDocument {
    pub doc: Rc<SharedDoc>,
}

impl WindowDocument {
    pub fn new(doc: Rc<SharedDoc>) -> Self {
        Self { doc }
    }
}

impl BlitzDocument for WindowDocument {
    fn inner(&self) -> DocGuard<'_> {
        let borrow = self.doc.base.borrow();
        DocGuard::RefCell(borrow)
    }

    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        let borrow = self.doc.base.borrow_mut();
        DocGuardMut::RefCell(borrow)
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        let handler = JsEventHandler {
            doc: Rc::downgrade(&self.doc),
        };
        let mut driver = EventDriver::new(self, handler);
        driver.handle_ui_event(event);
    }

    fn poll(&mut self, _task_context: Option<TaskContext>) -> bool {
        self.doc.take_host_dirty()
    }

    fn id(&self) -> usize {
        self.doc.base.borrow().id()
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

    let doc = Rc::new(SharedDoc::new(base));
    doc.set_env(env.clone());

    let node_id = doc.base.borrow().root_node().id;
    wrap_node(&doc, &env, node_id)
}
