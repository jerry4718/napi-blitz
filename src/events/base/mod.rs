//! DOM Event system built on `napi-inherit`'s `#[layer]` macro.
//!
//! The event class hierarchy follows the DOM standard:
//!
//! ```text
//! EventTarget
//! Event
//! ├── CustomEvent (detail)
//! └── MessageEvent (data, origin)
//! ```
//!
//! Each layer is a plain Rust struct declared with `#[layer]`; instance data
//! lives in the per-instance `OwnDataRegistry` (see `napi-inherit`). Listeners
//! are stored and dispatched in `event_target`.

mod dispatch;

mod event;
mod event_target;
mod message_event;
mod custom_event;

pub use custom_event::*;
pub use event::*;
pub use event_target::*;
pub use message_event::*;
