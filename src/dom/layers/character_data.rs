//! The `CharacterData` layer — abstract base of `Text` and `Comment`.
//! Carries the textual-data API on top of `Node`: `data`, `length`,
//! `appendData`, and the element-sibling lookups.

use std::rc::Rc;

use blitz::dom::{NodeData, NodeId};
use napi::{Env, Error, Result};
use napi_helpers::inherits::{Constructed, LayerRef, Super, proc::layer};

use crate::dom::{
    layers::{element::ElementLayer, node::NodeLayer},
    shared::{doc::SharedDocument, wrap_node},
};

/// Own block of the `CharacterData` class.
#[layer]
pub struct CharacterDataLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
}

#[layer(js_name = "CharacterData")]
impl CharacterDataLayer {
    #[layer(parent)]
    type Parent = NodeLayer;

    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "CharacterData is abstract; use Text or Comment instances",
        ))
    }

    #[layer(getter)]
    fn data(&self) -> String {
        self.text_of()
    }

    #[layer(setter)]
    fn set_data(&self, data: String) {
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.set_node_text(self.node_id, &data);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    /// `CharacterData.length` — the size of the string in UTF-16 code
    /// units, which is how the standard defines a string's length.
    #[layer(getter)]
    fn length(&self) -> u32 {
        self.text_of().encode_utf16().count() as u32
    }

    /// `CharacterData.nextElementSibling` — the first Element sibling after
    /// this node, skipping non-element siblings.
    // character_data.rs expands before element.rs registers the layer, so
    // the automatic `LayerRef<L>` mapping cannot resolve the JS name here.
    #[layer(getter, ts_return_type = "Element | null")]
    fn next_element_sibling(&self, env: &Env) -> Result<Option<LayerRef<ElementLayer>>> {
        self.element_sibling(env, true)
    }

    /// `CharacterData.previousElementSibling` — the first Element sibling
    /// before this node, skipping non-element siblings.
    #[layer(getter, ts_return_type = "Element | null")]
    fn previous_element_sibling(&self, env: &Env) -> Result<Option<LayerRef<ElementLayer>>> {
        self.element_sibling(env, false)
    }

    /// `CharacterData.appendData` — append to the node's text.
    #[layer]
    fn append_data(&self, data: String) {
        let text_of = self.text_of();

        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.set_node_text(self.node_id, &format!("{}{}", text_of, data));

        drop(mutator);
        drop(base);

        self.shared_doc.mark_host_dirty();
    }
}

impl CharacterDataLayer {
    fn text_of(&self) -> String {
        self.shared_doc
            .base()
            .get_node(self.node_id)
            .map(|n| n.text_content())
            .unwrap_or_default()
    }

    /// Walk the sibling axis in one direction and hand back the first
    /// element encountered, wrapped as an `Element` layer reference.
    fn element_sibling(&self, env: &Env, forward: bool) -> Result<Option<LayerRef<ElementLayer>>> {
        let base = self.shared_doc.base();
        let start = base
            .get_node(self.node_id)
            .and_then(|n| if forward { n.forward(1) } else { n.backward(1) });
        let mut cursor = start;
        while let Some(n) = cursor {
            if matches!(n.data, NodeData::Element(_)) {
                let id = n.id;
                drop(base);
                let node = wrap_node(&self.shared_doc, env, id)?;
                return Ok(Some(LayerRef::new(&node, env)?));
            }
            cursor = if forward { n.forward(1) } else { n.backward(1) };
        }
        Ok(None)
    }
}
