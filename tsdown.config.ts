import {defineConfig} from "tsdown";

export default defineConfig({
  entry: "src-js/index.ts",
  format: ["esm", "cjs"],
  outDir: "dist",
  platform: "node",
  tsconfig: "tsconfig.json",
  clean: true,
  sourcemap: false,
  minify: "dce-only",
  dts: true,
  fixedExtension: true,
  outExtensions: () => ({dts: ".d.ts"}),
  inputOptions(options, format) {
    options.transform = {
      ...options.transform,
      define: {
        ...options.transform?.define,
        "process.env.FORMAT": `"${format}"`,
        ...(format === "cjs" ? {"import.meta.url": '"__filename"'} : {}),
      },
    };
    return options;
  },
});
