// Public API of `@ylcc/napi-blitz`.
//
// This package owns the native-window path: winit event loop, OS windows, and
// the shared DOM API. Headless buffer rendering lives in `@ylcc/wasm-blitz`.

export { BlitzApp } from "./host/app";
export type { OpenWindowInit } from "./host/app";
export { Window } from "./host/window";

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
  PumpResult,
} from "./native";
