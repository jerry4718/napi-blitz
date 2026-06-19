//! `DocHandle`: the JS-facing handle to a blitz `BaseDocument`.
//!
//! The handle owns two shared cells:
//!   - `SharedBaseDoc`: `Rc<RefCell<BaseDocument>>` — the document tree.
//!   - `SharedBridge`: `Rc<RefCell<JsBridge>>` — the JS event-dispatch bridge.
//!
//! Splitting them into separate `RefCell`s is deliberate: blitz's
//! `EventDriver` needs `&mut dyn Document` for the duration of event
//! dispatch, and during that time the JS callback may fire. The JS
//! callback must be able to read/mutate the DOM (which borrows
//! `SharedBaseDoc`) without colliding with the bridge borrow. If both
//! lived in the same `RefCell<DocState>`, any JS-triggered DOM op
//! during dispatch would panic with "already mutably borrowed".
//!
//! `WindowDocument` implements `Document` by borrowing `SharedBaseDoc`
//! on each `inner()` / `inner_mut()` call and releasing immediately, so
//! no borrow spans across the JS callback boundary.

use std::{cell::RefCell, rc::Rc, sync::Arc, task::Context as TaskContext};

use blitz::{
    dom::{
        BULLET_FONT, BaseDocument, DEFAULT_CSS, DocGuard, DocGuardMut, Document as BlitzDocument,
        DocumentConfig, EventDriver, FontContext,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::UiEvent,
};
use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{Function, FunctionRef, Uint8Array},
};
use napi_derive::napi;
use parley::fontique::{Blob, FontInfoOverride, FontStyle, FontWeight, FontWidth};

use crate::event::{JsBridge, JsEventHandler};
use crate::payload::{DispatchResult, EventPayload};

const DEFAULT_HTML: &str = "<!DOCTYPE html><html><head></head><body></body></html>";

/// Helper for `register_font`: turn an optional CSS descriptor string
/// into the corresponding fontique type, mapping a parse failure into a
/// `napi::Error` so JS sees a thrown exception rather than a silent
/// fallback.
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

/// Options for `DocHandle.registerFont`. All fields are optional and act
/// as overrides for the metadata that `parley` would otherwise read from
/// the font file's own tables.
///
/// Mirrors the subset of CSS `@font-face` descriptors that map cleanly
/// onto the underlying font cache. Each `weight` / `style` / `stretch`
/// value is a CSS string (e.g. `"400"`, `"bold"`, `"italic"`,
/// `"oblique 14deg"`, `"condensed"`, `"75%"`); invalid input is
/// reported back to JS as a thrown error, matching how the browser
/// rejects invalid `FontFaceDescriptors`.
#[napi(object)]
pub struct RegisterFontOptions {
    /// Override the family name reported to CSS. If omitted, the family
    /// name embedded in the font file is used.
    pub family_name: Option<String>,
    /// CSS `font-weight` descriptor string, e.g. `"400"`, `"bold"`,
    /// `"100 900"` (variable). For variable fonts the lower bound is
    /// used as the registered weight.
    pub weight: Option<String>,
    /// CSS `font-style` descriptor, e.g. `"normal"`, `"italic"`,
    /// `"oblique"`, `"oblique 14deg"`.
    pub style: Option<String>,
    /// CSS `font-stretch` descriptor, e.g. `"normal"`, `"condensed"`,
    /// `"75%"`. (Standard name; we keep the CSS spelling rather than
    /// fontique's `width`.)
    pub stretch: Option<String>,
}

/// `Rc<RefCell<BaseDocument>>` — the document tree, shared between
/// `DocHandle` (JS side) and `WindowDocument` (blitz window side).
#[derive(Clone)]
pub struct SharedBaseDoc(pub Rc<RefCell<BaseDocument>>);

impl SharedBaseDoc {
    pub fn new(base: BaseDocument) -> Self {
        Self(Rc::new(RefCell::new(base)))
    }
}

/// `Rc<RefCell<JsBridge>>` — the JS event-dispatch bridge, shared
/// between `DocHandle` and `WindowDocument`.
#[derive(Clone)]
pub struct SharedBridge(pub Rc<RefCell<JsBridge>>);

