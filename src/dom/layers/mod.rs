//! DOM node layers for the napi runtime, built on `napi-inherit`'s
//! `#[layer]` macro. The hierarchy mirrors the boa-gui-runtime design:
//! `EventTarget → Node → Element → HTMLElement → HTMLHtmlElement /
//! HTMLInputElement / HTMLTextAreaElement`, plus `Text`, `Comment`, and
//! `Document → HTMLDocument` nodes.

// Module order matters for the `#[layer]` extends resolution: the parent
// layer's struct must be expanded before the child's impl (the layer
// registry is build-order dependent). List parents before their children.
pub(crate) mod node;
pub(crate) mod comment;
pub(crate) mod text;
pub(crate) mod element;
pub(crate) mod html_element;
pub(crate) mod html_html_element;
pub(crate) mod html_input_element;
pub(crate) mod html_textarea_element;
pub(crate) mod document;
pub(crate) mod html_document;
pub(crate) mod style_handler;
pub(crate) mod attributes_handler;

pub use comment::CommentLayer;
pub use document::DocumentLayer;
pub use element::ElementLayer;
pub use html_document::HTMLDocumentLayer;
pub use html_element::HTMLElementLayer;
pub use html_html_element::HTMLHtmlElementLayer;
pub use html_input_element::HTMLInputElementLayer;
pub use html_textarea_element::HTMLTextAreaElementLayer;
pub use node::NodeLayer;
pub use text::TextLayer;
