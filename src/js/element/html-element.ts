// `HTMLElement` — the user-facing element class for HTML documents.
// Adds the inline-style hooks. The full `style` Proxy (CSSStyleDeclaration-
// like) will land in a later refactor; for now we surface plain
// setStyle/removeStyle methods so callers can still drive blitz.

import { Element } from "./element";

export class HTMLElement extends Element {
  /**
   * Set a single inline style property.
   * TODO: replace with a Proxy-based `style` getter so callers can write
   * `el.style.color = "red"` and read `el.style.color`.
   */
  setStyle(name: string, value: string): void {
    this._handle.setStyleProperty(this._nodeId, name, value);
  }

  /** Remove a single inline style property. */
  removeStyle(name: string): void {
    this._handle.removeStyleProperty(this._nodeId, name);
  }
}
