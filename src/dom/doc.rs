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

use blitz::{
    dom::{
        BULLET_FONT, BaseDocument, DEFAULT_CSS, DocGuard, DocGuardMut,
        Document as BlitzDocument, DocumentConfig, EventDriver, FontContext, NodeData,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::UiEvent,
};
use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{Function, Object, Uint8Array},
    sys,
};
use napi_derive::napi;
use parley::fontique::{Blob, FontInfoOverride, FontStyle, FontWeight, FontWidth};

use crate::dom::event::JsEventHandler;
use crate::dom::global_creators as gc;
use crate::dom::node_cache::NodeCache;
use crate::dom::node_handle::NodeHandle;
use crate::dom::payload::EventPayload;

fn debug_ui_event_kind(event: &UiEvent) -> &'static str {
    match event {
        UiEvent::PointerMove(_) => "PointerMove",
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
    /// Weak-reference cache: blitz_node_id -> napi_ref (refcount=0)
    pub node_cache: RefCell<NodeCache>,
    /// Strong napi_ref to the JS Document object
    pub doc_js_ref: RefCell<Option<sys::napi_ref>>,
}

impl SharedDoc {
    pub fn new(base: BaseDocument) -> Self {
        Self {
            base: RefCell::new(base),
            host_dirty: Cell::new(false),
            node_cache: RefCell::new(NodeCache::new()),
            doc_js_ref: RefCell::new(None),
        }
    }

    pub fn mark_host_dirty(&self) {
        self.host_dirty.set(true);
    }

    pub fn take_host_dirty(&self) -> bool {
        self.host_dirty.replace(false)
    }
}

// ── wrap_node: create or fetch JS Node wrapper ───────────────────────

/// Wrap a blitz node_id into a JS Node object.
///
/// Uses `GlobalCreators` for JS constructor lookup and `SharedDoc`
/// for doc type lookup, node_cache, and doc_js ref.
pub fn wrap_node<'a>(
    doc: &Rc<SharedDoc>,
    node_id: usize,
    env: &'a Env,
) -> Result<Object<'a>> {
    // 1. Check cache.
    let cached = {
        let cache = doc.node_cache.borrow();
        NodeCache::get_from_map(&cache.entries, node_id, env)
    };
    if let Some(cached) = cached {
        return Ok(cached);
    }

    // 2. For Document nodes (nodeType 9), return the cached doc_js ref.
    let node_type = doc
        .base
        .borrow()
        .get_node(node_id)
        .map(|n| match &n.data {
            NodeData::Document => 9u32,
            NodeData::Element(_) => 1u32,
            NodeData::Text(_) => 3u32,
            NodeData::Comment => 8u32,
            _ => 0u32,
        })
        .unwrap_or(0);

    if node_type == 9 {
        let doc_ref = doc
            .doc_js_ref
            .borrow()
            .ok_or_else(|| Error::new(Status::GenericFailure, "doc_js not set"))?;
        let mut value = std::ptr::null_mut();
        napi::check_status!(unsafe {
            sys::napi_get_reference_value(env.raw(), doc_ref, &mut value)
        })?;
        let doc_obj = Object::from_raw(env.raw(), value);
        doc.node_cache
            .borrow_mut()
            .insert(node_id, &doc_obj, env, Rc::downgrade(doc))?;
        return Ok(doc_obj);
    }

    // 3. Create NodeHandle.
    let handle = NodeHandle::new(node_id, doc.clone());

    // 4. Get the constructor napi_ref and the doc_js napi_ref.
    let ctor_napi_ref = gc::get_node_constructor(node_type).ok_or_else(|| {
        Error::new(
            Status::GenericFailure,
            format!(
                "No JS constructor registered for nodeType {node_type} (node_id={node_id})"
            ),
        )
    })?;
    let doc_js_napi_ref = doc
        .doc_js_ref
        .borrow()
        .ok_or_else(|| Error::new(Status::GenericFailure, "doc_js not set"))?;

    // 5. Call new Constructor(handle, doc).
    let js_node = unsafe {
        let mut ctor_val = std::ptr::null_mut();
        napi::check_status!(sys::napi_get_reference_value(
            env.raw(),
            ctor_napi_ref,
            &mut ctor_val
        ))?;

        let mut doc_val = std::ptr::null_mut();
        napi::check_status!(sys::napi_get_reference_value(
            env.raw(),
            doc_js_napi_ref,
            &mut doc_val
        ))?;

        let handle_val =
            <NodeHandle as napi::bindgen_prelude::ToNapiValue>::to_napi_value(env.raw(), handle)?;

        let args = [handle_val, doc_val];
        let mut result = std::ptr::null_mut();
        napi::check_status!(sys::napi_new_instance(
            env.raw(),
            ctor_val,
            args.len(),
            args.as_ptr(),
            &mut result
        ))?;

        Object::from_raw(env.raw(), result)
    };

    // 6. Cache (weak ref).
    doc.node_cache
        .borrow_mut()
        .insert(node_id, &js_node, env, Rc::downgrade(doc))?;

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

