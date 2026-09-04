//! The `Comment` layer — a comment node under `CharacterData`.
//!
//! The textual-data API (`data`, `length`, `appendData`, ...) lives on the
//! `CharacterData` parent layer; `Comment` only fixes the class identity.
//! blitz keeps no comment payload, so the standard-visible value reads back
//! as whatever blitz retains for comment nodes.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::character_data::CharacterDataLayer;

/// Own block of the `Comment` class.
#[layer]
pub struct CommentLayer {}

#[layer(js_name = "Comment")]
impl CommentLayer {
    #[layer(parent)]
    type Parent = CharacterDataLayer;

    #[layer(constructor)]
    fn build(_sup: Super<CharacterDataLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Comment cannot be constructed directly; use document.createComment",
        ))
    }
}
