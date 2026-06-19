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
- `Multiple windows` from one `BlitzApp`.
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

## Quick start

### Open a window

```ts
import { BlitzApp } from "@ylcc/napi-blitz";

const app = BlitzApp.create();
const window = app.openWindow({
  title: "napi-blitz demo",
  width: 800,
  height: 600,
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

const { document } = window;
const button = document.createElement("button");
let count = 0;

button.textContent = `Clicked ${count} times`;
button.addEventListener("click", () => {
  count += 1;
  button.textContent = `Clicked ${count} times`;
});

document.body!.appendChild(button);

while (!window.closed) {
  app.pumpAppEvents(16);
}
```

### CommonJS

```js
const { BlitzApp } = require("@ylcc/napi-blitz");

const app = BlitzApp.create();
const win = app.openWindow({ title: "CommonJS demo" });

win.document.body.textContent = "Hello from CommonJS";

while (!win.closed) {
  app.pumpAppEvents(16);
}
```

### DOM mutation and style

```ts
import { BlitzApp } from "@ylcc/napi-blitz";

const app = BlitzApp.create();
const win = app.openWindow({
  title: "DOM demo",
  baseHtml: `<!doctype html><html><body></body></html>`,
});

const card = win.document.createElement("section");
card.setAttribute("class", "card");
card.style.padding = "16px";
card.style.border = "1px solid #999";
card.style.borderRadius = "8px";
card.textContent = "Created with the DOM API";

win.document.body!.appendChild(card);

while (!win.closed) {
  app.pumpAppEvents(16);
}
```

### Multiple windows

```ts
import { BlitzApp } from "@ylcc/napi-blitz";

const app = BlitzApp.create();
const a = app.openWindow({ title: "Window A", width: 360, height: 240 });
const b = app.openWindow({ title: "Window B", width: 360, height: 240 });

a.document.body.textContent = "A";
b.document.body.textContent = "B";

while (!a.closed || !b.closed) {
  app.pumpAppEvents(16);
}
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

## Development

Requirements:

- Rust toolchain
- Node.js with Node-API support
- pnpm via Corepack

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
