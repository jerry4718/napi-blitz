//! ES-style class inheritance infrastructure.
//!
//! Every layer of an inheritance chain is a plain Rust struct (an
//! [`ExtendLayer`]). Instances hold no direct fields: each layer's data lives
//! in a per-instance [`OwnDataRegistry`] attached once via `napi_wrap`, one
//! fixed-size slot per layer addressed by the compile-time
//! [`OwnBlock::IDX`]. Classes are plain function constructors whose
//! prototypes are linked with `Object.setPrototypeOf`.
//!
//! Three instantiation paths:
//! - JS `new X(...)`: the constructor callback runs `emit_own`, building each
//!   layer bottom-up through the `Super` hook (type-enforced: `Constructed`
//!   can only be assembled from the `SuperDone` receipt).
//! - Rust with an existing data chain: `new_from_chain` + `populate_chain`
//!   (fields move straight into the own blocks, parent first).

mod class;
mod layer;
mod macros;
mod own;
mod registry;

pub use napi_inherit_proc as proc;

pub use class::*;
pub use layer::*;
pub use own::*;
pub use registry::*;
