//! `DocHandle`: the JS-facing handle to a blitz `BaseDocument`.
//!
//! Per-document state lives in `Rc<SharedDoc>` and is shared between
//! `DocHandle` (JS side) and `WindowDocument` (blitz window side).
//! JS constructor refs, event factory, and napi env are global
//! (`GlobalCreators` static), not per-document.
//!
//! The GC finalizer weak-references `SharedDoc`. When a JS Node is
//! collected, the finalizer upgrades the weak ref, removes the NodeCache
//! entry, and if the blitz node is detached, drops it from the doc tree.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    task::Context as TaskContext,
};

use crate::{
    dom::{
        event::JsEventHandler, input_data_handle::InputDataHandle, node_cache::NodeCache,
        node_handle::NativeNode,
    },
    global::{get_element_constructor, get_node_constructor},
    helpers::JsWeakRef,
};
use blitz::{
    dom::{
        BULLET_FONT, BaseDocument, DEFAULT_CSS, DocGuard, DocGuardMut, Document as BlitzDocument,
        DocumentConfig, EventDriver, FontContext, NodeData, NodeId,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::UiEvent,
};
use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{FnArgs, FromNapiValue, Object, ObjectRef, ToNapiValue, Uint8Array},
};
use parley::fontique::{Blob, FontInfoOverride, FontStyle, FontWeight, FontWidth};

#[cfg(debug_assertions)]
fn debug_ui_event_kind(event: &UiEvent) -> &'static str {
    match event {
        UiEvent::PointerMove(_) => "PointerMove",
        UiEvent::PointerCancel(_) => "PointerCancel",
        UiEvent::PointerUp(_) => "PointerUp",
        UiEvent::PointerDown(_) => "PointerDown",
        UiEvent::Wheel(_) => "Wheel",
        UiEvent::KeyUp(_) => "KeyUp",
        UiEvent::KeyDown(_) => "KeyDown",
        UiEvent::Ime(_) => "Ime",
        UiEvent::AppleStandardKeybinding(_) => "AppleStandardKeybinding",
    }
}

const DEFAULT_HTML: &str = "<!DOCTYPE html><html><head></head><body></body></html>";

fn parse_descriptor<T>(
    label: &str,
    raw: Option<&str>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>> {
    let Some(s) = raw else {
        return Ok(None);
    };
    parse(s).map(Some).ok_or_else(|| {
        Error::new(
            Status::InvalidArg,
            format!("registerFont: invalid CSS `{label}` descriptor: {s:?}"),
        )
    })
}

/// Configuration passed to `DocHandle.create`.
#[napi(object)]
pub struct DocHandleConfig {
    pub ua_stylesheets: Option<Vec<String>>,
    pub base_html: Option<String>,
}

/// Options for `DocHandle.registerFont`.
#[napi(object)]
pub struct RegisterFontOptions {
    pub family_name: Option<String>,
    pub weight: Option<String>,
    pub style: Option<String>,
    pub stretch: Option<String>,
}

// ── SharedDoc: per-document shared state ─────────────────────────────

/// Per-document shared state. Held inside `Rc` so that `DocHandle`
/// and `WindowDocument` can share it. The GC finalizer uses `Weak`.
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
}

impl SharedDoc {
    pub fn new(base: BaseDocument) -> Self {
        Self {
            base: RefCell::new(base),
            host_dirty: Cell::new(false),
            node_cache: RefCell::new(NodeCache::new()),
            js_document_ref: RefCell::new(None),
            js_window_ref: RefCell::new(None),
        }
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
    /// If the parent is detached, the subtree stays weak.
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
    ///
    /// After this call the node has no children and the caller can proceed
    /// with whatever replacement operation it needs (set text, set inner
    /// HTML, etc.).
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
        Ok(())
    }
}

// ── wrap_node: create or fetch JS Node wrapper ───────────────────────

