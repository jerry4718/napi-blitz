//! DOM operations exposed to JS as methods on `DocHandle`.
//!
//! Methods that return DOM nodes (querySelector, createElement, etc.)
//! perform wrapping on the Rust side via `SharedWrapper::wrap_node` and
//! return JS Node objects directly. JS never calls `wrapNode` itself.

use blitz::{
    dom::BaseDocument,
    dom::{Attribute as BlitzAttribute, LocalName, Namespace, NodeId, QualName, local_name, ns},
    html::DocumentHtmlParser,
};
use napi::{
    Env, Error, Result,
    bindgen_prelude::{BigInt, Object},
};
use style::{Atom, invalidation::element::restyle_hints::RestyleHint, properties::PropertyId};

use crate::dom::{
    doc::{NativeDoc, wrap_node},
    node_handle::NativeNode,
};

/// Plain attribute pair used by the create/insert APIs.
#[napi(object)]
pub struct AttrInit {
    pub name: String,
    pub value: String,
    pub namespace: Option<String>,
}

pub(crate) fn make_qual_name(local: &str, namespace: Option<&str>) -> QualName {
    QualName {
        prefix: None,
        ns: namespace.map(Namespace::from).unwrap_or(ns!(html)),
        local: LocalName::from(local),
    }
}

fn js_to_node_id(id: &BigInt) -> NodeId {
    let (signed, value, lossless) = id.get_u64();
    if signed || !lossless {
        return NodeId::default();
    }
    NodeId::from_u64(value)
}

/// Mark a node as needing style/layout work after mutating its inline
/// declaration block through Blitz's style-property helpers.
///
/// `DocumentMutator::{set,remove}_style_property` in blitz-dom 0.3 delegates
/// straight to `BaseDocument`, which updates the parsed inline style block but
/// skips the invalidation work done by `DocumentMutator::set_attribute("style")`:
/// snapshotting, restyle hints, damage, and dirty ancestor propagation. The
/// missing dirty propagation means subsequent surface updates can skip the
/// subtree entirely.
///
/// Until blitz-dom exposes a fully invalidating style-property mutator, keep
/// the parsed-style mutation path but add the public invalidation pieces here.
pub(crate) fn mark_inline_style_mutated(state: &mut BaseDocument, node_id: NodeId) {
    state.snapshot_node(node_id);
    if let Some(node) = state.get_node_mut(node_id) {
        node.set_restyle_hint(RestyleHint::RESTYLE_STYLE_ATTRIBUTE);
    }
}

pub(crate) fn set_detached_attribute(
    state: &mut BaseDocument,
    node_id: NodeId,
    name: QualName,
    value: &str,
) -> bool {
    let Some(node) = state.get_node_mut(node_id) else {
        return false;
    };
    if node.flags.is_in_document() || name.local == local_name!("style") {
        return false;
    }

    // Upstream Blitz workaround:
    // `DocumentMutator::set_attribute` currently calls `snapshot_node` before
    // checking whether the target node is in the document. For detached nodes
    // that have never been styled, that snapshot can later be consumed by
    // Stylo invalidation after the node is inserted into the document. With
    // ancestor selectors such as `.page-header h1`, Stylo may then read the
    // old primary style from the snapshot path and panic because it is `None`.
    //
    // Direct Rust repro against blitz-dom 0.3.0-alpha.5:
    //   1. create <header> detached
    //   2. mutator.set_attribute(header, class, "page-header")
    //   3. append detached <h1> child and then append header to <body>
    //   4. resolve a document containing `.page-header h1 { ... }`
    //
    // Browser DOM semantics do not require detached attribute changes to enter
    // style invalidation at all. Keep detached ordinary attributes as plain DOM
    // data updates here. Once Blitz handles detached-node snapshots safely,
    // remove this fast path and let all attributes use `DocumentMutator` again.
    let Some(element) = node.element_data_mut() else {
        return false;
    };
    if name.local == local_name!("id") {
        element.id = Some(Atom::from(value));
    }
    element.attrs.set(name, value);
    true
}

