// Shim around the auto-generated napi-rs entry. The native bundle
// (`native/index.cjs` + `native/index.d.ts` + `*.node`) lives at the
// package root in a sibling `native/` directory.
//
// We resolve it dynamically via `createRequire` so the same source file
// works regardless of whether the consumer is running TypeScript directly
// (rootDir = package src/) or the compiled output (in dist/).

import {createRequire} from "node:module";

declare const __filename: string;

// ESM and CJS resolve the native module the same way; only the filename
// source differs. rolldown replaces process.env.FORMAT with a literal via
// transform.define, then output.minify: 'dce-only' eliminates the
// unreachable branch.
const require = createRequire(
  process.env.FORMAT === "cjs"
    ? __filename
    // import.meta.url is only valid in ESM; in the CJS build rolldown
    // defines it as "__filename", then DCE removes this branch.
    : import.meta.url,
);

const mod = require("../native/index.cjs") as typeof import("../native");

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
export const pickFile = mod.pickFile;
export const pickFiles = mod.pickFiles;
export const pickFolder = mod.pickFolder;
export const pickFolders = mod.pickFolders;
export const saveFile = mod.saveFile;
