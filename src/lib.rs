//! napi-blitz / wasm-blitz shared Rust backend around the [`blitz`] HTML/CSS engine.
//!
//! Architecture boundaries:
//! - [`dom`] exposes the shared nodeId-based document and event bridge used by
//!   every host package.
//! - [`app`] owns the winit event loop and window lifecycle exported by
//!   `@ylcc/napi-blitz`.
//! - [`window`] owns the window handle and options types.
//! - [`buffer_surface`] owns the headless RGBA frame path exported by
//!   `@ylcc/wasm-blitz`.

#[macro_use]
extern crate napi_derive;

mod global;

mod helpers;

mod dom;

mod dialog;

#[cfg(feature = "buffer-surface")]
mod buffer_surface;

#[cfg(feature = "native-window")]
mod app;
mod renderer;
#[cfg(feature = "native-window")]
mod window;
