// Headless buffer renderer. Unlike `BlitzApp`, this wrapper owns no native
// window and no winit event loop. It resolves a DOM document and returns RGBA
// frames that the host can display through Canvas, WebGPU, native windows, or
// any other surface exchange mechanism.

import {
  NativeBufferRendererCtor,
  type BufferFrame,
  type BufferRendererOptions,
  type NativeBufferRenderer,
} from "../native";
import { HTMLDocument } from "../document/html-document";

interface DocumentInternalsForBufferRenderer {
  readonly _native: import("../native").NativeDocHandle;
}

function pluckDoc(doc: HTMLDocument): DocumentInternalsForBufferRenderer {
  return doc as unknown as DocumentInternalsForBufferRenderer;
}

export interface BufferBlitzAppInit extends BufferRendererOptions {
  uaStylesheets?: string[];
  baseHtml?: string;
}

export class BufferBlitzApp {
  readonly document: HTMLDocument;
  readonly _native: NativeBufferRenderer;

  private constructor(document: HTMLDocument, native: NativeBufferRenderer) {
    this.document = document;
    this._native = native;
  }

  static create(init: BufferBlitzAppInit): BufferBlitzApp {
    const document = new HTMLDocument({
      uaStylesheets: init.uaStylesheets,
      baseHtml: init.baseHtml,
    });
    const native = NativeBufferRendererCtor.create({
      width: init.width,
      height: init.height,
      scale: init.scale,
    });
    return new BufferBlitzApp(document, native);
  }

  resize(options: BufferRendererOptions): void {
    this._native.resize(options);
  }

  render(): BufferFrame {
    return this._native.render(pluckDoc(this.document)._native);
  }
}

export type { BufferFrame, BufferRendererOptions };