fn should_log_ui_event(event: &UiEvent) -> bool {
    matches!(event, UiEvent::PointerDown(_) | UiEvent::PointerUp(_))
}

// ── DocHandle: JS-facing handle ───────────────────────────────────────

#[napi]
pub struct DocHandle {
    pub(crate) doc: Rc<SharedDoc>,
    pub(crate) font_ctx: FontContext,
    #[cfg(feature = "native-window")]
    pub(crate) moved_into_window: bool,
}

#[cfg(feature = "native-window")]
impl DocHandle {
    pub(crate) fn share_doc(&self) -> Rc<SharedDoc> {
        self.doc.clone()
    }
}

#[napi]
impl DocHandle {
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

    #[cfg(feature = "native-window")]
    pub(crate) fn doc_id(&self) -> usize {
        self.doc.base.borrow().id()
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
        self.doc.base.borrow().root_node().id as u64
    }

    #[napi]
    pub fn root_element_id(&self) -> u64 {
        self.doc.base.borrow().root_element().id as u64
    }

    #[napi]
    pub fn set_doc_js(&self, env: Env, doc: Object) -> Result<()> {
        gc::set_env(env.raw());
        let mut napi_ref = std::ptr::null_mut();
        napi::check_status!(unsafe {
            sys::napi_create_reference(env.raw(), napi::JsValue::raw(&doc), 1, &mut napi_ref)
        })?;
        *self.doc.doc_js_ref.borrow_mut() = Some(napi_ref);
        Ok(())
    }
}

// ── Global registration functions (no DocHandle instance needed) ──────

#[napi]
pub fn register_node_constructor(
    env: Env,
    node_type: u32,
    constructor: Function<napi::bindgen_prelude::Unknown, napi::bindgen_prelude::Unknown>,
) -> Result<()> {
    gc::set_env(env.raw());
    let mut napi_ref = std::ptr::null_mut();
    napi::check_status!(unsafe {
        sys::napi_create_reference(env.raw(), napi::JsValue::raw(&constructor), 1, &mut napi_ref)
    })?;
    gc::insert_node_constructor(node_type, napi_ref);
    Ok(())
}

#[napi]
pub fn register_event_factory(
    env: Env,
    factory: Function<EventPayload, napi::bindgen_prelude::Unknown>,
) -> Result<()> {
    gc::set_env(env.raw());
    let mut napi_ref = std::ptr::null_mut();
    napi::check_status!(unsafe {
        sys::napi_create_reference(env.raw(), napi::JsValue::raw(&factory), 1, &mut napi_ref)
    })?;
    gc::set_event_factory(napi_ref);
    Ok(())
}

/// Internal helper: build a WindowDocument from a DocHandle.
#[cfg(feature = "native-window")]
pub(crate) fn make_window_document(handle: &DocHandle) -> Box<WindowDocument> {
    Box::new(WindowDocument::new(handle.share_doc()))
}
