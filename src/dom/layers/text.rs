//! The `Text` layer — a text node directly under `Node`.

use std::rc::Rc;

use blitz::dom::NodeId;
use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::node::NodeLayer;
use crate::dom::shared::doc::SharedDocument;

/// Own block of the `Text` class.
#[layer]
pub struct TextLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
}

#[layer(js_name = "Text")]
impl TextLayer {
    #[layer(parent)]
    type Parent = NodeLayer;

    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Text cannot be constructed directly; use document.createTextNode",
        ))
    }

    #[layer(getter)]
    fn data(&self) -> String {
        self.text_of()
    }

    /// `CharacterData.appendData` — append to the node's text.
    #[layer]
    fn append_data(&mut self, data: String) {
        let text = format!("{}{}", self.text_of(), data);
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.set_node_text(self.node_id, &text);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }
}

impl TextLayer {
    fn text_of(&self) -> String {
        self.shared_doc
            .base()
            .get_node(self.node_id)
            .map(|n| n.text_content())
            .unwrap_or_default()
    }
}