pub(crate) fn remove_detached_attribute(
    state: &mut BaseDocument,
    node_id: NodeId,
    name: &QualName,
) -> bool {
    let Some(node) = state.get_node_mut(node_id) else {
        return false;
    };
    if node.flags.is_in_document() || name.local == local_name!("style") {
        return false;
    }

    // See `set_detached_attribute` for why detached ordinary attributes bypass
    // `DocumentMutator` until the upstream Blitz snapshot bug is fixed.
    let Some(element) = node.element_data_mut() else {
        return false;
    };
    if name.local == local_name!("id") {
        element.id = None;
    }
    element.attrs.remove(name);
    true
}

#[napi]
impl NativeDoc {
    /// Replace document content from an HTML string. Useful for tests and
    /// initial bootstrapping when `base_html` was not enough.
    #[napi]
    pub fn load_html(&mut self, html: String) {
        let mut state = self.doc.base.borrow_mut();
        {
            let mut mutator = state.mutate();
            DocumentHtmlParser::parse_into_mutator(&mut mutator, &html);
        }
        state.resolve(0.0);
        drop(state);
        self.doc.mark_host_dirty();
    }

    /// Find a single node by CSS selector. Returns a wrapped JS Node or null.
    #[napi]
    pub fn query_selector<'a>(&self, selector: String, env: &'a Env) -> Result<Option<Object<'a>>> {
        let state = self.doc.base.borrow();
        match state.query_selector(&selector) {
            Ok(Some(id)) => Ok(Some(wrap_node(&self.doc, id, env)?)),
            Ok(None) => Ok(None),
            Err(err) => Err(Error::from_reason(format!("query_selector: {err:?}"))),
        }
    }

    /// Find all nodes by CSS selector. Returns wrapped JS Node objects.
    #[napi]
    pub fn query_selector_all<'a>(
        &self,
        selector: String,
        env: &'a Env,
    ) -> Result<Vec<Object<'a>>> {
        let state = self.doc.base.borrow();
        match state.query_selector_all(&selector) {
            Ok(ids) => {
                let mut result = Vec::new();
                for id in ids {
                    result.push(wrap_node(&self.doc, id, env)?);
                }
                Ok(result)
            }
            Err(err) => Err(Error::from_reason(format!("query_selector_all: {err:?}"))),
        }
    }

    /// Element-scoped `querySelector`: first match in the subtree rooted at
    /// `root_id` (exclusive — the root element itself is not a candidate,
    /// matching the DOM spec for `Element.querySelector`). We parse the
    /// selector via blitz's public `try_parse_selector_list` and then call
    /// stylo's `dom_apis::query_selector` directly with `root_id`'s node as
    /// the root — bypassing blitz's `query_selector_raw`, which is hardcoded
    /// to `self.root_node()`.
    #[napi]
    pub fn query_selector_in(&self, root_id: BigInt, selector: String) -> Result<Option<u64>> {
        let state = self.doc.base.borrow();
        let selector_list = state
            .try_parse_selector_list(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector_in: {err:?}")))?;

        let Some(root_node) = state.get_node(js_to_node_id(&root_id)) else {
            return Ok(None);
        };

        use blitz::dom::Node;
        let mut result: Option<&Node> = None;
        style::dom_apis::query_selector::<&Node, style::dom_apis::QueryFirst>(
            root_node,
            &selector_list,
            &mut result,
            style::dom_apis::MayUseInvalidation::Yes,
        );
        Ok(result.map(|node| node.id.as_u64()))
    }

    /// Element-scoped `querySelectorAll`: all matches in the subtree rooted
    /// at `root_id` (exclusive). Same approach as `query_selector_in`.
    #[napi]
    pub fn query_selector_all_in(&self, root_id: BigInt, selector: String) -> Result<Vec<u64>> {
        let state = self.doc.base.borrow();
        let selector_list = state
            .try_parse_selector_list(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector_all_in: {err:?}")))?;

        let Some(root_node) = state.get_node(js_to_node_id(&root_id)) else {
            return Ok(Vec::new());
        };

        use blitz::dom::Node;
        let mut results: style::dom_apis::QuerySelectorAllResult<&Node> = Default::default();
        style::dom_apis::query_selector::<&Node, style::dom_apis::QueryAll>(
            root_node,
            &selector_list,
            &mut results,
            style::dom_apis::MayUseInvalidation::Yes,
        );
        Ok(results.iter().map(|node| node.id.as_u64()).collect())
    }

    /// Lookup by `id=` attribute, like `document.getElementById`.
    #[napi]
    pub fn get_element_by_id<'a>(&self, id: String, env: &'a Env) -> Option<Object<'a>> {
        let node_id = self.doc.base.borrow().get_element_by_id(&id)?;
        wrap_node(&self.doc, node_id, env).ok()
    }

    /// Find the document's `<title>` element id, or None if no title
    /// element exists in the tree. Uses the same pre-order DFS as the
    /// other structural lookups (`html`/`head`/`body`) — cheaper than
    /// `querySelector("title")` which dispatches through the CSS
    /// selector engine.
    #[napi]
    pub fn find_title_node<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let id = self.find_first_static(local_name!("title"))?;
        wrap_node(&self.doc, id, env).ok()
    }

    /// True iff the given node id currently exists in the document.
    #[napi]
    pub fn has_node(&self, id: BigInt) -> bool {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&id))
            .is_some()
    }

    #[napi]
    pub fn node_handle(&self, id: BigInt) -> Option<NativeNode> {
        let node_id = js_to_node_id(&id);
        self.doc.base.borrow().get_node(node_id)?;
        Some(NativeNode::new(node_id, self.doc.clone()))
    }
}

