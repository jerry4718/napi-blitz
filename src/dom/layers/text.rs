//! The `Text` layer — a text node under `CharacterData`.
//!
//! The textual-data API (`data`, `length`, `appendData`, ...) lives on the
//! `CharacterData` parent layer; `Text` only fixes the class identity.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::character_data::CharacterDataLayer;

/// Own block of the `Text` class.
#[layer]
pub struct TextLayer {}

#[layer(js_name = "Text")]
impl TextLayer {
    #[layer(parent)]
    type Parent = CharacterDataLayer;

    #[layer(constructor)]
    fn build(_sup: Super<CharacterDataLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Text cannot be constructed directly; use document.createTextNode",
        ))
    }
}
