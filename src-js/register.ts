// One-time registration of JS constructors and event factory with the
// Rust global registry. This module is imported for its side effects.
//
// Must be imported after all DOM classes are defined. Importing this
// module registers everything; subsequent imports are no-ops (the Rust
// side stores in a global, and JS classes don't change).

import type {EventPayload} from "./native";
import {registerEventFactory, registerNodeConstructor, registerElementConstructor} from "./native";
import {NodeTypes} from "./base/node";
import {Text} from "./base/text";
import {Comment} from "./base/comment";
import {Document} from "./document/document";
import {HTMLElement} from "./element/html-element";
import {HTMLInputElement} from "./element/html-input-element";
import {HTMLTextAreaElement} from "./element/html-textarea-element";
import {buildEvent} from "./events/events";

const HTML_NS = "http://www.w3.org/1999/xhtml";

registerNodeConstructor(NodeTypes.TEXT_NODE, Text as unknown as (arg: unknown) => unknown);
registerNodeConstructor(NodeTypes.COMMENT_NODE, Comment as unknown as (arg: unknown) => unknown);
registerNodeConstructor(NodeTypes.DOCUMENT_NODE, Document as unknown as (arg: unknown) => unknown);
registerNodeConstructor(NodeTypes.ELEMENT_NODE, HTMLElement as unknown as (arg: unknown) => unknown);
registerElementConstructor(HTML_NS, "input", HTMLInputElement as unknown as (arg: unknown) => unknown);
registerElementConstructor(HTML_NS, "textarea", HTMLTextAreaElement as unknown as (arg: unknown) => unknown);
registerEventFactory((payload: EventPayload) => buildEvent(payload));
