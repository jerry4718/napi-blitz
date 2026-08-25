// `Element` — element nodes (div, span, ...). Concrete user-facing
// subclass is `HTMLElement`; this base class hosts the parts of the DOM
// `Element` interface that are independent of the HTML namespace.

import {Node} from "../base/node";
import type {AttrInit, DomRect} from "../native";
import {type AttributesMap, makeAttributesProxy} from "./attributes";
import {TypedEventTarget} from "../helpers/events";
import type {ElementEventMap} from "../events/event-maps";

export class Element extends TypedEventTarget<ElementEventMap, typeof Node>(Node) {
  private _attributesProxy: AttributesMap | null = null;

  /** Element local tag name, lowercased (e.g. "div"). */
  get tagName(): string {
    // The web spec returns uppercase for HTML; blitz hands us lowercase,
    // which we return as-is.
    return this._handle.tagName() ?? "";
  }

  /** Mirrors web `Element.localName` (the lowercase tag name). */
  get localName(): string {
    return this.tagName;
  }

  // ---- Attributes --------------------------------------------------------

  /**
   * NamedNodeMap-like attribute view. Proxy-backed so reads/writes go
   * to the native side on each access. We expose object-style access
   * (`el.attributes.id`, `el.attributes.id = "x"`, `delete el.attributes.id`,
   * `for (const k in el.attributes)`).
   */
  get attributes(): AttributesMap {
    if (this._attributesProxy === null) {
      this._attributesProxy = makeAttributesProxy(this._handle);
    }
    return this._attributesProxy;
  }

  getAttribute(name: string): string | null {
    return this._handle.getAttribute(name);
  }

  setAttribute(name: string, value: string): void {
    this._handle.setAttribute(name, value, null);
  }

  setAttributeNS(namespace: string | null, name: string, value: string): void {
    this._handle.setAttribute(name, value, namespace);
  }

  removeAttribute(name: string): void {
    this._handle.removeAttribute(name, null);
  }

  removeAttributeNS(namespace: string | null, name: string): void {
    this._handle.removeAttribute(name, namespace);
  }

  hasAttribute(name: string): boolean {
    return this._handle.getAttribute(name) !== null;
  }

  /** Snapshot of attribute names. */
  getAttributeNames(): string[] {
    return this._handle.getAttributes().map((a: AttrInit) => a.name);
  }

  // ---- Convenience id / class --------------------------------------------

  get id(): string {
    return this.getAttribute("id") ?? "";
  }

  set id(value: string) {
    this.setAttribute("id", value);
  }

  get className(): string {
    return this.getAttribute("class") ?? "";
  }

  set className(value: string) {
    this.setAttribute("class", value);
  }

  // ---- HTML serialization ------------------------------------------------

  get innerHTML(): string {
    return this._handle.innerHtml() ?? "";
  }

  set innerHTML(value: string) {
    this._handle.setInnerHtml(value);
  }

  get outerHTML(): string {
    return this._handle.outerHtml() ?? "";
  }

  // ---- Queries scoped to this element ------------------------------------

  /**
   * All descendant elements with the given tag name. Per spec the
   * element itself is not included in the result; our native
   * `findAllByLocalNameIn` starts the DFS at this element's children,
   * so that holds. Snapshot array, not a live collection.
   *
   * `"*"` matches all descendant elements via `findAllElementsIn`.
   */
  getElementsByTagName(name: string): Element[] {
    if (name === "*") {
      return this._doc._native.findAllElementsIn(this._handle) as Element[];
    }
    return this._doc._native.findAllByLocalNameIn(this._handle, name.toLowerCase()) as Element[];
  }

  /**
   * All descendant elements carrying the given class name. Element
   * itself is excluded (DFS starts at children). Snapshot array.
   */
  getElementsByClassName(className: string): Element[] {
    return this._doc._native.findAllByClassNameIn(this._handle, className) as Element[];
  }

  /**
   * First descendant element matching `selector`, or null. Uses stylo's
   * selector engine via `querySelectorIn` — the root is this element,
   * and per spec the element itself is not a candidate.
   */
  querySelector(selector: string): Element | null {
    return this._handle.querySelector(selector) as Element | null;
  }

  /**
   * All descendant elements matching `selector`. Snapshot array.
   * Selector matching runs through stylo scoped to this element's
   * subtree.
   */
  querySelectorAll(selector: string): Element[] {
    return this._handle.querySelectorAll(selector) as Element[];
  }

  getBoundingClientRect(): DomRect | null {
    return this._handle.getBoundingClientRect();
  }

  get scrollTop(): number {
    return this._handle.scrollTop;
  }

  set scrollTop(v: number) {
    this._handle.scrollTop = v;
  }

  get scrollLeft(): number {
    return this._handle.scrollLeft;
  }

  set scrollLeft(v: number) {
    this._handle.scrollLeft = v;
  }

  get scrollHeight(): number {
    return this._handle.scrollHeight;
  }

  get scrollWidth(): number {
    return this._handle.scrollWidth;
  }

  get clientHeight(): number {
    return this._handle.clientHeight;
  }

  get clientWidth(): number {
    return this._handle.clientWidth;
  }
}
