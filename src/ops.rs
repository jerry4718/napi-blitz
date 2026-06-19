//! Flat, nodeId-based DOM operations exposed to JS as methods on `DocHandle`.
//!
//! Every method here takes node ids (`u64`) as input and returns plain
//! values. Element wrapper objects live entirely in the JS layer.

use blitz::dom::{Attribute as BlitzAttribute, LocalName, Namespace, QualName, local_name, ns};
use blitz::html::DocumentHtmlParser;
use napi::{Error, Result, bindgen_prelude::BigInt};
use napi_derive::napi;
use style::properties::PropertyId;

use crate::doc::DocHandle;

/// Plain attribute pair used by the create/insert APIs.
#[napi(object)]
pub struct AttrInit {
    pub name: String,
    pub value: String,
    pub namespace: Option<String>,
}

fn make_qual_name(local: &str, namespace: Option<&str>) -> QualName {
    QualName {
        prefix: None,
        ns: namespace.map(Namespace::from).unwrap_or(ns!(html)),
        local: LocalName::from(local),
    }
}

fn js_node_id_to_usize(id: &BigInt) -> usize {
    let (signed, value, lossless) = id.get_u64();
    if signed || !lossless {
        return usize::MAX;
    }
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[napi]
impl DocHandle {
    /// Replace document content from an HTML string. Useful for tests and
    /// initial bootstrapping when `base_html` was not enough.
    #[napi]
    pub fn load_html(&mut self, html: String) {
        let mut state = self.base.0.borrow_mut();
        {
            let mut mutator = state.mutate();
            DocumentHtmlParser::parse_into_mutator(&mut mutator, &html);
        }
        state.resolve(0.0);
    }

    /// Find a single node by CSS selector. Returns its node id or null.
    #[napi]
    pub fn query_selector(&self, selector: String) -> Result<Option<u64>> {
        let state = self.base.0.borrow();
        match state.query_selector(&selector) {
            Ok(Some(id)) => Ok(Some(id as u64)),
            Ok(None) => Ok(None),
            Err(err) => Err(Error::from_reason(format!("query_selector: {err:?}"))),
        }
    }

    /// Find all nodes by CSS selector. Returns a list of node ids.
    #[napi]
    pub fn query_selector_all(&self, selector: String) -> Result<Vec<u64>> {
        let state = self.base.0.borrow();
        match state.query_selector_all(&selector) {
            Ok(ids) => Ok(ids.into_iter().map(|id| id as u64).collect()),
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
        let state = self.base.0.borrow();
        let selector_list = state
            .try_parse_selector_list(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector_in: {err:?}")))?;

        let Some(root_node) = state.get_node(js_node_id_to_usize(&root_id)) else {
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
        Ok(result.map(|node| node.id as u64))
    }

    /// Element-scoped `querySelectorAll`: all matches in the subtree rooted
    /// at `root_id` (exclusive). Same approach as `query_selector_in`.
    #[napi]
    pub fn query_selector_all_in(&self, root_id: BigInt, selector: String) -> Result<Vec<u64>> {
        let state = self.base.0.borrow();
        let selector_list = state
            .try_parse_selector_list(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector_all_in: {err:?}")))?;

        let Some(root_node) = state.get_node(js_node_id_to_usize(&root_id)) else {
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
        Ok(results.iter().map(|node| node.id as u64).collect())
    }

    /// Lookup by `id=` attribute, like `document.getElementById`.
    #[napi]
    pub fn get_element_by_id(&self, id: String) -> Option<u64> {
        self.base
            .0
            .borrow()
            .get_element_by_id(&id)
            .map(|id| id as u64)
    }

    /// Find the document's `<title>` element id, or None if no title
    /// element exists in the tree. Uses the same pre-order DFS as the
    /// other structural lookups (`html`/`head`/`body`) — cheaper than
    /// `querySelector("title")` which dispatches through the CSS
    /// selector engine.
    #[napi]
    pub fn find_title_node_id(&self) -> Option<u64> {
        self.find_first_static(local_name!("title"))
    }

    /// True iff the given node id currently exists in the document.
    #[napi]
    pub fn has_node(&self, id: BigInt) -> bool {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&id))
            .is_some()
    }
}

#[napi]
impl DocHandle {
    /// Create an element node. Returns its node id. The element is detached
    /// (no parent) until inserted.
    #[napi]
    pub fn create_element(
        &mut self,
        local_name: String,
        namespace: Option<String>,
        attrs: Option<Vec<AttrInit>>,
    ) -> u64 {
        let mut state = self.base.0.borrow_mut();
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
        mutator.create_element(qn, attr_vec) as u64
    }

    /// Create a text node with the given content.
    #[napi]
    pub fn create_text_node(&mut self, text: String) -> u64 {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.create_text_node(&text) as u64
    }

    /// Create an empty comment node.
    #[napi]
    pub fn create_comment_node(&mut self) -> u64 {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.create_comment_node() as u64
    }

    /// Deep-clone an existing node and return the new node's id.
    #[napi]
    pub fn deep_clone_node(&mut self, node_id: BigInt) -> u64 {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.deep_clone_node(js_node_id_to_usize(&node_id)) as u64
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
        let mut state = self.base.0.borrow_mut();
        let Some(source) = state.get_node(js_node_id_to_usize(&node_id)) else {
            return 0;
        };
        // Cloning `NodeData` deep-copies attributes, text, and the
        // (Arc-shared) parsed `style` declaration block. We never
        // touch `children` / `parent` so the clone starts detached.
        let data = source.data.clone();
        state.create_node(data) as u64
    }
}

#[napi]
impl DocHandle {
    /// Parent node id, if any.
    #[napi]
    pub fn parent_id(&self, node_id: BigInt) -> Option<u64> {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&node_id))
            .and_then(|n| n.parent)
            .map(|id| id as u64)
    }

    /// First child id, if any.
    #[napi]
    pub fn first_child_id(&self, node_id: BigInt) -> Option<u64> {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&node_id))
            .and_then(|n| n.children.first().copied())
            .map(|id| id as u64)
    }

    /// Last child id, if any.
    #[napi]
    pub fn last_child_id(&self, node_id: BigInt) -> Option<u64> {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&node_id))
            .and_then(|n| n.children.last().copied())
            .map(|id| id as u64)
    }

    /// All children, in document order.
    #[napi]
    pub fn child_ids(&self, node_id: BigInt) -> Vec<u64> {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&node_id))
            .map(|n| n.children.iter().map(|id| *id as u64).collect())
            .unwrap_or_default()
    }

    /// Next sibling id, if any.
    #[napi]
    pub fn next_sibling_id(&self, node_id: BigInt) -> Option<u64> {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&node_id))
            .and_then(|n| n.forward(1))
            .map(|n| n.id as u64)
    }

    /// Previous sibling id, if any.
    #[napi]
    pub fn previous_sibling_id(&self, node_id: BigInt) -> Option<u64> {
        self.base
            .0
            .borrow()
            .get_node(js_node_id_to_usize(&node_id))
            .and_then(|n| n.backward(1))
            .map(|n| n.id as u64)
    }
}

