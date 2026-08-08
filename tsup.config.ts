import {defineConfig} from "tsup";

export default defineConfig([
  {
    entry: ["src-js/index.ts"],
    format: ["esm"],
    outDir: "dist",
    outExtension: () => ({ js: ".mjs", dts: ".d.ts" }),
    dts: true,
    splitting: false,
    sourcemap: false,
    clean: true,
    tsconfig: "tsconfig.json",
  },
  {
    entry: ["src-js/index.ts"],
    format: ["cjs"],
    outDir: "dist",
    outExtension: () => ({ js: ".cjs" }),
    dts: false,
    splitting: false,
    sourcemap: false,
    clean: false,
    tsconfig: "tsconfig.json",
    esbuildOptions: (options) => {
      options.define = {
        ...options.define,
        "import.meta.dirname": "__dirname",
      };
    },
  },
]);