#[napi]
impl NativeDoc {
    /// Create an element node. Returns a wrapped JS Node. The element is
    /// detached (no parent) until inserted.
    #[napi]
    pub fn create_element<'a>(
        &mut self,
        local_name: String,
        namespace: Option<String>,
        attrs: Option<Vec<AttrInit>>,
        env: &'a Env,
    ) -> Result<Object<'a>> {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        let qn = make_qual_name(&local_name, namespace.as_deref());
        let attr_vec: Vec<BlitzAttribute> = attrs
            .unwrap_or_default()
            .into_iter()
            .map(|a| BlitzAttribute {
                name: make_qual_name(&a.name, a.namespace.as_deref()),
                value: a.value,
            })
            .collect();
        let node_id = mutator.create_element(qn, attr_vec);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, node_id, env)
    }

    /// Create a text node with the given content. Returns a wrapped JS Node.
    #[napi]
    pub fn create_text_node<'a>(&mut self, text: String, env: &'a Env) -> Result<Object<'a>> {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        let node_id = mutator.create_text_node(&text);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, node_id, env)
    }

    /// Create a comment node with the given content. Returns a wrapped JS Node.
    #[napi]
    pub fn create_comment_node<'a>(&mut self, text: String, env: &'a Env) -> Result<Object<'a>> {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        let node_id = mutator.create_comment_node(&text);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, node_id, env)
    }

    /// Deep-clone an existing node and return the new node's id.
    #[napi]
    pub fn deep_clone_node(&mut self, node_id: BigInt) -> u64 {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        mutator.deep_clone_node(js_to_node_id(&node_id)).as_u64()
    }

    /// Shallow-clone a node: same data (tag name, attributes, text
    /// payload, etc.) but no children. The new node has no parent.
    /// Returns the new node's id.
    ///
    /// Cloning a missing nodeId returns 0 (the document root) — the
    /// caller should make sure the source id is valid first. The
    /// alternative (returning `Option<u64>`) noisily complicates the
    /// JS-side cloneNode wrapper for a case JS code can never trigger.
    #[napi]
    pub fn shallow_clone_node(&mut self, node_id: BigInt) -> u64 {
        let mut state = self.doc.base.borrow_mut();
        let Some(source) = state.get_node(js_to_node_id(&node_id)) else {
            return 0;
        };
        // Cloning `NodeData` deep-copies attributes, text, and the
        // (Arc-shared) parsed `style` declaration block. We never
        // touch `children` / `parent` so the clone starts detached.
        let data = source.data.clone();
        state.create_node(data).as_u64()
    }
}