/// Mirrors web NodeType numeric codes for the small subset blitz exposes.
const NODE_TYPE_ELEMENT: u32 = 1;
const NODE_TYPE_TEXT: u32 = 3;
const NODE_TYPE_COMMENT: u32 = 8;
const NODE_TYPE_DOCUMENT: u32 = 9;
const NODE_TYPE_OTHER: u32 = 0;

#[napi]
impl DocHandle {
    /// DOM-style `nodeType` (1=Element, 3=Text, 8=Comment, 9=Document).
    #[napi]
    pub fn node_type(&self, node_id: BigInt) -> u32 {
        let state = self.base.0.borrow();
        let Some(node) = state.get_node(js_node_id_to_usize(&node_id)) else {
            return NODE_TYPE_OTHER;
        };
        use blitz::dom::NodeData;
        match &node.data {
            NodeData::Document => NODE_TYPE_DOCUMENT,
            NodeData::Element(_) => NODE_TYPE_ELEMENT,
            NodeData::Text(_) => NODE_TYPE_TEXT,
            NodeData::Comment => NODE_TYPE_COMMENT,
            _ => NODE_TYPE_OTHER,
        }
    }

    /// Local element tag name (lowercased), e.g. "div". Returns None for
    /// non-element nodes.
    #[napi]
    pub fn tag_name(&self, node_id: BigInt) -> Option<String> {
        let state = self.base.0.borrow();
        state
            .get_node(js_node_id_to_usize(&node_id))
            .and_then(|n| n.element_data())
            .map(|el| el.name.local.to_string())
    }

