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
    return new Element(this._native, nodeId, this);
  }
}
