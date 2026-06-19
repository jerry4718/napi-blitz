// Shim around the auto-generated napi-rs entry. The native bundle
// (`native/index.js` + `native/index.d.ts` + `*.node`) lives at the
// package root in a sibling `native/` directory.
//
// We resolve it dynamically via `createRequire` so the same source file
// works regardless of whether the consumer is running TypeScript directly
// (rootDir = package src/) or the compiled output (in dist/). The relative
// path from this module to the native bundle differs in the two cases,
// so we anchor the lookup on `__dirname` and walk up to the package root.

import { createRequire } from "node:module";
import * as path from "node:path";

import type {
  AttrInit,
  BufferFrame,
  BufferRenderer as NativeBufferRenderer,
  BufferRendererOptions,
  DispatchResult,
  DocHandle as NativeDocHandle,
  DocHandleConfig,
  EventPayload,
  ImeData,
  InputData,
  KeyData,
  PointerData,
  RegisterFontOptions,
  WheelData,
} from "../native";

interface NativeModuleShape {
  BufferRenderer: typeof NativeBufferRenderer;
  DocHandle: typeof NativeDocHandle;
}

// Both `src/native.ts` and `dist/native.js` sit one directory below the
// package root, so a single `..` step reaches it.
const packageRoot = path.resolve(__dirname, "..");
const requireFromRoot = createRequire(path.join(packageRoot, "_anchor.js"));

const mod: NativeModuleShape = requireFromRoot("./native/index.js");

export const NativeBufferRendererCtor: typeof NativeBufferRenderer =
  mod.BufferRenderer;
export const NativeDocHandleCtor: typeof NativeDocHandle = mod.DocHandle;

export type {
  AttrInit,
  BufferFrame,
  BufferRendererOptions,
  DispatchResult,
  DocHandleConfig,
  EventPayload,
  ImeData,
  InputData,
  KeyData,
  PointerData,
  RegisterFontOptions,
  WheelData,
  NativeBufferRenderer,
  NativeDocHandle,
};