impl SharedBridge {
    pub fn new(bridge: JsBridge) -> Self {
        Self(Rc::new(RefCell::new(bridge)))
    }
}

/// Adapter that implements blitz's `Document` trait around our shared
/// `BaseDocument`. Each `inner()` / `inner_mut()` call borrows the
/// `RefCell` transiently and releases the guard before returning to the
/// caller's scope — crucially, no borrow spans across JS callbacks.
pub struct WindowDocument {
    pub base: SharedBaseDoc,
    pub bridge: SharedBridge,
}

impl WindowDocument {
    pub fn new(base: SharedBaseDoc, bridge: SharedBridge) -> Self {
        Self { base, bridge }
    }
}

impl BlitzDocument for WindowDocument {
    fn inner(&self) -> DocGuard<'_> {
        let borrow = self.base.0.borrow();
        DocGuard::RefCell(borrow)
    }

    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        let borrow = self.base.0.borrow_mut();
        DocGuardMut::RefCell(borrow)
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        // Clone the bridge `Rc` so we can borrow it independently of
        // `&mut self`. This lets `EventDriver::new(self, handler)` take
        // `&mut self` (for `inner()` / `inner_mut()` calls) while the
        // handler holds a separate `RefMut<JsBridge>`.
        //
        // The bridge and base live in *separate* `RefCell`s, so JS
        // callbacks (triggered inside `driver.handle_ui_event`) can
        // freely borrow `base` without colliding with the bridge borrow.
        let bridge_rc = self.bridge.0.clone();
        let mut bridge = bridge_rc.borrow_mut();
        let handler = JsEventHandler {
            bridge: &mut bridge,
        };

        let mut driver = EventDriver::new(self, handler);
        driver.handle_ui_event(event);
    }

    fn poll(&mut self, _task_context: Option<TaskContext>) -> bool {
        false
    }

    fn id(&self) -> usize {
        self.base.0.borrow().id()
    }
}

/// JS-facing handle. Holds the shared document state and exposes the flat
/// nodeId-based DOM API.
#[napi]
pub struct DocHandle {
    pub(crate) base: SharedBaseDoc,
    pub(crate) bridge: SharedBridge,
    /// A clone of the same `FontContext` that was passed into
    /// `BaseDocument::new`. Because we call `make_shared()` on its
    /// `Collection` and `SourceCache` before cloning, mutations on
    /// either copy (registering fonts here vs. blitz's `@font-face`
    /// loader) propagate to the other through the internal `Arc`s.
    /// This lets JS register raw font buffers at runtime without going
    /// through the network/CSS path.
    pub(crate) font_ctx: FontContext,
    /// Whether ownership of the document has been moved into a window.
    /// After this we still keep the `Rc` so the JS side can keep mutating
    /// the DOM, but we refuse to attach it to a second window.
    pub(crate) moved_into_window: bool,
}

impl DocHandle {
    pub(crate) fn share_base(&self) -> SharedBaseDoc {
        self.base.clone()
    }
    pub(crate) fn share_bridge(&self) -> SharedBridge {
        self.bridge.clone()
    }
}

