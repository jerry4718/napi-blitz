// Shim around the auto-generated napi-rs entry. The native bundle
// (`native/index.cjs` + `native/index.d.ts` + `*.node`) lives at the
// package root in a sibling `native/` directory.
//
// We resolve it dynamically via `createRequire` so the same source file
// works regardless of whether the consumer is running TypeScript directly
// (rootDir = package src/) or the compiled output (in dist/).

import {createRequire} from "node:module";
import * as path from "node:path";

// ESM: import.meta.dirname (Node 20.11+).
// CJS: tsup define replaces import.meta.dirname with __dirname.
const dirname: string = import.meta.dirname;

// This module sits one directory below the package root,
// so a single `..` step reaches it.
const packageRoot = path.resolve(dirname, "..");
const requireFromRoot = createRequire(path.join(packageRoot, "_anchor.js"));

const mod = requireFromRoot("./native/index.cjs") as typeof import("../native");

export type * from "../native";

export const NativeApp = mod.NativeApp;
export const NativeDoc = mod.NativeDoc;
export const NativeWindow = mod.NativeWindow;
export const NativeNode = mod.NativeNode;
export const WindowOptions = mod.WindowOptions;
export const initEnv = mod.initEnv;
export const registerNodeConstructor = mod.registerNodeConstructor;
export const registerElementConstructor = mod.registerElementConstructor;
export const registerEventFactory = mod.registerEventFactory;
export const registerDispatchFn = mod.registerDispatchFn;
export const registerCancelBubbleGetter = mod.registerCancelBubbleGetter;
export const registerDefaultPreventedGetter = mod.registerDefaultPreventedGetter;
export const pickFile = mod.pickFile;
export const pickFiles = mod.pickFiles;
export const pickFolder = mod.pickFolder;
export const pickFolders = mod.pickFolders;
export const saveFile = mod.saveFile;
