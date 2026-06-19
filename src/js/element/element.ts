// `Element` — element nodes (div, span, ...). Concrete user-facing
// subclass is `HTMLElement`; this base class hosts the parts of the DOM
// `Element` interface that are independent of the HTML namespace.

import { Node } from "../base/node";
import { makeAttributesProxy, type AttributesMap } from "./attributes";
import type { Document } from "../document/document";

export class Element extends Node {
  private _attributesProxy: AttributesMap | null = null;

  /** Element local tag name, lowercased (e.g. "div"). */
  get tagName(): string {
    // Spec returns uppercase for HTML; blitz hands us lowercase. We
    // return what blitz says for now; HTMLElement layers on the
    // HTML-uppercasing nuance later if needed.
    return this._handle.tagName(this._nodeId) ?? "";
  }

  /** Same as `tagName` for now; mirrors web `Element.localName`. */
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
      this._attributesProxy = makeAttributesProxy(this._handle, this._nodeId);
    }
    return this._attributesProxy;
  }

  getAttribute(name: string): string | null {
    return this._handle.getAttribute(this._nodeId, name);
  }

  setAttribute(name: string, value: string): void {
    this._handle.setAttribute(this._nodeId, name, value, null);
  }

  setAttributeNS(namespace: string | null, name: string, value: string): void {
    this._handle.setAttribute(this._nodeId, name, value, namespace);
  }

  removeAttribute(name: string): void {
    this._handle.removeAttribute(this._nodeId, name, null);
  }

  removeAttributeNS(namespace: string | null, name: string): void {
    this._handle.removeAttribute(this._nodeId, name, namespace);
  }

  hasAttribute(name: string): boolean {
    return this._handle.getAttribute(this._nodeId, name) !== null;
  }

  /** Snapshot of attribute names. */
  getAttributeNames(): string[] {
    return this._handle.getAttributes(this._nodeId).map((a) => a.name);
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
    return this._handle.innerHtml(this._nodeId) ?? "";
  }
  set innerHTML(value: string) {
    this._handle.setInnerHtml(this._nodeId, value);
  }

  get outerHTML(): string {
    return this._handle.outerHtml(this._nodeId) ?? "";
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
    const owner = this._ownerDocument as unknown as Document;
    if (name === "*") {
      return owner._native
        .findAllElementsIn(this._nodeId)
        .map((id) => owner._wrap(id) as Element);
    }
    return owner._native
      .findAllByLocalNameIn(this._nodeId, name.toLowerCase())
      .map((id) => owner._wrap(id) as Element);
  }

  /**
   * All descendant elements carrying the given class name. Element
   * itself is excluded (DFS starts at children). Snapshot array.
   */
  getElementsByClassName(className: string): Element[] {
    const owner = this._ownerDocument as unknown as Document;
    return owner._native
      .findAllByClassNameIn(this._nodeId, className)
      .map((id) => owner._wrap(id) as Element);
  }

  // TODO: native side currently only exposes document-scoped querySelector.
  // We'll add element-scoped queries when blitz exposes them.
}