#[napi]
impl DocHandle {
    /// Create a new document.
    #[napi(factory)]
    pub fn create(env: Env, config: DocHandleConfig<'_>) -> Result<Self> {
        // Build the font context up-front so we can both pass it to
        // blitz (via `DocumentConfig::font_ctx`) and keep a cloned
        // handle on `DocHandle` for runtime JS-side font registration.
        //
        // Calling `make_shared()` on the `Collection` and `SourceCache`
        // is mandatory: without it, `Clone` produces independent copies
        // and mutations on one would not be visible to the other. After
        // `make_shared`, both copies hold the same internal `Arc<Shared>`
        // and any `register_fonts` call goes through the shared
        // `Mutex<Data>`.
        let mut font_ctx = FontContext::new();
        // Bullet font for list-item markers. blitz also registers this
        // when it builds its own default `FontContext`, but since we are
        // now supplying ours we must register it ourselves.
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

        // Parse the initial HTML into the base document.
        {
            let mut mutator = base.mutate();
            DocumentHtmlParser::parse_into_mutator(&mut mutator, &base_html);
        }
        base.resolve(0.0);

        let callback_ref: FunctionRef<EventPayload, DispatchResult> =
            config.on_dispatch.create_ref()?;
        let bridge = JsBridge::new(env, callback_ref);

        let shared_base = SharedBaseDoc::new(base);
        let shared_bridge = SharedBridge::new(bridge);

        Ok(Self {
            base: shared_base,
            bridge: shared_bridge,
            font_ctx: shared_font_ctx,
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

    /// blitz-internal `BaseDocument` id. Used by `BlitzApp` to route window
    /// open/close to the right `View`.
    pub(crate) fn doc_id(&self) -> usize {
        self.base.0.borrow().id()
    }

    /// Recompute style + layout. Called from JS after batches of mutations or
    /// before painting. `time_ms` drives CSS animations.
    #[napi]
    pub fn resolve(&mut self, time_ms: f64) {
        self.base.0.borrow_mut().resolve(time_ms);
    }

    /// Register a font from a raw byte buffer.
    ///
    /// The buffer must contain a single TTF/OTF/WOFF/WOFF2 font file (or
    /// a TrueType collection — every face inside will be registered).
    /// Returns the number of faces that were added.
    ///
    /// This bypasses blitz's `@font-face` machinery entirely: there is
    /// no network fetch, no CSS parsing, just a direct insert into the
    /// shared `Collection`. The font becomes available to all subsequent
    /// layout/paint work driven by this document.
    ///
    /// The buffer is taken by value and stored inside the font cache;
    /// callers may safely drop their reference to it after this call
    /// returns.
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
        // Copy the bytes out of the V8 typed array into an owned Vec so
        // the font cache (which keeps the data alive in an `Arc<[u8]>`)
        // doesn't depend on V8's GC. `Uint8Array` is zero-copy on the
        // way in but owning a Rust-side copy decouples lifetimes.
        let bytes: Vec<u8> = data.to_vec();
        let blob = Blob::new(Arc::new(bytes) as _);

        // Parse the optional CSS descriptor strings into fontique types.
        // Each helper returns `Option<T>`: `None` for unspecified, an
        // `Err` for an explicit-but-unparseable value (we surface that
        // back to JS just like a browser would reject an invalid
        // `FontFaceDescriptors` entry).
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

    /// The id of the root node (always 0 for blitz, but expose it for JS).
    #[napi]
    pub fn root_node_id(&self) -> u32 {
        self.base.0.borrow().root_node().id as u32
    }

    /// The id of `<html>` (the root *element*).
    #[napi]
    pub fn root_element_id(&self) -> u32 {
        self.base.0.borrow().root_element().id as u32
    }

    /// Update the set of node ids JS currently has live wrappers for. Rust
    /// uses this to short-circuit dispatch when no listener could exist.
    #[napi]
    pub fn set_listened_nodes(&mut self, ids: Vec<u32>) {
        let mut bridge = self.bridge.0.borrow_mut();
        bridge.listened_nodes = ids.into_iter().collect();
    }

    /// Add a single node id to the listened set. Cheaper than calling
    /// `set_listened_nodes` for incremental subscription updates.
    #[napi]
    pub fn add_listened_node(&mut self, id: u32) {
        self.bridge.0.borrow_mut().listened_nodes.insert(id);
    }

    /// Remove a node id from the listened set.
    #[napi]
    pub fn remove_listened_node(&mut self, id: u32) {
        self.bridge.0.borrow_mut().listened_nodes.remove(&id);
    }
}

/// Internal helper: build a [`WindowDocument`] from a [`DocHandle`] without
/// transferring the underlying `Rc`s away from the handle. The window will
/// receive `Box<WindowDocument>`; the handle keeps its own clones.
pub(crate) fn make_window_document(handle: &DocHandle) -> Box<WindowDocument> {
    Box::new(WindowDocument::new(
        handle.share_base(),
        handle.share_bridge(),
    ))
}