/// Wrap a blitz node_id into a JS Node object.
///
/// Uses the global registry for JS constructor lookup and `SharedDoc`
/// for document lookup, node cache, and the JS Document ref.
pub fn wrap_node<'a>(doc: &Rc<SharedDoc>, node_id: NodeId, env: &'a Env) -> Result<Object<'a>> {
    // 1. Return an existing JS wrapper only after confirming that the
    //    underlying DOM node still exists.
    let cached = {
        let cache = doc.node_cache.borrow();
        cache.get(node_id, env)
    };
    if let Some(cached) = cached {
        return Ok(cached);
    }

    // 2. Read the node metadata. Invalid or stale ids must not fall
    //    through to constructor lookup as a made-up nodeType.
    let (node_type, qual_name) = {
        let base = doc.base.borrow();
        let node = base.get_node(node_id).ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                format!("No DOM node found for node_id={node_id}"),
            )
        })?;

        match &node.data {
            NodeData::Document(_) => (9u32, None),
            NodeData::Element(el) => (1u32, Some(el.name.clone())),
            NodeData::Text(_) => (3u32, None),
            NodeData::Comment { .. } => (8u32, None),
            _ => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Unsupported DOM node type for node_id={node_id}"),
                ));
            }
        }
    };

    // 3. Resolve the JS Document once. It is either returned directly
    //    or passed to the selected node constructor.
    let js_document = doc
        .js_document_ref
        .borrow()
        .as_ref()
        .and_then(|weak| weak.get_value(env))
        .ok_or_else(|| Error::new(Status::GenericFailure, "js_document_ref not set or dead"))?;

    if node_type == 9 {
        let strong = true; // Document node is always in-document.
        doc.node_cache.borrow_mut().insert(
            node_id,
            &js_document,
            env,
            strong,
            Rc::downgrade(doc),
        )?;
        return Ok(js_document);
    }

    // 4. Create the native node handle.
    let handle = NativeNode::new(node_id, doc.clone());

    // 5. Prefer a tag-specific element constructor over the generic
    //    constructor (matched by ns + local), fall back to the generic
    //    node_type constructor.
    let element_ctor = qual_name
        .as_ref()
        .and_then(|qn| get_element_constructor(&qn.ns, &qn.local));

    // 6. Build the optional extra argument for element constructors.
    let extra: Option<ObjectRef> = if element_ctor.is_some()
        && matches!(
            qual_name.as_ref().map(|qn| qn.local.as_ref()),
            Some("input") | Some("textarea")
        ) {
        let h = InputDataHandle::new(node_id, doc.clone());
        let obj_ref = ObjectRef::from_unknown(h.into_unknown(env)?)?;
        Some(obj_ref)
    } else {
        None
    };

    // 7. Call new Constructor(handle, document[, extra]).
    let document_ref = js_document.create_ref::<true>()?;

    let js_node = if let Some(ctor) = element_ctor {
        let ctor_fn = ctor.borrow_back(env)?;
        let result = ctor_fn.new_instance(FnArgs::from((handle, document_ref, extra)))?;
        Object::from_unknown(result)?
    } else {
        let ctor = get_node_constructor(node_type).ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                format!(
                    "No JS constructor registered for nodeType {node_type} (node_id={node_id})"
                ),
            )
        })?;
        let ctor_fn = ctor.borrow_back(env)?;
        let result = ctor_fn.new_instance(FnArgs::from((handle, document_ref)))?;
        Object::from_unknown(result)?
    };

    // 8. Determine initial reference strength: strong if the node is
    //    currently in the document tree, weak otherwise.
    let strong = doc.is_in_document(node_id);

    // 9. Cache the JS wrapper with the determined strength.
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
        #[cfg(debug_assertions)]
        if should_log_ui_event(&event) {
            eprintln!("napi-blitz[ui]: enter kind={}", debug_ui_event_kind(&event));
        }
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

#[cfg(debug_assertions)]
fn should_log_ui_event(event: &UiEvent) -> bool {
    matches!(event, UiEvent::PointerDown(_) | UiEvent::PointerUp(_))
}