#[napi]
impl NativeDoc {
    /// Parent node id, if any.
    #[napi]
    pub fn parent_id(&self, node_id: BigInt) -> Option<u64> {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.parent)
            .map(|id| id.as_u64())
    }

    /// First child id, if any.
    #[napi]
    pub fn first_child_id(&self, node_id: BigInt) -> Option<u64> {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.children.first().copied())
            .map(|id| id.as_u64())
    }

    /// Last child id, if any.
    #[napi]
    pub fn last_child_id(&self, node_id: BigInt) -> Option<u64> {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.children.last().copied())
            .map(|id| id.as_u64())
    }

    /// All children, in document order.
    #[napi]
    pub fn child_ids(&self, node_id: BigInt) -> Vec<u64> {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&node_id))
            .map(|n| n.children.iter().map(|id| id.as_u64()).collect())
            .unwrap_or_default()
    }

    /// Next sibling id, if any.
    #[napi]
    pub fn next_sibling_id(&self, node_id: BigInt) -> Option<u64> {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.forward(1))
            .map(|n| n.id.as_u64())
    }

    /// Previous sibling id, if any.
    #[napi]
    pub fn previous_sibling_id(&self, node_id: BigInt) -> Option<u64> {
        self.doc
            .base
            .borrow()
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.backward(1))
            .map(|n| n.id.as_u64())
    }
}

/// Mirrors web NodeType numeric codes for the small subset blitz exposes.
const NODE_TYPE_ELEMENT: u32 = 1;
const NODE_TYPE_TEXT: u32 = 3;
const NODE_TYPE_COMMENT: u32 = 8;
const NODE_TYPE_DOCUMENT: u32 = 9;
const NODE_TYPE_OTHER: u32 = 0;

#[napi]
impl NativeDoc {
    /// DOM-style `nodeType` (1=Element, 3=Text, 8=Comment, 9=Document).
    #[napi]
    pub fn node_type(&self, node_id: BigInt) -> u32 {
        let state = self.doc.base.borrow();
        let Some(node) = state.get_node(js_to_node_id(&node_id)) else {
            return NODE_TYPE_OTHER;
        };
        use blitz::dom::NodeData;
        match &node.data {
            NodeData::Document(_) => NODE_TYPE_DOCUMENT,
            NodeData::Element(_) => NODE_TYPE_ELEMENT,
            NodeData::Text(_) => NODE_TYPE_TEXT,
            NodeData::Comment { .. } => NODE_TYPE_COMMENT,
            _ => NODE_TYPE_OTHER,
        }
    }

    /// Local element tag name (lowercased), e.g. "div". Returns None for
    /// non-element nodes.
    #[napi]
    pub fn tag_name(&self, node_id: BigInt) -> Option<String> {
        let state = self.doc.base.borrow();
        state
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.element_data())
            .map(|el| el.name.local.to_string())
    }

    /// Concatenated text content of this node and its descendants. Mirrors
    /// `Node.textContent`.
    #[napi]
    pub fn text_content(&self, node_id: BigInt) -> Option<String> {
        let state = self.doc.base.borrow();
        state
            .get_node(js_to_node_id(&node_id))
            .map(|n| n.text_content())
    }

    /// Get an attribute value, or None if missing or node is not an element.
    #[napi]
    pub fn get_attribute(&self, node_id: BigInt, name: String) -> Option<String> {
        let state = self.doc.base.borrow();
        let node = state.get_node(js_to_node_id(&node_id))?;
        let local = LocalName::from(name.as_str());
        node.attr(local).map(|s| s.to_string())
    }

    /// All attribute (name, value) pairs on this node, or empty if not an
    /// element.
    #[napi]
    pub fn get_attributes(&self, node_id: BigInt) -> Vec<AttrInit> {
        let state = self.doc.base.borrow();
        let Some(node) = state.get_node(js_to_node_id(&node_id)) else {
            return Vec::new();
        };
        let Some(attrs) = node.attrs() else {
            return Vec::new();
        };
        attrs
            .iter()
            .map(|a| AttrInit {
                name: a.name.local.to_string(),
                value: a.value.clone(),
                namespace: Some(a.name.ns.to_string()),
            })
            .collect()
    }
}

#[napi]
impl NativeDoc {
    /// Set an attribute on an element.
    #[napi]
    pub fn set_attribute(
        &mut self,
        node_id: BigInt,
        name: String,
        value: String,
        namespace: Option<String>,
    ) {
        let mut state = self.doc.base.borrow_mut();
        let node_id = js_to_node_id(&node_id);
        let name = make_qual_name(&name, namespace.as_deref());
        if set_detached_attribute(&mut state, node_id, name.clone(), &value) {
            drop(state);
            self.doc.mark_host_dirty();
            return;
        }
        let mut mutator = state.mutate();
        mutator.set_attribute(node_id, name, &value);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
    }

