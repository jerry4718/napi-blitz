#![deny(clippy::all)]
//! napi-blitz: a Node.js binding around the [`blitz`] HTML/CSS engine.
//!
//! The Rust side exposes a flat, nodeId-based API on the [`DocHandle`] class.
//! All DOM operations identify nodes by their numeric id; element-like
//! wrapper objects live entirely in the JS / TS layer.
//!
//! Event flow:
//! 1. `BlitzApp.pumpAppEvents` drives winit synchronously from the JS thread.
//! 2. When blitz produces a `DomEvent`, our [`event::JsEventHandler`]
//!    serializes the event chain plus payload and calls back into JS through
//!    the document's `__dispatchFromNative` hook.
//! 3. JS dispatches the event using standard `EventTarget` semantics.
//!    `stopPropagation` / `preventDefault` on the JS side are reported back to
//!    Rust via the return value, which we translate into blitz `EventState`.

mod app;
mod app_bridge;
mod app_handler;
mod doc;
mod event;
mod ops;
mod payload;
mod window;

pub use app::BlitzApp;
pub use app_bridge::{AppDispatchResult, AppEventPayload};
pub use doc::{DocHandle, DocHandleConfig};
pub use payload::*;
pub use window::Window;