    /// Concatenated text content of this node and its descendants. Mirrors
    /// `Node.textContent`.
    #[napi]
    pub fn text_content(&self, node_id: BigInt) -> Option<String> {
        let state = self.base.0.borrow();
        state
            .get_node(js_node_id_to_usize(&node_id))
            .map(|n| n.text_content())
    }

    /// Get an attribute value, or None if missing or node is not an element.
    #[napi]
    pub fn get_attribute(&self, node_id: BigInt, name: String) -> Option<String> {
        let state = self.base.0.borrow();
        let node = state.get_node(js_node_id_to_usize(&node_id))?;
        let local = LocalName::from(name.as_str());
        node.attr(local).map(|s| s.to_string())
    }

    /// All attribute (name, value) pairs on this node, or empty if not an
    /// element.
    #[napi]
    pub fn get_attributes(&self, node_id: BigInt) -> Vec<AttrInit> {
        let state = self.base.0.borrow();
        let Some(node) = state.get_node(js_node_id_to_usize(&node_id)) else {
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
impl DocHandle {
    /// Set an attribute on an element.
    #[napi]
    pub fn set_attribute(
        &mut self,
        node_id: BigInt,
        name: String,
        value: String,
        namespace: Option<String>,
    ) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.set_attribute(
            js_node_id_to_usize(&node_id),
            make_qual_name(&name, namespace.as_deref()),
            &value,
        );
    }

    /// Remove an attribute from an element.
    #[napi]
    pub fn remove_attribute(&mut self, node_id: BigInt, name: String, namespace: Option<String>) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.clear_attribute(
            js_node_id_to_usize(&node_id),
            make_qual_name(&name, namespace.as_deref()),
        );
    }

    /// Set a single inline style property (e.g. "color", "#ff0000").
    #[napi]
    pub fn set_style_property(&mut self, node_id: BigInt, name: String, value: String) {
        let mut state = self.base.0.borrow_mut();
        state.set_style_property(js_node_id_to_usize(&node_id), &name, &value);
    }

    /// Remove a single inline style property.
    #[napi]
    pub fn remove_style_property(&mut self, node_id: BigInt, name: String) {
        let mut state = self.base.0.borrow_mut();
        state.remove_style_property(js_node_id_to_usize(&node_id), &name);
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
        let state = self.base.0.borrow();
        let element_data = state
            .get_node(js_node_id_to_usize(&node_id))?
            .element_data()?;
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
        let state = self.base.0.borrow();
        let Some(element_data) = state
            .get_node(js_node_id_to_usize(&node_id))
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
        let state = self.base.0.borrow();
        let Some(element_data) = state
            .get_node(js_node_id_to_usize(&node_id))
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
    pub fn set_text_content(&mut self, node_id: BigInt, text: String) {
        let mut state = self.base.0.borrow_mut();
        // For text/comment nodes we update the existing data.
        let is_text = state
            .get_node(js_node_id_to_usize(&node_id))
            .map(|n| n.is_text_node())
            .unwrap_or(false);
        if is_text {
            let mut mutator = state.mutate();
            mutator.set_node_text(js_node_id_to_usize(&node_id), &text);
            return;
        }

        // Otherwise reset element children to a single text node.
        let children: Vec<usize> = state
            .get_node(js_node_id_to_usize(&node_id))
            .map(|n| n.children.clone())
            .unwrap_or_default();
        {
            let mut mutator = state.mutate();
            for c in &children {
                mutator.remove_and_drop_node(*c);
            }
            let text_id = mutator.create_text_node(&text);
            mutator.append_children(js_node_id_to_usize(&node_id), &[text_id]);
        }
    }
}

#[napi]
impl DocHandle {
    /// Append `child` as the last child of `parent`. Mirrors `Node.appendChild`.
    #[napi]
    pub fn append_child(&mut self, parent_id: BigInt, child_id: BigInt) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.append_children(
            js_node_id_to_usize(&parent_id),
            &[js_node_id_to_usize(&child_id)],
        );
    }

    /// Insert `node` immediately before `anchor`. If `anchor` is None, behaves
    /// like `appendChild`. Matches `Node.insertBefore`.
    #[napi]
    pub fn insert_before(&mut self, parent_id: BigInt, node_id: BigInt, anchor_id: Option<BigInt>) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        match anchor_id {
            Some(anchor) => {
                mutator.insert_nodes_before(
                    js_node_id_to_usize(&anchor),
                    &[js_node_id_to_usize(&node_id)],
                );
            }
            None => {
                mutator.append_children(
                    js_node_id_to_usize(&parent_id),
                    &[js_node_id_to_usize(&node_id)],
                );
            }
        }
    }