    /// Remove an attribute from an element.
    #[napi]
    pub fn remove_attribute(&mut self, node_id: BigInt, name: String, namespace: Option<String>) {
        let mut state = self.doc.base.borrow_mut();
        let node_id = js_to_node_id(&node_id);
        let name = make_qual_name(&name, namespace.as_deref());
        if remove_detached_attribute(&mut state, node_id, &name) {
            drop(state);
            self.doc.mark_host_dirty();
            return;
        }
        let mut mutator = state.mutate();
        mutator.clear_attribute(node_id, name);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
    }

    /// Set a single inline style property (e.g. "color", "#ff0000").
    #[napi]
    pub fn set_style_property(&mut self, node_id: BigInt, name: String, value: String) {
        let mut state = self.doc.base.borrow_mut();
        let node_id = js_to_node_id(&node_id);
        mark_inline_style_mutated(&mut state, node_id);
        state.set_style_property(node_id, &name, &value);
        drop(state);
        self.doc.mark_host_dirty();
    }

    /// Remove a single inline style property.
    #[napi]
    pub fn remove_style_property(&mut self, node_id: BigInt, name: String) {
        let mut state = self.doc.base.borrow_mut();
        let node_id = js_to_node_id(&node_id);
        mark_inline_style_mutated(&mut state, node_id);
        state.remove_style_property(node_id, &name);
        drop(state);
        self.doc.mark_host_dirty();
    }

    /// Read a single inline style property's serialized value, or
    /// `null` if the property is not set on this element.
    ///
    /// Implements CSSOM `CSSStyleDeclaration.getPropertyValue`:
    /// the value is rendered through stylo's `property_value_to_css`,
    /// which handles both longhands and shorthands. An unknown
    /// property name (one stylo doesn't recognize) also returns `null`
    /// rather than throwing — matching browser semantics.
    #[napi]
    pub fn get_style_property(&self, node_id: BigInt, name: String) -> Option<String> {
        let state = self.doc.base.borrow();
        let element_data = state.get_node(js_to_node_id(&node_id))?.element_data()?;
        let block = element_data.style_attribute.as_ref()?;
        let property_id = PropertyId::parse_enabled_for_all_content(&name).ok()?;

        let guard = state.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        // `property_value_to_css` writes nothing when the property is
        // not present. Distinguish "set to empty" from "absent" via
        // `block.declarations()` would be more rigorous, but the
        // browser behavior of `getPropertyValue` is also "" for
        // unset, so we collapse the two: an empty result means absent.
        block.property_value_to_css(&property_id, &mut buf).ok()?;
        if buf.is_empty() { None } else { Some(buf) }
    }

    /// List the long-hand names of every property currently in this
    /// element's inline style block.
    ///
    /// Used by the JS-side `style` Proxy to implement `Object.keys`,
    /// `for...in`, and `length`. The names are stylo's longhand
    /// identifiers (e.g. `"color"`, `"margin-top"`). Custom properties
    /// (`--foo`) are included as-is.
    #[napi]
    pub fn get_style_property_names(&self, node_id: BigInt) -> Vec<String> {
        let state = self.doc.base.borrow();
        let Some(element_data) = state
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.element_data())
        else {
            return Vec::new();
        };
        let Some(block) = element_data.style_attribute.as_ref() else {
            return Vec::new();
        };
        let guard = state.guard().read();
        let block = block.read_with(&guard);
        block
            .declarations()
            .iter()
            .map(|d| d.id().name().into_owned())
            .collect()
    }

    /// Read the entire `style` attribute as a single CSS string. Used
    /// to back `CSSStyleDeclaration.cssText`. Returns the empty string
    /// when the element has no inline style at all.
    #[napi]
    pub fn get_style_attribute(&self, node_id: BigInt) -> String {
        let state = self.doc.base.borrow();
        let Some(element_data) = state
            .get_node(js_to_node_id(&node_id))
            .and_then(|n| n.element_data())
        else {
            return String::new();
        };
        let Some(block) = element_data.style_attribute.as_ref() else {
            return String::new();
        };
        let guard = state.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        let _ = block.to_css(&mut buf);
        buf
    }

    /// Replace this node's text content. For elements this resets to a single
    /// text-node child; for text/comment nodes this updates their content.
    #[napi]
    pub fn set_text_content(&mut self, node_id: BigInt, text: String, env: &Env) {
        let nid = js_to_node_id(&node_id);
        let mut state = self.doc.base.borrow_mut();
        // For text/comment nodes we update the existing data.
        let is_text = state
            .get_node(nid)
            .map(|n| n.is_text_node())
            .unwrap_or(false);
        if is_text {
            let mut mutator = state.mutate();
            mutator.set_node_text(nid, &text);
            drop(mutator);
            drop(state);
            self.doc.mark_host_dirty();
            return;
        }

        // Otherwise reset element children to a single text node.
        drop(state);
        self.doc.detach_children(nid, env).ok();
        let mut state = self.doc.base.borrow_mut();
        {
            let mut mutator = state.mutate();
            let text_id = mutator.create_text_node(&text);
            mutator.append_children(nid, &[text_id]);
        }
        drop(state);
        self.doc.mark_host_dirty();
    }
}

