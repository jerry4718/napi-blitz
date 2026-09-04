//! DOM layer system for napi-blitz, restructured on `napi-inherit`'s
//! `#[layer]` macro (the wintertc-events approach).
//!
//! Class hierarchy:
//!
//! ```text
//! EventTarget                      (wintertc-events)
//! └── Node
//!     ├── Element
//!     │   └── HTMLElement
//!     │       ├── HTMLInputElement
//!     │       └── HTMLTextAreaElement
//!     ├── Text
//!     ├── Comment
//!     └── Document
//!         └── HTMLDocument
//!
//! Event                            (wintertc-events)
//! └── UIEvent
//!     ├── MouseEvent
//!     │   └── PointerEvent
//!     ├── WheelEvent
//!     ├── KeyboardEvent
//!     ├── CompositionEvent
//!     └── FocusEvent
//! ```
//!
//! Each layer keeps its mutable state in a `state` field (one struct per
//! layer), mirroring how `EventLayer` separates the mutable dispatch state
//! from the immutable configuration fields, so a `state` can later be
//! wrapped in a `RefCell`.

// `#[layer]` expands to paths under `napi_inherit::`, which lives here via
// the `napi_helpers::inherit` re-export.
use napi_helpers::inherit as napi_inherit;

mod dispatch;
mod layers;
mod shared;

pub use layers::*;
pub use shared::create_document;
