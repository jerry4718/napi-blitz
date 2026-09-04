//! The `Text` layer — a text node directly under `Node`.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::layers::node::NodeLayer;

/// Own block of the `Text` class.
#[layer(js_name = "Text", parent = NodeLayer)]
pub struct TextLayer {}

#[layer]
impl TextLayer {
    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Text cannot be constructed directly; use document.createTextNode",
        ))
    }
}
