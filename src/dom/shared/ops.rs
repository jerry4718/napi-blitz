//! Small shared helpers for node/element operations: name construction,
//! detached-node attribute mutation, inline-style invalidation, and the
//! plain attribute/rect payload structs.

use blitz::dom::{BaseDocument, LocalName, Namespace, NodeId, QualName, local_name, ns};
use napi_derive::napi;
use style::{Atom, invalidation::element::restyle_hints::RestyleHint};

/// Plain attribute pair used by the create/insert APIs.
#[napi(object)]
pub struct AttrInit {
    pub name: String,
    pub value: String,
    pub namespace: Option<String>,
}

#[napi(object)]
pub struct DomRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

pub(crate) fn make_qual_name(local: &str, namespace: Option<&str>) -> QualName {
    QualName {
        prefix: None,
        ns: namespace.map(Namespace::from).unwrap_or(ns!(html)),
        local: LocalName::from(local),
    }
}

/// Mark a node as needing style/layout work after mutating its inline
/// declaration block through blitz's style-property helpers (the upstream
/// mutator skips the invalidation pieces).
pub(crate) fn mark_inline_style_mutated(state: &mut BaseDocument, node_id: NodeId) {
    state.snapshot_node(node_id);
    if let Some(node) = state.get_node_mut(node_id) {
        node.set_restyle_hint(RestyleHint::RESTYLE_STYLE_ATTRIBUTE);
    }
}

/// Set an attribute on a detached (never-styled) node without entering
/// style invalidation.
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
    let Some(element) = node.element_data_mut() else {
        return false;
    };
    if name.local == local_name!("id") {
        element.id = None;
    }
    element.attrs.remove(name);
    true
}
