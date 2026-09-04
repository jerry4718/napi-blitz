//! The `Comment` layer — a comment node directly under `Node`.

use napi::{Error, Result, bindgen_prelude::Object};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event_target::EventTargetLayer;

use crate::layers::node::NodeLayer;

/// Own block of the `Comment` class.
#[layer(js_name = "Comment", parent = NodeLayer)]
pub struct CommentLayer {}

#[layer]
impl CommentLayer {
    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Comment cannot be constructed directly; use document.createComment",
        ))
    }
}
