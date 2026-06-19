// `Element` is the JS-side wrapper for one blitz node. It extends
// `EventTarget` so listeners get standard DOM semantics
// (addEventListener / dispatchEvent / preventDefault /
// stopPropagation / stopImmediatePropagation).
//
// The native handle and the underlying nodeId are stored in `private`
// fields. They are not part of the public API; Document reaches in
// through a controlled cast inside its own module, the only intended
// "friend" channel into Element.

import type { NativeDocHandle, AttrInit } from "./native";

export class Element extends EventTarget {
  private readonly _handle: NativeDocHandle;
  private readonly _nodeId: number;

  /**
   * Construct an Element. Always call this through Document's factory
   * methods; constructing one directly would not register it with the
   * owning document and is not part of the public contract.
   *
   * (TypeScript cannot enforce package-private constructors, but
   * Document is the only intended caller.)
   */
  constructor(handle: NativeDocHandle, nodeId: number) {
    super();
    this._handle = handle;
    this._nodeId = nodeId;
  }

  /** DOM-style nodeType (1 element, 3 text, 8 comment, 9 document). */
  get nodeType(): number {
    return this._handle.nodeType(this._nodeId);
  }

  /** Element local tag name, lowercased. Null for non-element nodes. */
  get tagName(): string | null {
    return this._handle.tagName(this._nodeId);
  }

  /** All-descendants concatenated text. */
  get textContent(): string | null {
    return this._handle.textContent(this._nodeId);
  }
  set textContent(value: string) {
    this._handle.setTextContent(this._nodeId, value);
  }

  // Attributes ------------------------------------------------------------

  getAttribute(name: string): string | null {
    return this._handle.getAttribute(this._nodeId, name);
  }

  setAttribute(name: string, value: string, namespace?: string): void {
    this._handle.setAttribute(this._nodeId, name, value, namespace ?? null);
  }

  removeAttribute(name: string, namespace?: string): void {
    this._handle.removeAttribute(this._nodeId, name, namespace ?? null);
  }

  /** Snapshot of all attributes. */
  attributes(): AttrInit[] {
    return this._handle.getAttributes(this._nodeId);
  }

  // Inline styles ---------------------------------------------------------

  setStyle(name: string, value: string): void {
    this._handle.setStyleProperty(this._nodeId, name, value);
  }

  removeStyle(name: string): void {
    this._handle.removeStyleProperty(this._nodeId, name);
  }

  // Inner HTML ------------------------------------------------------------

  setInnerHtml(html: string): void {
    this._handle.setInnerHtml(this._nodeId, html);
  }
}
