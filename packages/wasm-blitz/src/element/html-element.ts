// `HTMLElement` — the user-facing element class for HTML documents.
//
// Adds the `style` accessor: a Proxy that mirrors the web
// `CSSStyleDeclaration` interface for inline styles. All reads and
// writes flow through the native side per access; the same proxy
// instance is reused across calls so identity comparisons work.

import { Element } from "./element";
import { makeStyleProxy, type StyleDeclaration } from "./style";

export class HTMLElement extends Element {
  private _styleProxy: StyleDeclaration | null = null;

  /**
   * The element's inline-style declaration. Mirrors the web
   * `HTMLElement.style` getter:
   *
   * ```ts
   * el.style.color = "red";
   * el.style.fontSize = "12px";       // camelCase or kebab-case
   * el.style.getPropertyValue("color");
   * el.style.cssText = "color: red";
   * ```
   *
   * The returned object is a Proxy; every property read/write goes
   * through the native side. The proxy itself is cached so
   * `el.style === el.style`.
   */
  get style(): StyleDeclaration {
    if (this._styleProxy === null) {
      this._styleProxy = makeStyleProxy(this._handle, this._nodeId);
    }
    return this._styleProxy;
  }
}