    /// Insert `node` immediately after `anchor`.
    #[napi]
    pub fn insert_after(&mut self, anchor_id: BigInt, node_id: BigInt) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.insert_nodes_after(
            js_node_id_to_usize(&anchor_id),
            &[js_node_id_to_usize(&node_id)],
        );
    }

    /// Detach a node from its parent. The node is kept around (still
    /// addressable by id) so JS wrappers stay valid. Use `drop_node` to
    /// release storage.
    #[napi]
    pub fn remove(&mut self, node_id: BigInt) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.remove_node(js_node_id_to_usize(&node_id));
    }

    /// Detach and free the node.
    #[napi]
    pub fn drop_node(&mut self, node_id: BigInt) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.remove_and_drop_node(js_node_id_to_usize(&node_id));
    }

    /// Replace `anchor` with `node` in its parent.
    #[napi]
    pub fn replace_with(&mut self, anchor_id: BigInt, node_id: BigInt) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.replace_node_with(
            js_node_id_to_usize(&anchor_id),
            &[js_node_id_to_usize(&node_id)],
        );
    }

    /// Replace this element's inner HTML.
    #[napi]
    pub fn set_inner_html(&mut self, node_id: BigInt, html: String) {
        let mut state = self.base.0.borrow_mut();
        let mut mutator = state.mutate();
        mutator.set_inner_html(js_node_id_to_usize(&node_id), &html);
    }

    /// Serialize this node (including the node itself) to HTML. Mirrors
    /// `Element.outerHTML`. Returns None for unknown nodes.
    #[napi]
    pub fn outer_html(&self, node_id: BigInt) -> Option<String> {
        let state = self.base.0.borrow();
        state
            .get_node(js_node_id_to_usize(&node_id))
            .map(|n| n.outer_html())
    }

    /// Serialize the children of this node to HTML, without the node's own
    /// open/close tags. Mirrors `Element.innerHTML`.
    #[napi]
    pub fn inner_html(&self, node_id: BigInt) -> Option<String> {
        let state = self.base.0.borrow();
        let node = state.get_node(js_node_id_to_usize(&node_id))?;
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
        let state = self.base.0.borrow();
        let needle = LocalName::from(name.as_str());
        dfs_find(&state, 0, |n| n.data.is_element_with_tag_name(&needle)).map(|id| id as u64)
    }

    /// All element ids matching the given local tag name, in tree order.
    /// Mirrors `getElementsByTagName(name)` minus the live-collection
    /// semantics — JS gets a snapshot.
    #[napi]
    pub fn find_all_by_local_name(&self, name: String) -> Vec<u64> {
        let state = self.base.0.borrow();
        let needle = LocalName::from(name.as_str());
        dfs_collect(&state, 0, |n| n.data.is_element_with_tag_name(&needle))
            .into_iter()
            .map(|id| id as u64)
            .collect()
    }

    /// All element ids matching the given local tag name, scoped to the
    /// subtree rooted at `root_id` (exclusive — `root_id` itself is not
    /// checked). Pre-order DFS from `root_id`'s children.
    #[napi]
    pub fn find_all_by_local_name_in(&self, root_id: BigInt, name: String) -> Vec<u64> {
        let state = self.base.0.borrow();
        let needle = LocalName::from(name.as_str());
        dfs_collect_children(&state, js_node_id_to_usize(&root_id), |n| {
            n.data.is_element_with_tag_name(&needle)
        })
        .into_iter()
        .map(|id| id as u64)
        .collect()
    }

    /// All element ids in the subtree rooted at `root_id` (exclusive),
    /// i.e. every descendant element regardless of tag. Backs
    /// `element.getElementsByTagName("*")`.
    #[napi]
    pub fn find_all_elements_in(&self, root_id: BigInt) -> Vec<u64> {
        let state = self.base.0.borrow();
        dfs_collect_children(&state, js_node_id_to_usize(&root_id), |n| {
            n.data.downcast_element().is_some()
        })
        .into_iter()
        .map(|id| id as u64)
        .collect()
    }

    /// All element ids whose `class` attribute contains `class_name` as
    /// one of its whitespace-separated tokens. Document-scoped.
    #[napi]
    pub fn find_all_by_class_name(&self, class_name: String) -> Vec<u64> {
        let state = self.base.0.borrow();
        let needle = class_name;
        dfs_collect(&state, 0, |n| node_has_class(n, &needle))
            .into_iter()
            .map(|id| id as u64)
            .collect()
    }

    /// All element ids whose `class` attribute contains `class_name`,
    /// scoped to the subtree rooted at `root_id` (exclusive).
    #[napi]
    pub fn find_all_by_class_name_in(&self, root_id: BigInt, class_name: String) -> Vec<u64> {
        let state = self.base.0.borrow();
        let needle = class_name;
        dfs_collect_children(&state, js_node_id_to_usize(&root_id), |n| {
            node_has_class(n, &needle)
        })
        .into_iter()
        .map(|id| id as u64)
        .collect()
    }

    /// `<html>` element id. Uses the `local_name!` macro for a zero-cost
    /// atom comparison. Returns None for documents without an `<html>`
    /// root (unusual but possible during partial parsing).
    #[napi]
    pub fn html_element_id(&self) -> Option<u64> {
        self.find_first_static(local_name!("html"))
    }

    /// `<head>` element id, or None if missing.
    #[napi]
    pub fn head_element_id(&self) -> Option<u64> {
        self.find_first_static(local_name!("head"))
    }

    /// `<body>` element id, or None if missing.
    #[napi]
    pub fn body_element_id(&self) -> Option<u64> {
        self.find_first_static(local_name!("body"))
    }
}

impl DocHandle {
    /// Shared fast-path for `local_name!`-constructed atoms. Bypasses the
    /// `LocalName::from(&str)` allocation that `find_first_by_local_name`
    /// has to do for the runtime-string case.
    fn find_first_static(&self, needle: LocalName) -> Option<u64> {
        let state = self.base.0.borrow();
        dfs_find(&state, 0, |n| n.data.is_element_with_tag_name(&needle)).map(|id| id as u64)
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

use blitz::dom::BaseDocument;

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
fn dfs_find<F>(doc: &BaseDocument, root: usize, pred: F) -> Option<usize>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let mut stack: Vec<usize> = vec![root];
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
fn dfs_collect<F>(doc: &BaseDocument, root: usize, pred: F) -> Vec<usize>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let mut out = Vec::new();
    let mut stack: Vec<usize> = vec![root];
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
fn dfs_collect_children<F>(doc: &BaseDocument, root: usize, pred: F) -> Vec<usize>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let root_node = match doc.get_node(root) {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut stack: Vec<usize> = root_node.children.iter().rev().copied().collect();
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
