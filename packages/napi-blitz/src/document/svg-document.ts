// `SVGDocument` — stub for SVG documents. Reserved for future
// expansion; we'll add `SVGElement` when we ship SVG-specific APIs.
//
// For now, element wrappers are plain `Element`.

import { Document, type DocumentInit } from "./document";
import { Element } from "../element/element";

export class SVGDocument extends Document {
  constructor(init?: DocumentInit) {
    super(init);
  }

  protected _makeElementWrapper(nodeId: bigint): Element {
    const handle = this._native.nodeHandle(nodeId);
    if (handle === null) {
      throw new Error(`Attempted to wrap missing element ${nodeId.toString()}`);
    }
    return new Element(handle, nodeId, this);
  }
}