#[napi]
impl NativeDoc {
    /// Append `child` as the last child of `parent`. Mirrors `Node.appendChild`.
    #[napi]
    pub fn append_child(&mut self, parent_id: BigInt, child_id: BigInt, env: &Env) -> Result<()> {
        let child_nid = js_to_node_id(&child_id);
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        mutator.append_children(js_to_node_id(&parent_id), &[child_nid]);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        self.doc
            .make_in_document_subtree_strong(js_to_node_id(&parent_id), child_nid, env)?;
        Ok(())
    }

    /// Insert `node` immediately before `anchor`. If `anchor` is None, behaves
    /// like `appendChild`. Matches `Node.insertBefore`.
    #[napi]
    pub fn insert_before(
        &mut self,
        parent_id: BigInt,
        node_id: BigInt,
        anchor_id: Option<BigInt>,
        env: &Env,
    ) -> Result<()> {
        let nid = js_to_node_id(&node_id);
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        match anchor_id {
            Some(anchor) => {
                mutator.insert_nodes_before(js_to_node_id(&anchor), &[nid]);
            }
            None => {
                mutator.append_children(js_to_node_id(&parent_id), &[nid]);
            }
        }
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        // Switch the inserted subtree to strong refs if parent is in document.
        self.doc
            .make_in_document_subtree_strong(js_to_node_id(&parent_id), nid, env)?;
        Ok(())
    }

    /// Insert `node` immediately after `anchor`.
    #[napi]
    pub fn insert_after(&mut self, anchor_id: BigInt, node_id: BigInt, env: &Env) -> Result<()> {
        let nid = js_to_node_id(&node_id);
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        mutator.insert_nodes_after(js_to_node_id(&anchor_id), &[nid]);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        // Switch the inserted subtree to strong refs if parent is in document.
        self.doc
            .make_in_document_subtree_strong(js_to_node_id(&anchor_id), nid, env)?;
        Ok(())
    }

    /// Detach a node from its parent. The node is kept around (still
    /// addressable by id) so JS wrappers stay valid. Use `drop_node` to
    /// release storage.
    #[napi]
    pub fn remove(&mut self, node_id: BigInt, env: &Env) -> Result<()> {
        let nid = js_to_node_id(&node_id);
        // Switch to weak before removing, while parent chain is intact.
        self.doc.make_in_document_subtree_weak(nid, env)?;
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        mutator.remove_node(nid);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        Ok(())
    }

    /// Replace `anchor` with `node` in its parent.
    #[napi]
    pub fn replace_with(&mut self, anchor_id: BigInt, node_id: BigInt, env: &Env) -> Result<()> {
        let anchor_nid = js_to_node_id(&anchor_id);
        let node_nid = js_to_node_id(&node_id);
        // Switch the anchor to weak before detaching, while parent chain is intact.
        if let Err(e) = self.doc.make_in_document_subtree_weak(anchor_nid, env) {
            eprintln!("napi-blitz: make_in_document_subtree_weak failed: {e}");
        }
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        mutator.replace_node_with(anchor_nid, &[node_nid]);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        // The new node is now in document -> strong.
        self.doc
            .make_in_document_subtree_strong(node_nid, node_nid, env)?;
        Ok(())
    }

    /// Replace this element's inner HTML.
    #[napi]
    pub fn set_inner_html(&mut self, node_id: BigInt, html: String, env: &Env) {
        let nid = js_to_node_id(&node_id);
        self.doc.detach_children(nid, env).ok();
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        mutator.set_inner_html(nid, &html);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
    }