// ── DocHandle: JS-facing handle ───────────────────────────────────────

#[napi]
pub struct NativeDoc {
    pub(crate) doc: Rc<SharedDoc>,
    pub(crate) font_ctx: FontContext,
    #[cfg(feature = "native-window")]
    pub(crate) moved_into_window: bool,
}

#[cfg(feature = "native-window")]
impl NativeDoc {
    pub(crate) fn share_doc(&self) -> Rc<SharedDoc> {
        self.doc.clone()
    }
}

#[napi]
impl NativeDoc {
    #[napi(factory)]
    pub fn create(_env: Env, config: DocHandleConfig) -> Result<Self> {
        let mut font_ctx = FontContext::new();
        font_ctx
            .collection
            .register_fonts(Blob::new(Arc::new(BULLET_FONT) as _), None);
        font_ctx.collection.make_shared();
        font_ctx.source_cache.make_shared();
        let shared_font_ctx = font_ctx.clone();

        let ua_stylesheets = config
            .ua_stylesheets
            .unwrap_or_else(|| vec![DEFAULT_CSS.to_string()]);
        let base_html = config.base_html.unwrap_or_else(|| DEFAULT_HTML.to_string());

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

        Ok(Self {
            doc,
            font_ctx: shared_font_ctx,
            #[cfg(feature = "native-window")]
            moved_into_window: false,
        })
    }

    #[cfg(feature = "native-window")]
    pub(crate) fn mark_attached(&mut self) -> bool {
        if self.moved_into_window {
            false
        } else {
            self.moved_into_window = true;
            true
        }
    }

    #[napi]
    pub fn resolve(&mut self, time_ms: f64) {
        self.doc.base.borrow_mut().resolve(time_ms);
    }

    #[napi]
    pub fn register_font(
        &mut self,
        data: Uint8Array,
        options: Option<RegisterFontOptions>,
    ) -> Result<u32> {
        if data.is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "registerFont: data buffer is empty",
            ));
        }
        let bytes: Vec<u8> = data.to_vec();
        let blob = Blob::new(Arc::new(bytes) as _);

        let family_name = options.as_ref().and_then(|o| o.family_name.as_deref());
        let weight = parse_descriptor(
            "weight",
            options.as_ref().and_then(|o| o.weight.as_deref()),
            FontWeight::parse_css,
        )?;
        let style = parse_descriptor(
            "style",
            options.as_ref().and_then(|o| o.style.as_deref()),
            FontStyle::parse_css,
        )?;
        let width = parse_descriptor(
            "stretch",
            options.as_ref().and_then(|o| o.stretch.as_deref()),
            FontWidth::parse_css,
        )?;

        let info_override =
            if family_name.is_some() || weight.is_some() || style.is_some() || width.is_some() {
                Some(FontInfoOverride {
                    family_name,
                    weight,
                    style,
                    width,
                    ..Default::default()
                })
            } else {
                None
            };

        let registered = self.font_ctx.collection.register_fonts(blob, info_override);
        let face_count: usize = registered.iter().map(|(_, fonts)| fonts.len()).sum();
        Ok(face_count as u32)
    }

    #[napi]
    pub fn root_node_id(&self) -> u64 {
        self.doc.base.borrow().root_node().id.as_u64()
    }

    #[napi]
    pub fn root_element_id(&self) -> u64 {
        self.doc.base.borrow().root_element().id.as_u64()
    }

    #[napi]
    pub fn set_document_ref(&self, env: Env, document: Object) -> Result<()> {
        *self.doc.js_document_ref.borrow_mut() = Some(JsWeakRef::new(&document, &env)?);
        Ok(())
    }

    /// Store a ref to the JS Window object so Rust can forward
    /// pointer events to it via the registered dispatch function.
    #[napi]
    pub fn set_window_ref(&self, env: Env, window: Object) -> Result<()> {
        *self.doc.js_window_ref.borrow_mut() = Some(JsWeakRef::new(&window, &env)?);
        Ok(())
    }
}
