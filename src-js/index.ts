// Public API of `@ylcc/napi-blitz`.
//
// This package owns the native-window path: winit event loop, OS windows, and
// the shared DOM API. Headless buffer rendering lives in `@ylcc/wasm-blitz`.
//
// Every class here is a `#[layer]` chain exported by the native module:
// the DOM classes (Node, Element, Document, the event classes, ...), plus
// `BlitzApp` and `Window` — both extend the layer `EventTarget`, so the
// Rust side dispatches lifecycle events straight onto their layer slots.
// The only JS-side logic left is the pump-loop driver in `./pump`.

import {setPumpAppLoop} from "./native";
import {pumpAppLoop} from "./pump";

// Native `BlitzApp.pumpLoop` forwards to this function to run the loop.
setPumpAppLoop(pumpAppLoop);

export * from "./native";
export * from "./pump";