// `XMLDocument` — stub for XML documents. Element wrappers are plain
// `Element` until we have an XML-specific subclass.
//
// Reserved for future expansion; HTMLDocument is the path most users
// take today.

import { Document, type DocumentInit } from "./document";
import { Element } from "../element/element";

export class XMLDocument extends Document {
  constructor(init?: DocumentInit) {
    super(init);
  }

  protected _makeElementWrapper(nodeId: bigint): Element {
    return new Element(this._native, nodeId, this);
  }
}