    /// Serialize this node (including the node itself) to HTML. Mirrors
    /// `Element.outerHTML`. Returns None for unknown nodes.
    #[napi]
    pub fn outer_html(&self, node_id: BigInt) -> Option<String> {
        let state = self.doc.base.borrow();
        state
            .get_node(js_to_node_id(&node_id))
            .map(|n| n.outer_html())
    }

    /// Serialize the children of this node to HTML, without the node's own
    /// open/close tags. Mirrors `Element.innerHTML`.
    #[napi]
    pub fn inner_html(&self, node_id: BigInt) -> Option<String> {
        let state = self.doc.base.borrow();
        let node = state.get_node(js_to_node_id(&node_id))?;
        let mut out = String::new();
        for &child_id in &node.children {
            if let Some(child) = state.get_node(child_id) {
                child.write_outer_html(&mut out);
            }
        }
        Some(out)
    }

    // -- Fast tree lookups --------------------------------------------------
    //
    // These bypass the CSS selector engine entirely. We run a pre-order
    // DFS over the document tree (using `Node.children` + `get_node`,
    // both pub) and short-circuit on the first match. blitz has an
    // internal `TreeTraverser` that does the same thing, but it isn't
    // re-exported from `blitz::dom`; our hand-rolled walk is
    // equivalent in cost.
    //
    // Document-scoped lookups start at node 0 (the document root).
    // Element-scoped lookups (`*_in`) start at the element's children,
    // matching the spec: `element.getElementsByTagName` does not return
    // the element itself.

    /// First element id matching the given local tag name (lowercase),
    /// or None if no element matches. Pre-order traversal from the
    /// document root.
    #[napi]
    pub fn find_first_by_local_name(&self, name: String) -> Option<u64> {
        let state = self.doc.base.borrow();
        let needle = LocalName::from(name.as_str());
        dfs_find(&state, state.root_node().id, |n| {
            n.data.is_element_with_tag_name(&needle)
        })
        .map(|id| id.as_u64())
    }

    /// All element ids matching the given local tag name, in tree order.
    /// Mirrors `getElementsByTagName(name)` minus the live-collection
    /// semantics — JS gets a snapshot.
    #[napi]
    pub fn find_all_by_local_name<'a>(&self, name: String, env: &'a Env) -> Vec<Object<'a>> {
        let state = self.doc.base.borrow();
        let needle = LocalName::from(name.as_str());
        let ids = dfs_collect(&state, state.root_node().id, |n| {
            n.data.is_element_with_tag_name(&needle)
        });
        drop(state);
        ids.into_iter()
            .filter_map(|id| wrap_node(&self.doc, id, env).ok())
            .collect()
    }

    /// All element ids matching the given local tag name, scoped to the
    /// subtree rooted at `root_id` (exclusive — `root_id` itself is not
    /// checked). Pre-order DFS from `root_id`'s children.
    #[napi]
    pub fn find_all_by_local_name_in<'a>(
        &self,
        root: &NativeNode,
        name: String,
        env: &'a Env,
    ) -> Vec<Object<'a>> {
        let state = self.doc.base.borrow();
        let needle = LocalName::from(name.as_str());
        let ids = dfs_collect_children(&state, root.node_id, |n| {
            n.data.is_element_with_tag_name(&needle)
        });
        drop(state);
        ids.into_iter()
            .filter_map(|id| wrap_node(&self.doc, id, env).ok())
            .collect()
    }

    /// All element ids in the subtree rooted at `root_id` (exclusive),
    /// i.e. every descendant element regardless of tag. Backs
    /// `element.getElementsByTagName("*")`.
    #[napi]
    pub fn find_all_elements_in<'a>(&self, root: &NativeNode, env: &'a Env) -> Vec<Object<'a>> {
        let state = self.doc.base.borrow();
        let ids = dfs_collect_children(&state, root.node_id, |n| {
            n.data.downcast_element().is_some()
        });
        drop(state);
        ids.into_iter()
            .filter_map(|id| wrap_node(&self.doc, id, env).ok())
            .collect()
    }

    /// All element ids whose `class` attribute contains `class_name` as
    /// one of its whitespace-separated tokens. Document-scoped.
    #[napi]
    pub fn find_all_by_class_name<'a>(&self, class_name: String, env: &'a Env) -> Vec<Object<'a>> {
        let state = self.doc.base.borrow();
        let needle = class_name;
        let ids = dfs_collect(&state, state.root_node().id, |n| node_has_class(n, &needle));
        drop(state);
        ids.into_iter()
            .filter_map(|id| wrap_node(&self.doc, id, env).ok())
            .collect()
    }

    /// All element ids whose `class` attribute contains `class_name`,
    /// scoped to the subtree rooted at `root_id` (exclusive).
    #[napi]
    pub fn find_all_by_class_name_in<'a>(
        &self,
        root: &NativeNode,
        class_name: String,
        env: &'a Env,
    ) -> Vec<Object<'a>> {
        let state = self.doc.base.borrow();
        let needle = class_name;
        let ids = dfs_collect_children(&state, root.node_id, |n| node_has_class(n, &needle));
        drop(state);
        ids.into_iter()
            .filter_map(|id| wrap_node(&self.doc, id, env).ok())
            .collect()
    }

    /// `<html>` element. Uses the `local_name!` macro for a zero-cost
    /// atom comparison. Returns None for documents without an `<html>`
    /// root (unusual but possible during partial parsing).
    #[napi]
    pub fn html_element<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let id = self.find_first_static(local_name!("html"))?;
        wrap_node(&self.doc, id, env).ok()
    }

    #[napi]
    pub fn head_element<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let id = self.find_first_static(local_name!("head"))?;
        wrap_node(&self.doc, id, env).ok()
    }

    #[napi]
    pub fn body_element<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let id = self.find_first_static(local_name!("body"))?;
        wrap_node(&self.doc, id, env).ok()
    }
}

