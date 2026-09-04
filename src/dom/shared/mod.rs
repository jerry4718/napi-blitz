pub(crate) mod doc;
pub(crate) mod node_cache;
pub(crate) mod ops;

pub use doc::create_document;
pub(crate) use doc::{SharedDoc, WindowDocument, wrap_node};
