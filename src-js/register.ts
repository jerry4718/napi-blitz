// One-time registration of JS constructors and event factory with the
// Rust global registry. This module is imported for its side effects.
//
// Must be imported after all DOM classes are defined. Importing this
// module registers everything; subsequent imports are no-ops (the Rust
// side stores in a global, and JS classes don't change).

import type {EventPayload} from "./native";
import {
  initEnv,
  registerCancelBubbleGetter,
  registerDefaultPreventedGetter,
  registerDispatchFn,
  registerElementConstructor,
  registerEventFactory,
  registerLazyCurrentTargetSetter,
  registerLazyTargetSetter,
  registerNodeConstructor
} from "./native";
import {NodeTypes} from "./base/node";
import {Text} from "./base/text";
import {Comment} from "./base/comment";
import {Document} from "./document/document";
import {HTMLElement} from "./element/html-element";
import {HTMLInputElement} from "./element/html-input-element";
import {HTMLTextAreaElement} from "./element/html-textarea-element";
import {buildEvent, setLazyCurrentTarget, setLazyTarget} from "./events/events";
import {dispatchEvent} from "./helpers/events.ts";

initEnv();

registerNodeConstructor(NodeTypes.TEXT_NODE, Text as any);
registerNodeConstructor(NodeTypes.COMMENT_NODE, Comment as any);
registerNodeConstructor(NodeTypes.DOCUMENT_NODE, Document as any);
registerNodeConstructor(NodeTypes.ELEMENT_NODE, HTMLElement as any);

const HTML_NS = "http://www.w3.org/1999/xhtml";

registerElementConstructor(HTML_NS, "input", HTMLInputElement as any);
registerElementConstructor(HTML_NS, "textarea", HTMLTextAreaElement as any);

registerEventFactory((payload: EventPayload) => buildEvent(payload));
registerDispatchFn(dispatchEvent as any);

registerCancelBubbleGetter(((event: Event) => event.cancelBubble) as any);
registerDefaultPreventedGetter(((event: Event) => event.defaultPrevented) as any);

registerLazyTargetSetter(setLazyTarget as any);
registerLazyCurrentTargetSetter(setLazyCurrentTarget as any);
