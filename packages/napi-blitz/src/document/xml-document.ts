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
    const handle = this._native.nodeHandle(nodeId);
    if (handle === null) {
      throw new Error(`Attempted to wrap missing element ${nodeId.toString()}`);
    }
    return new Element(handle, nodeId, this);
  }
}
