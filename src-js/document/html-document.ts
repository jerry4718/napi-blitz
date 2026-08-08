// `HTMLDocument` - concrete Document for HTML content.
// Element wrappers are produced as `HTMLElement` instances via Rust's
// NodeCache + wrap_node.

import {Document, type DocumentInit} from "./document";
import {NativeDoc} from "../native";

export class HTMLDocument extends Document {
  /**
   * Create a new HTMLDocument with a fresh native DocHandle.
   * This is the primary way to construct a document in headless mode
   * (tests, buffer renderer, etc.).
   */
  static create(init?: DocumentInit): HTMLDocument {
    const handle = NativeDoc.create({
      uaStylesheets: init?.uaStylesheets,
      baseHtml: init?.baseHtml,
    });
    return new HTMLDocument(handle);
  }

  constructor(handle: InstanceType<typeof NativeDoc>) {
    super(handle);
  }
}
