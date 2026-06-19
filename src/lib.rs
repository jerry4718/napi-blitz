#![deny(clippy::all)]
//! napi-blitz / wasm-blitz shared Rust backend around the [`blitz`] HTML/CSS engine.
//!
//! Architecture boundaries:
//! - [`dom`] exposes the shared nodeId-based document and event bridge used by
//!   every host package.
//! - [`native_window`] owns the winit/native-window path exported by
//!   `@ylcc/napi-blitz`.
//! - [`buffer_surface`] owns the headless RGBA frame path exported by
//!   `@ylcc/wasm-blitz`.

#[cfg(feature = "buffer-surface")]
mod buffer_surface;
mod dom;
#[cfg(feature = "native-window")]
mod native_window;

#[cfg(feature = "buffer-surface")]
pub use buffer_surface::{BufferFrame, BufferRenderer, BufferRendererOptions};
pub use dom::*;
pub use dom::{DocHandle, DocHandleConfig};
#[cfg(feature = "native-window")]
pub use native_window::{AppDispatchResult, AppEventPayload, BlitzApp, Window};
