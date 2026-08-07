// Shim around the auto-generated napi-rs entry. The native bundle
// (`native/index.js` + `native/index.d.ts` + `*.node`) lives at the
// package root in a sibling `native/` directory.
//
// We resolve it dynamically via `createRequire` so the same source file
// works regardless of whether the consumer is running TypeScript directly
// (rootDir = package src/) or the compiled output (in dist/). The relative
// path from this module to the native bundle differs in the two cases,
// so we anchor the lookup on `__dirname` and walk up to the package root.

import {createRequire} from "node:module";
import * as path from "node:path";

import type {
  AppDispatchResult,
  AppEventPayload,
  AttrInit,
  BlitzApp as NativeBlitzApp,
  DialogOptions,
  DocHandle as NativeDocHandle,
  DocHandleConfig,
  DomRect,
  EventPayload,
  FileFilter,
  ImeData,
  InputData,
  KeyData,
  MonitorInfo,
  NodeHandle as NativeNodeHandle,
  PointerData,
  PumpResult,
  registerEventFactory as _registerEventFactory,
  RegisterFontOptions,
  registerNodeConstructor as _registerNodeConstructor,
  VideoModeInfo,
  WheelData,
  Window as NativeWindow,
  WindowOptions as NativeWindowOptions,
} from "../native";

interface NativeModuleShape {
  BlitzApp: typeof NativeBlitzApp;
  DocHandle: typeof NativeDocHandle;
  NodeHandle: typeof NativeNodeHandle;
  WindowOptions: typeof NativeWindowOptions;
  registerNodeConstructor: typeof _registerNodeConstructor;
  registerEventFactory: typeof _registerEventFactory;
  pickFile: (options?: DialogOptions | null) => Promise<string | null>;
  pickFiles: (options?: DialogOptions | null) => Promise<string[]>;
  pickFolder: (options?: DialogOptions | null) => Promise<string | null>;
  pickFolders: (options?: DialogOptions | null) => Promise<string[]>;
  saveFile: (options?: DialogOptions | null) => Promise<string | null>;
}

// Both `src/native.ts` and `dist/native.js` sit one directory below the
// package root, so a single `..` step reaches it.
const packageRoot = path.resolve(__dirname, "..");
const requireFromRoot = createRequire(path.join(packageRoot, "_anchor.js"));

const mod: NativeModuleShape = requireFromRoot("./native/index.js");

export const NativeBlitzAppCtor: typeof NativeBlitzApp = mod.BlitzApp;
export const NativeDocHandleCtor: typeof NativeDocHandle = mod.DocHandle;
export const NativeNodeHandleCtor: typeof NativeNodeHandle = mod.NodeHandle;
export const NativeWindowOptionsCtor: typeof NativeWindowOptions = mod.WindowOptions;
export const registerNodeConstructor = mod.registerNodeConstructor;
export const registerEventFactory = mod.registerEventFactory;

export const pickFile = mod.pickFile;
export const pickFiles = mod.pickFiles;
export const pickFolder = mod.pickFolder;
export const pickFolders = mod.pickFolders;
export const saveFile = mod.saveFile;

export type {
  AppDispatchResult,
  AppEventPayload,
  AttrInit,
  DialogOptions,
  DocHandleConfig,
  DomRect,
  EventPayload,
  FileFilter,
  ImeData,
  InputData,
  KeyData,
  MonitorInfo,
  PointerData,
  PumpResult,
  RegisterFontOptions,
  VideoModeInfo,
  NativeNodeHandle,
  WheelData,
  NativeBlitzApp,
  NativeDocHandle,
  NativeNodeHandle as NodeHandle,
  NativeWindow as Window,
  NativeWindowOptions as WindowOptions,
};
