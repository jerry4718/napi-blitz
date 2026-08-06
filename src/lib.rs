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

mod dom;

mod dialog;

#[cfg(feature = "buffer-surface")]
mod buffer_surface;

#[cfg(feature = "native-window")]
mod native_window;
mod renderer;
