// `HTMLDocument` — concrete Document for HTML content. Element wrappers
// are produced as `HTMLElement` instances.

import { Document, type DocumentInit } from "./document";
import { Element } from "../element/element";
import { HTMLElement } from "../element/html-element";

export class HTMLDocument extends Document {
  constructor(init?: DocumentInit) {
    super(init);
  }

  protected _makeElementWrapper(nodeId: bigint): Element {
    const handle = this._native.nodeHandle(nodeId);
    if (handle === null) {
      throw new Error(`Attempted to wrap missing element ${nodeId.toString()}`);
    }
    return new HTMLElement(handle, nodeId, this);
  }
}
