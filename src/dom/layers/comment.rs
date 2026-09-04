//! The `Comment` layer — a comment node directly under `Node`.
//!
//! blitz keeps no comment payload, so `data` always reads as the empty
//! string and assignments are accepted and dropped.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::node::NodeLayer;

/// Own block of the `Comment` class.
#[layer]
pub struct CommentLayer {}

#[layer(js_name = "Comment")]
impl CommentLayer {
    #[layer(parent)]
    type Parent = NodeLayer;

    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Comment cannot be constructed directly; use document.createComment",
        ))
    }

    /// blitz keeps no comment payload; the standard-visible value is empty.
    #[layer(getter)]
    fn data(&self) -> String {
        // todo!( " real " )
        String::new()
    }

    /// Accepted per the standard API; blitz drops the content.
    #[layer(setter)]
    fn set_data(&self, _data: String) {}
}
