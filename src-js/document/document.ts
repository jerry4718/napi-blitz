// `Document` - abstract base class for any document type. Concrete
// subclasses are `HTMLDocument`, `XMLDocument`, `SVGDocument`.
//
// Rust creates the native document handle and injects it into the JS
// constructor. The JS constructor stores the handle and sets the
// document ref on it. Node constructors and the event factory are
// registered globally in `register.ts`. All node wrapping is done by
// Rust (NodeCache + wrap_node); JS methods forward to the native
// handle, which returns already-wrapped JS Node objects.

import {NativeDoc, NativeNode} from "../native";
import {Node} from "../base/node";
import {Element} from "../element/element";
import {Text} from "../base/text";
import {Comment} from "../base/comment";
import {FontFaceSet} from "../fonts/font-face-set";
import type {DocumentInternals} from "../internal/internal";
import {TypedEventTarget} from "../helpers/events";
import type {DocumentEventMap} from "../events/event-maps";

export interface DocumentInit {
  uaStylesheets?: string[];
  baseHtml?: string;
}

/**
 * Top-level base. Most users instantiate one of the concrete subclasses
 * (e.g. `HTMLDocument`); this base class is exported mainly for
 * `instanceof` checks and shared code.
 */
export abstract class Document extends TypedEventTarget<DocumentEventMap, typeof Node>(Node) implements DocumentInternals {
  readonly _native: InstanceType<typeof NativeDoc>;

  /** Lazily-built `FontFaceSet` exposed via `document.fonts`. */
  private _fontsSet: FontFaceSet | null = null;

  /**
   * @internal Constructed by Rust via `registerNodeConstructor` or by
   * `HTMLDocument.create()`. The `handle` is the native document handle.
   */
  constructor(handle: InstanceType<typeof NativeDoc>) {
    super(handle as unknown as InstanceType<typeof NativeNode>, handle as unknown as Document);
    this._native = handle;
    // Set the JS Document object reference so Rust can pass `doc` to
    // each JS Node constructor.
    this._native.setDocumentRef(this);
  }

  // ----- Standard DOM root accessors --------------------------------------

  /** Document is the root - it has no parent. */
  override get parentNode(): Node | null {
    return null;
  }

  override get parentElement(): Node | null {
    return null;
  }

  /** Document nodeType is always 9. */
  override get nodeType(): number {
    return 9;
  }

  /** The first child of the Document is the <html> element. */
  override get firstChild(): Node | null {
    return this.documentElement;
  }

  override get lastChild(): Node | null {
    return this.documentElement;
  }

  override get childNodes(): Node[] {
    const children: Node[] = [];
    const html = this.documentElement;
    if (html) children.push(html);
    return children;
  }

  override get hasChildNodes(): boolean {
    return true;
  }

  get documentElement(): Element {
    return this._native.htmlElement() as Element;
  }

  get head(): Element | null {
    return this._native.headElement() as Element | null;
  }

  get body(): Element | null {
    return this._native.bodyElement() as Element | null;
  }

  get title(): string {
    const el = this._native.findTitleNode();
    if (el === null) return "";
    return (el as Element).textContent ?? "";
  }

  set title(value: string) {
    const existing = this._native.findTitleNode();
    if (existing !== null) {
      (existing as Element).textContent = value;
      return;
    }
    const titleEl = this.createElement("title");
    titleEl.textContent = value;
    const head = this.head ?? this.documentElement;
    head.appendChild(titleEl);
  }

  // ----- Factories --------------------------------------------------------

  createElement(localName: string): Element {
    return this._native.createElement(localName, null, null) as Element;
  }

  createElementNS(namespace: string | null, qualifiedName: string): Element {
    return this._native.createElement(qualifiedName, namespace, null) as Element;
  }

  createTextNode(text: string): Text {
    return this._native.createTextNode(text) as Text;
  }

  createComment(data?: string): Comment {
    const comment = this._native.createCommentNode(data || '') as Comment;
    if (data !== undefined && data !== "") {
      comment.data = data;
    }
    return comment;
  }

  // ----- Queries ----------------------------------------------------------

  querySelector(selector: string): Element | null {
    return this._native.querySelector(selector) as Element | null;
  }

  querySelectorAll(selector: string): Element[] {
    return this._native.querySelectorAll(selector) as Element[];
  }

  getElementById(id: string): Element | null {
    return this._native.getElementById(id) as Element | null;
  }

  getElementsByTagName(name: string): Element[] {
    if (name === "*") {
      return this.querySelectorAll("*");
    }
    return this._native.findAllByLocalName(name.toLowerCase()) as Element[];
  }

  getElementsByClassName(className: string): Element[] {
    return this._native.findAllByClassName(className) as Element[];
  }

  // ----- Layout / lifecycle ----------------------------------------------

  resolve(timeMs = 0): void {
    this._native.resolve(timeMs);
  }

  // ----- Fonts ------------------------------------------------------------

  get fonts(): FontFaceSet {
    if (this._fontsSet === null) {
      this._fontsSet = new FontFaceSet(this._native);
    }
    return this._fontsSet;
  }
}
