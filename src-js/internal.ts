// Internal bootstrap wiring and internal-protocol types. Not part of the
// public package surface: the entry points would let user code replace the
// pump-loop driver or the listener registry, and the protocol types describe
// the JS↔Rust wire format, so both live here instead of `./native.ts`.

import {createRequire} from "node:module";

declare const __filename: string;

// Same resolution as `./native.ts`: works for both root-dir TypeScript runs
// and the compiled dist output; rolldown defines the format branch via
// transform.define and DCE removes the dead one.
const require = createRequire(
  process.env.FORMAT === "cjs"
    ? __filename
    : import.meta.url,
);

const mod = require("../native/index.cjs") as typeof import("../native");

// Wire-format types of the listener registry, straight from the generated
// native declarations.
export type {ListenerOps, ListenerSpec} from "../native";

import {ListenerOps} from "./listener-registry";
import {pumpAppLoop} from "./pump";

// Native `BlitzApp.pumpLoop` forwards to this function to run the loop.
mod.setPumpAppLoop(pumpAppLoop);

// The JS-side listener registry is the strong holder of every listener
// callback, anchored by each target's own lifetime (see ./listener-registry).
mod.setListenerOps(ListenerOps);

// `on<event>` IDL-style attributes live on `Node.prototype`, defined from
// Rust once native classes are registered.
mod.defineNodeOnEventAttributes();

// `<body>` forwards window event handlers to the Window's attribute
// listener; the reflecting `on<event>` attributes are defined on its own
// prototype.
mod.defineHtmlBodyEventAttributes();

// `Window` is EventTarget-rooted (not a Node), so its window event
// handlers and bubbled interaction events get their own `on<event>`
// attributes on `Window.prototype`.
mod.defineWindowEventAttributes();