impl NativeDoc {
    /// Shared fast-path for `local_name!`-constructed atoms. Bypasses the
    /// `LocalName::from(&str)` allocation that `find_first_by_local_name`
    /// has to do for the runtime-string case.
    fn find_first_static(&self, needle: LocalName) -> Option<NodeId> {
        let state = self.doc.base.borrow();
        dfs_find(&state, state.root_node().id, |n| {
            n.data.is_element_with_tag_name(&needle)
        })
    }
}

// --- Pre-order DFS helpers -----------------------------------------------
//
// These mirror blitz's internal `TreeTraverser` (which isn't pub-exported).
// `BaseDocument::get_node` + `Node.children` are both pub, so the walk
// costs the same as the upstream version: a Vec-backed stack with reversed
// children pushed per node.
//
// `dfs_find` / `dfs_collect` start at `root` and include `root` in the
// traversal. `dfs_collect_children` starts at `root`'s children, excluding
// `root` itself — for element-scoped lookups where the spec says the
// element itself is not part of the result.

/// Check whether a node's `class` attribute contains `class_name` as one
/// of its whitespace-separated tokens. Returns false for non-elements.
fn node_has_class(node: &blitz::dom::Node, class_name: &str) -> bool {
    let Some(class_str) = node.attr(local_name!("class")) else {
        return false;
    };
    class_str.split_whitespace().any(|c| c == class_name)
}

/// Find the first node id (pre-order, starting from `root` inclusive)
/// where `pred` returns true.
fn dfs_find<F>(doc: &BaseDocument, root: NodeId, pred: F) -> Option<NodeId>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let mut stack: Vec<NodeId> = vec![root];
    while let Some(id) = stack.pop() {
        let node = doc.get_node(id)?;
        if pred(node) {
            return Some(id);
        }
        for &child in node.children.iter().rev() {
            stack.push(child);
        }
    }
    None
}

/// Collect every node id (pre-order, starting from `root` inclusive)
/// where `pred` returns true.
fn dfs_collect<F>(doc: &BaseDocument, root: NodeId, pred: F) -> Vec<NodeId>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = doc.get_node(id) else {
            break;
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

/// Collect every node id (pre-order, starting from `root`'s children,
/// excluding `root` itself) where `pred` returns true. Used for
/// element-scoped lookups.
fn dfs_collect_children<F>(doc: &BaseDocument, root: NodeId, pred: F) -> Vec<NodeId>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let root_node = match doc.get_node(root) {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut stack: Vec<NodeId> = root_node.children.iter().rev().copied().collect();
    let mut out = Vec::new();
    while let Some(id) = stack.pop() {
        let Some(node) = doc.get_node(id) else {
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
