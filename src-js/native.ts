// Shim around the auto-generated napi-rs entry. The native bundle
// (`native/index.cjs` + `native/index.d.ts` + `*.node`) lives at the
// package root in a sibling `native/` directory.
//
// We resolve it dynamically via `createRequire` so the same source file
// works regardless of whether the consumer is running TypeScript directly
// (rootDir = package src/) or the compiled output (in dist/).

import {createRequire} from "node:module";

declare const __filename: string;

// ESM and CJS resolve the native module the same way; only the filename
// source differs. rolldown replaces process.env.FORMAT with a literal via
// transform.define, then output.minify: 'dce-only' eliminates the
// unreachable branch.
const require = createRequire(
  process.env.FORMAT === "cjs"
    ? __filename
    // import.meta.url is only valid in ESM; in the CJS build rolldown
    // defines it as "__filename", then DCE removes this branch.
    : import.meta.url,
);

// The layer classes re-export their instance types under the same name as
// the runtime constructor below. A bare `export type *` re-export loses the
// type binding next to a local `export const` of the same name, so each name
// is re-declared explicitly (the local declaration shadows the star
// re-export; the value comes from `mod`).
//
// The remaining native types are re-exported explicitly (not via
// `export type *`), so the dts bundler keeps their real declarations instead
// of degrading them to `undefined<...>` when a same-named local `type` +
// `const` pair is present.
import type * as types from "../native";

const mod = require("../native/index.cjs") as typeof import("../native");

export type {
  AddEventListenerOptions,
  Anything,
  AttrInit,
  AttributesHandler,
  CustomEventInit,
  DialogOptions,
  DocHandleConfig,
  DomRect,
  EventInit,
  EventListenerOptions,
  ExtendEventMap,
  FileFilter,
  FontFaceDescriptors,
  MessageEventInit,
  MonitorInfo,
  PumpResult,
  RegisterFontOptions,
  StyleHandler,
  TypedEventTarget,
  TypedEventTargetConstructor,
  VideoModeInfo,
  WindowHandle,
} from "../native";

// Runtime values: app/window handles, document creation, window ref
// registration, and the DOM class layer chain — each layer class is
// exported by its JS name, its instance type next to it.
export type Window = types.Window;
export const Window = mod.Window;
export type WindowOptions = types.WindowOptions;
export const WindowOptions = mod.WindowOptions;
export const createDocument = mod.createDocument;
export const pickFile = mod.pickFile;
export const pickFiles = mod.pickFiles;
export const pickFolder = mod.pickFolder;
export const pickFolders = mod.pickFolders;
export const saveFile = mod.saveFile;

export type BlitzApp = types.BlitzApp;
export const BlitzApp = mod.BlitzApp;
export type Comment = types.Comment;
export const Comment = mod.Comment;
export type CompositionEvent = types.CompositionEvent;
export const CompositionEvent = mod.CompositionEvent;
export type CustomEvent = types.CustomEvent;
export const CustomEvent = mod.CustomEvent;
export type Document = types.Document;
export const Document = mod.Document;
export type Element = types.Element;
export const Element = mod.Element;
export type Event = types.Event;
export const Event = mod.Event;
export type EventTarget = types.EventTarget;
export const EventTarget = mod.EventTarget;
export type FocusEvent = types.FocusEvent;
export const FocusEvent = mod.FocusEvent;
export type FontFace = types.FontFace;
export const FontFace = mod.FontFace;
export type FontFaceSet = types.FontFaceSet;
export const FontFaceSet = mod.FontFaceSet;
export type HTMLBodyElement = types.HTMLBodyElement;
export const HTMLBodyElement = mod.HTMLBodyElement;
export type HTMLDocument = types.HTMLDocument;
export const HTMLDocument = mod.HTMLDocument;
export type HTMLElement = types.HTMLElement;
export const HTMLElement = mod.HTMLElement;
export type HTMLHtmlElement = types.HTMLHtmlElement;
export const HTMLHtmlElement = mod.HTMLHtmlElement;
export type HTMLInputElement = types.HTMLInputElement;
export const HTMLInputElement = mod.HTMLInputElement;
export type HTMLTextAreaElement = types.HTMLTextAreaElement;
export const HTMLTextAreaElement = mod.HTMLTextAreaElement;
export type InputEvent = types.InputEvent;
export const InputEvent = mod.InputEvent;
export type KeyboardEvent = types.KeyboardEvent;
export const KeyboardEvent = mod.KeyboardEvent;
export type MessageEvent = types.MessageEvent;
export const MessageEvent = mod.MessageEvent;
export type MouseEvent = types.MouseEvent;
export const MouseEvent = mod.MouseEvent;
export type Node = types.Node;
export const Node = mod.Node;
export type PointerEvent = types.PointerEvent;
export const PointerEvent = mod.PointerEvent;
export type Text = types.Text;
export const Text = mod.Text;
export type UIEvent = types.UIEvent;
export const UIEvent = mod.UIEvent;
export type WheelEvent = types.WheelEvent;
export const WheelEvent = mod.WheelEvent;
export const NodeTypes = mod.NodeTypes;