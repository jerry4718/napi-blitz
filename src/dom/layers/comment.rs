//! The `Comment` layer — a comment node directly under `Node`.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::node::NodeLayer;

/// Own block of the `Comment` class.
#[layer(js_name = "Comment")]
pub struct CommentLayer {}

#[layer]
impl CommentLayer {
    #[layer(parent)]
    type Parent = NodeLayer;

    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Comment cannot be constructed directly; use document.createComment",
        ))
    }
}
