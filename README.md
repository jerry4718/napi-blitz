# `@ylcc/napi-blitz`

![CI](https://github.com/jerry4718/napi-blitz/workflows/CI/badge.svg)

A Node.js native binding for [Blitz](https://github.com/DioxusLabs/blitz), built with [napi-rs](https://napi.rs/), exposing a small browser-like DOM API that can render HTML/CSS into native desktop windows.

[中文文档](./README-zh.md)

## What is this?

`@ylcc/napi-blitz` lets JavaScript create and mutate a Blitz-backed HTML document from Node-compatible runtimes. It is useful for experiments, native UI prototypes, DOM renderer adapters, and testing Blitz layout/event behavior without embedding a browser engine.

Highlights:

- `Native OS windows` driven by Blitz and winit.
- `Single-file startup`: after installing the dependency, a single `JS/TS file` is enough to open a native window, build DOM nodes, and run the event loop. Handy for `lightweight`, `low-stakes` tools and prototypes.
- No `Electron-style IPC` layer and no `Tauri-style WebView`: your JS calls native DOM bindings directly.
- Standard-ish DOM wrappers: `document.createElement`, `appendChild`, `textContent`, `setAttribute`, `querySelector`, event listeners, inline styles, etc.
- `Multiple windows` from one `NativeApp`.
- Prebuilt N-API packages for supported platforms.
- TypeScript declarations included.

> This is not a browser shell. It does not embed Chromium/WebKit/WebView, and it does not shuttle UI updates through an IPC bridge like Electron. Your application code runs in Node/Bun/Deno and mutates the Blitz-backed native DOM objects directly.

## Screenshot

![screenshot](https://raw.githubusercontent.com/jerry4718/napi-blitz/main/screenshots/demo-counter.png)

## Installation

### npm

```bash
npm install @ylcc/napi-blitz
```

### pnpm

```bash
pnpm add @ylcc/napi-blitz
```

### yarn

```bash
yarn add @ylcc/napi-blitz
```

### Bun

```bash
bun add @ylcc/napi-blitz
```

### Deno

Deno can load npm packages with native Node-API addons, but it needs FFI permission:

```ts
// main.ts
import napiBlitz from "npm:@ylcc/napi-blitz";

const { BlitzApp } = napiBlitz;
```

```bash
deno run --allow-ffi --allow-env --allow-read main.ts
```

## Quick start

> 💡 Tip: In environments that support top-level `await`, the `main` function wrapper is not required.

### Open a window

```ts
import { BlitzApp, HTMLDocument, WindowOptions } from "@ylcc/napi-blitz";

const document = HTMLDocument.create({
  baseHtml: `<!doctype html>
<html>
<head>
  <title>napi-blitz demo</title>
  <style>
    body { margin: 24px; font-family: sans-serif; }
    button { padding: 8px 12px; }
  </style>
</head>
<body></body>
</html>`,
});

async function main() {
  const app = BlitzApp.create();
  // Start the pump loop first: `openWindow` only resolves once a pump
  // creates the window. The loop runs in the background and exits on its
  // own once every window is closed.
  app.pumpLoop();
  const window = await app.openWindow(
    document,
    WindowOptions.builder()
      .title("napi-blitz demo")
      .size(800, 600),
  );

  const button = document.createElement("button");
  let count = 0;

  button.textContent = `Clicked ${count} times`;
  button.addEventListener("click", () => {
    count += 1;
    button.textContent = `Clicked ${count} times`;
  });

  document.body!.appendChild(button);
}

main();
```

### CommonJS

```js
const { BlitzApp, HTMLDocument, WindowOptions } = require("@ylcc/napi-blitz");

async function main() {
  const doc = HTMLDocument.create();
  const app = BlitzApp.create();
  // Start the pump loop first: `openWindow` only resolves once a pump
  // creates the window.
  app.pumpLoop();
  const win = await app.openWindow(doc, WindowOptions.builder().title("CommonJS demo"));

  doc.body.textContent = "Hello from CommonJS";
}

main();
```

### DOM mutation and style

```ts
import { BlitzApp, HTMLDocument, WindowOptions } from "@ylcc/napi-blitz";

const document = HTMLDocument.create({
  baseHtml: `<!doctype html><html><body></body></html>`,
});

async function main() {
  const app = BlitzApp.create();
  app.pumpLoop();
  const win = await app.openWindow(
    document,
    WindowOptions.builder().title("DOM demo"),
  );

  const card = document.createElement("section");
  card.setAttribute("class", "card");
  card.style.padding = "16px";
  card.style.border = "1px solid #999";
  card.style.borderRadius = "8px";
  card.textContent = "Created with the DOM API";

  document.body!.appendChild(card);
}

main();
```

### Multiple windows

```ts
import { BlitzApp, HTMLDocument, WindowOptions } from "@ylcc/napi-blitz";

async function main() {
  const app = BlitzApp.create();
  app.pumpLoop();
  const docA = HTMLDocument.create();
  const docB = HTMLDocument.create();
  const a = await app.openWindow(docA, WindowOptions.builder().title("Window A").size(360, 240));
  const b = await app.openWindow(docB, WindowOptions.builder().title("Window B").size(360, 240));

  docA.body.textContent = "A";
  docB.body.textContent = "B";
}

main();
```

## Examples in this repository

```bash
pnpm install
pnpm run build:debug

pnpm --dir examples/html-tags start
pnpm --dir examples/vue-jsx-dom start
pnpm --dir examples/vue-jsx-multi-window start
```

Examples:

- `examples/html-tags`: DOM-only HTML tag matrix.
- `examples/vue-jsx-dom`: Vue 3 custom renderer targeting the napi-blitz DOM API.
- `examples/vue-jsx-multi-window`: multi-window Vue renderer demo.

## Supported platforms

The package publishes prebuilt N-API binaries for the platforms that pass the CI build matrix. Linux and FreeBSD builds load `fontconfig` at runtime for system font discovery.

| Target | Status | Notes |
| --- | --- | --- |
| `x86_64-apple-darwin` | Supported | macOS x64. |
| `aarch64-apple-darwin` | Supported | macOS Apple Silicon. |
| `x86_64-pc-windows-msvc` | Supported | Windows x64. |
| `aarch64-pc-windows-msvc` | Supported | Windows ARM64 build artifact. |
| `x86_64-unknown-linux-gnu` | Supported | Built with napi-cross. Requires `fontconfig` at runtime. |
| `x86_64-unknown-linux-musl` | Supported | Built with zig/cargo-zigbuild. Requires `fontconfig` at runtime. |
| `aarch64-unknown-linux-gnu` | Supported | Cross-compiled with napi-cross. Requires `fontconfig` at runtime. |
| `aarch64-unknown-linux-musl` | Supported | Cross-compiled with zig/cargo-zigbuild. Requires `fontconfig` at runtime. |
| `x86_64-unknown-freebsd` | Supported | Built in a FreeBSD VM. Requires `fontconfig` and `python3` while building from source. |
| `i686-pc-windows-msvc` | Temporarily disabled | Blocked by a 32-bit `anyrender` `FilterEffect` size assertion. See the local `ci-32bit-anyrender-patch` branch for the experimental patched build. |
| `armv7-unknown-linux-gnueabihf` | Temporarily disabled | Blocked by the same 32-bit `anyrender` assertion. See the local `ci-32bit-anyrender-patch` branch for the experimental patched build. |
| `wasm32-wasip1-threads` | Disabled | Kept in CI comments until the WASI build/test path is fixed. |

When building Linux targets from source, the CI enables vendored OpenSSL and runtime-loaded fontconfig to avoid cross `pkg-config` sysroot requirements.

## Runtime dependencies

Linux and FreeBSD builds use Blitz system font integration, so minimal runtime images need `fontconfig` available at runtime. `pkg-config` and development headers are only needed when building from source.

Most desktop Linux distributions already include these libraries. Slim containers usually do not.

```bash
# Debian / Ubuntu runtime images
apt-get install -y fontconfig libfontconfig1

# Alpine runtime images
apk add --no-cache fontconfig

# FreeBSD
pkg install -y fontconfig
```

## Development

Requirements:

- Rust toolchain
- Node.js with Node-API support
- pnpm

```bash
corepack enable
pnpm install
pnpm run build:debug
pnpm test
```

Useful scripts:

```bash
pnpm run fmt
pnpm run fmt:check
pnpm run lint:strict
pnpm run build:debug
pnpm test
```

## Acknowledgements

This project exists on top of a lot of serious work from the Rust UI and web-platform ecosystem, especially [Blitz](https://github.com/DioxusLabs/blitz), [winit](https://github.com/rust-windowing/winit), [napi-rs](https://github.com/napi-rs/napi-rs), [Servo](https://github.com/servo/servo), [Stylo](https://github.com/servo/servo/tree/main/components/style), and [Rust](https://github.com/rust-lang/rust) itself.

## License

MIT
