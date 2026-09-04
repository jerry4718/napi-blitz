// Public API of `@ylcc/napi-blitz`.
//
// This package owns the native-window path: winit event loop, OS windows, and
// the shared DOM API. Headless buffer rendering lives in `@ylcc/wasm-blitz`.
//
// Every class here is a `#[layer]` chain exported by the native module:
// the DOM classes (Node, Element, Document, the event classes, ...), plus
// `BlitzApp` and `Window` — both extend the layer `EventTarget`, so the
// Rust side dispatches lifecycle events straight onto their layer slots.
// The only JS-side logic left is the pump-loop driver in `./pump`; internal
// bootstrap wiring (pump-loop injection, listener-registry registration)
// lives in `./internal` and is deliberately absent from this surface.

import "./internal";

export * from "./native";
export * from "./pump";
