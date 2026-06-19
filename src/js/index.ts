// Public API of `@ylcc/napi-blitz`.

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
