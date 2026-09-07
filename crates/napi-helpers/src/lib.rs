//! Small building blocks on top of napi that the higher layers
//! (napi-inherit, wintertc-events, napi-blitz-dom, ...) share.

pub mod anything;
pub mod deferred;
pub mod log;
pub mod proxy;

mod inherit;
mod refs;

pub mod inherits {
    pub use super::inherit::*;
    pub use crate::from_chain;
    pub use crate::layer_chain;
    pub use crate::refs::LayerRef;
    pub use napi_inherit_proc as proc;
}

pub use deferred::Deferred;
pub use refs::{Finalize, ToggleRef, WeakRef, finalize_trampoline};
