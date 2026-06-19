// Public API of `@ylcc/wasm-blitz`.
//
// This package owns the headless/buffer-surface architecture. The current
// experimental backend is still the Rust N-API renderer, but the JS API is kept
// separate so this package can move to a wasm/browser backend without changing
// `@ylcc/napi-blitz`'s native-window contract.

export { BufferBlitzApp } from "./buffer/app";
export type { BufferBlitzAppInit, BufferFrame } from "./buffer/app";

export { Document } from "./document/document";
export type { DocumentInit } from "./document/document";
export { HTMLDocument } from "./document/html-document";
export { XMLDocument } from "./document/xml-document";
export { SVGDocument } from "./document/svg-document";

export { Node, NodeTypes } from "./base/node";
export { CharacterData } from "./base/character-data";
export { Text } from "./base/text";
export { Comment } from "./base/comment";
export { Element } from "./element/element";
export { HTMLElement } from "./element/html-element";

export type { AttributesMap } from "./element/attributes";
export type { StyleDeclaration } from "./element/style";

export { FontFace } from "./fonts/font-face";
export type {
  FontFaceDescriptors,
  FontFaceLoadStatus,
  FontFaceSource,
} from "./fonts/font-face";
export { FontFaceSet } from "./fonts/font-face-set";
export type { FontFaceSetLoadStatus } from "./fonts/font-face-set";

export {
  BlitzDomEvent,
  BlitzPointerEvent,
  BlitzWheelEvent,
  BlitzKeyboardEvent,
  BlitzInputEvent,
  BlitzImeEvent,
} from "./events/events";

export type {
  AttrInit,
  EventPayload,
  PointerData,
  WheelData,
  KeyData,
  InputData,
  ImeData,
} from "./native";
