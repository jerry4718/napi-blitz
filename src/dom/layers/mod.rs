//! DOM node layers for the napi runtime, built on `napi-inherit`'s
//! `#[layer]` macro. The hierarchy mirrors the boa-gui-runtime design:
//! `EventTarget → Node → Element → HTMLElement → HTMLInputElement /
//! HTMLTextAreaElement`, plus `Text`, `Comment`, and
//! `Document → HTMLDocument` nodes.

pub(crate) mod comment;
pub(crate) mod document;
pub(crate) mod element;
pub(crate) mod html_document;
pub(crate) mod html_element;
pub(crate) mod html_input_element;
pub(crate) mod html_textarea_element;
pub(crate) mod node;
pub(crate) mod text;

pub use comment::CommentLayer;
pub use document::DocumentLayer;
pub use element::ElementLayer;
pub use html_document::HTMLDocumentLayer;
pub use html_element::HTMLElementLayer;
pub use html_input_element::HTMLInputElementLayer;
pub use html_textarea_element::HTMLTextAreaElementLayer;
pub use node::{NodeLayer, NodeState};
pub use text::TextLayer;
