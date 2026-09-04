// ── wrap_node: materialize a JS wrapper for a blitz node ─────────────

use crate::{
    dom::{
        CharacterDataLayer, CommentLayer, DocumentLayer, ElementLayer, HTMLBodyElementLayer,
        HTMLDocumentLayer, HTMLElementLayer, HTMLHtmlElementLayer, HTMLInputElementLayer,
        HTMLTextAreaElementLayer, NodeLayer, TextLayer, layers::element::ElementState,
        shared::doc::SharedDocument,
    },
    events::base::EventTargetLayer,
};
use blitz::{
    dom::{NodeData, local_name, node::NodeKind},
    traits::NodeId,
};
use napi::{Env, Error, Status, bindgen_prelude::Object};
use napi_helpers::inherits::{from_chain, layer_chain};
use std::{cell::RefCell, rc::Rc};

/// Return the cached JS wrapper for `node_id`, or build the matching
/// `#[layer]` chain via `new_from_chain` and cache it.
///
/// Document nodes resolve to (and register) the JS Document object.
pub fn wrap_node<'a>(
    shared_doc: &Rc<SharedDocument>,
    env: &'a Env,
    node_id: NodeId,
) -> napi::Result<Object<'a>> {
    // 1. Return an existing JS wrapper only after confirming that the
    //    underlying DOM node still exists.
    if let Some(cached) = shared_doc.node_cache().get(node_id, env) {
        return Ok(cached);
    }

    // 2. Read the node metadata. Invalid or stale ids must not fall
    //    through to chain building as a made-up nodeType.
    let (node_kind, qual_name) = {
        let base = shared_doc.base();
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

    let base_layer = layer_chain!(
        EventTargetLayer::fresh(),
        NodeLayer {
            node_id,
            shared_doc: shared_doc.clone(),
        },
    );
    // 3. Document node: resolve to the JS Document object, creating and
    //    registering it on first access. The wrapper's lifetime anchor is
    //    `SharedDocument.document_ref` (two-state); the cache entry here
    //    only carries identity.
    if let NodeKind::Document = node_kind {
        if let Some(existing) = shared_doc
            .document_ref()
            .as_ref()
            .and_then(|r| r.get_value(env))
        {
            shared_doc.node_cache_mut().insert(
                node_id,
                &existing,
                env,
                shared_doc.cache_strength(node_id),
                Rc::downgrade(shared_doc),
            )?;
            return Ok(existing);
        }
        let document = from_chain!(
            (HTMLDocumentLayer, env),
            ..base_layer,
            DocumentLayer {
                shared: shared_doc.clone()
            },
            HTMLDocumentLayer {},
        )?;
        shared_doc.set_document_ref(env, &document)?;
        shared_doc.node_cache_mut().insert(
            node_id,
            &document,
            env,
            shared_doc.cache_strength(node_id),
            Rc::downgrade(shared_doc),
        )?;
        return Ok(document);
    }

    let js_node = match node_kind {
        NodeKind::Element => match qual_name.map(|qn| qn.local) {
            Some(local_name!("html")) => from_chain!(
                (HTMLHtmlElementLayer, env),
                ..base_layer,
                ElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                    style_proxy: RefCell::new(None),
                    attributes_proxy: RefCell::new(None),
                    state: ElementState::default(),
                },
                HTMLElementLayer {},
                HTMLHtmlElementLayer {},
            )?,
            Some(local_name!("body")) => from_chain!(
                (HTMLBodyElementLayer, env),
                ..base_layer,
                ElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                    style_proxy: RefCell::new(None),
                    attributes_proxy: RefCell::new(None),
                    state: ElementState::default(),
                },
                HTMLElementLayer {},
                HTMLBodyElementLayer {},
            )?,
            Some(local_name!("input")) => from_chain!(
                (HTMLInputElementLayer, env),
                ..base_layer,
                ElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                    style_proxy: RefCell::new(None),
                    attributes_proxy: RefCell::new(None),
                    state: ElementState::default(),
                },
                HTMLElementLayer {},
                HTMLInputElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                },
            )?,
            Some(local_name!("textarea")) => from_chain!(
                (HTMLTextAreaElementLayer, env),
                ..base_layer,
                ElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                    style_proxy: RefCell::new(None),
                    attributes_proxy: RefCell::new(None),
                    state: ElementState::default(),
                },
                HTMLElementLayer {},
                HTMLTextAreaElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                },
            )?,
            _ => from_chain!(
                (HTMLElementLayer, env),
                ..base_layer,
                ElementLayer {
                    node_id,
                    shared_doc: shared_doc.clone(),
                    style_proxy: RefCell::new(None),
                    attributes_proxy: RefCell::new(None),
                    state: ElementState::default(),
                },
                HTMLElementLayer {},
            )?,
        },
        NodeKind::Text => from_chain!(
            (TextLayer, env),
            ..base_layer,
            CharacterDataLayer {
                node_id,
                shared_doc: shared_doc.clone(),
            },
            TextLayer {},
        )?,
        NodeKind::Comment => from_chain!(
            (CommentLayer, env),
            ..base_layer,
            CharacterDataLayer {
                node_id,
                shared_doc: shared_doc.clone(),
            },
            CommentLayer {},
        )?,
        _ => {
            return Err(Error::new(
                Status::GenericFailure,
                format!("No layer for node_kind {node_kind:?} (node_id={node_id})"),
            ));
        }
    };

    // 5. Determine the reference strength from the window-live gate:
    //    wrappers of in-document nodes are pinned only while a window is
    //    attached and live.
    let strong = shared_doc.cache_strength(node_id);

    // 6. Cache the JS wrapper with the determined strength.
    shared_doc.node_cache_mut().insert(
        node_id,
        &js_node,
        env,
        strong,
        Rc::downgrade(shared_doc),
    )?;

    Ok(js_node)
}
