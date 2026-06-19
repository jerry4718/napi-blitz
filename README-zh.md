# `@ylcc/napi-blitz`

![CI](https://github.com/jerry4718/napi-blitz/workflows/CI/badge.svg)

`@ylcc/napi-blitz` 是 [Blitz](https://github.com/DioxusLabs/blitz) 的 Node.js 原生绑定，基于 [napi-rs](https://napi.rs/) 构建，提供一套类似浏览器 DOM 的 JavaScript API，用来把 HTML/CSS 渲染到原生桌面窗口。

[English README](./README.md)

## 这是什么？

它允许你在 Node 兼容运行时里创建 Blitz 文档、打开原生窗口，并通过 DOM API 修改界面。适合做原生 UI 实验、DOM renderer 适配、Blitz 布局/事件调试，或不想嵌入完整浏览器引擎的桌面原型。

主要特性：

- 基于 Blitz + winit 的 `原生 OS 窗口`。
- `单文件启动`：安装好依赖以后，只需要写一个 `JS/TS 文件`，就能打开原生窗口、构建 DOM、跑事件循环，适合一些 `轻量`、`不那么敏感` 的小工具和原型场景。
- 不是 `Electron IPC` 方案，也不是 `Tauri WebView` 方案：JS 直接调用原生 DOM 绑定。
- 类浏览器 DOM 封装：`document.createElement`、`appendChild`、`textContent`、`setAttribute`、`querySelector`、事件监听、内联样式等。
- 一个 `BlitzApp` 可管理 `多个窗口`。
- 发布平台对应的 N-API 预编译包。
- 内置 TypeScript 类型声明。

> 它不是浏览器壳，不嵌入 Chromium/WebKit/WebView，也不会像 Electron 那样通过 IPC 桥转发 UI 更新。你的应用代码运行在 Node/Bun/Deno 中，并直接操作 Blitz-backed 原生 DOM 对象。

## 截图

![screenshot](https://raw.githubusercontent.com/jerry4718/napi-blitz/main/screenshots/demo-counter.png)

## 安装

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

Deno 可以加载 npm 包里的原生 Node-API addon，但运行时需要开启 FFI 权限：

```ts
// main.ts
import napiBlitz from "npm:@ylcc/napi-blitz";

const { BlitzApp } = napiBlitz;
```

```bash
deno run --allow-ffi --allow-env --allow-read main.ts
```

## 运行时依赖

Linux 和 FreeBSD 构建会使用 Blitz 的系统字体集成，所以在精简运行时镜像里需要有 `fontconfig`。`pkg-config` 和开发头文件只在从源码构建时需要，运行时不需要。

大多数桌面 Linux 发行版默认已经带有这些库，但 slim 容器通常没有。

```bash
# Debian / Ubuntu 运行时镜像
apt-get install -y fontconfig libfontconfig1

# Alpine 运行时镜像
apk add --no-cache fontconfig

# FreeBSD
pkg install -y fontconfig
```

## 支持平台

本包会为通过 CI 构建矩阵的平台发布预编译 N-API 二进制。Linux 和 FreeBSD 构建会在运行时加载 `fontconfig`，用于系统字体发现。

| Target | 状态 | 说明 |
| --- | --- | --- |
| `x86_64-apple-darwin` | 支持 | macOS x64。 |
| `aarch64-apple-darwin` | 支持 | macOS Apple Silicon。 |
| `x86_64-pc-windows-msvc` | 支持 | Windows x64。 |
| `aarch64-pc-windows-msvc` | 支持 | Windows ARM64 构建产物。 |
| `x86_64-unknown-linux-gnu` | 支持 | 通过 napi-cross 构建。运行时需要 `fontconfig`。 |
| `x86_64-unknown-linux-musl` | 支持 | 通过 zig/cargo-zigbuild 构建。运行时需要 `fontconfig`。 |
| `aarch64-unknown-linux-gnu` | 支持 | 通过 napi-cross 交叉编译。运行时需要 `fontconfig`。 |
| `aarch64-unknown-linux-musl` | 支持 | 通过 zig/cargo-zigbuild 交叉编译。运行时需要 `fontconfig`。 |
| `x86_64-unknown-freebsd` | 支持 | 在 FreeBSD VM 中构建。从源码构建时需要 `fontconfig` 和 `python3`。 |
| `i686-pc-windows-msvc` | 暂时禁用 | 受 `anyrender` 32 位 `FilterEffect` size assertion 阻塞。本地 `ci-32bit-anyrender-patch` 分支保留了实验性的 patched build。 |
| `armv7-unknown-linux-gnueabihf` | 暂时禁用 | 受同一个 `anyrender` 32 位断言阻塞。本地 `ci-32bit-anyrender-patch` 分支保留了实验性的 patched build。 |
| `wasm32-wasip1-threads` | 禁用 | 保留在 CI 注释里，等待 WASI build/test 流程修复后再启用。 |

从源码构建 Linux targets 时，CI 会启用 vendored OpenSSL，并让 fontconfig 走运行时加载，以避免交叉编译时配置 `pkg-config` sysroot。

## 快速开始

### 打开一个窗口

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

### DOM 修改和样式

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

### 多窗口

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

## 仓库内示例

```bash
pnpm install
pnpm run build:debug

pnpm --dir examples/html-tags start
pnpm --dir examples/vue-jsx-dom start
pnpm --dir examples/vue-jsx-multi-window start
```

示例说明：

- `examples/html-tags`：纯 DOM API 的 HTML 标签矩阵。
- `examples/vue-jsx-dom`：Vue 3 custom renderer，渲染目标是 napi-blitz DOM API。
- `examples/vue-jsx-multi-window`：多窗口 Vue renderer demo。

## 开发

环境要求：

- Rust toolchain
- 支持 Node-API 的 Node.js
- 通过 Corepack 使用 pnpm

```bash
corepack enable
pnpm install
pnpm run build:debug
pnpm test
```

常用脚本：

```bash
pnpm run fmt
pnpm run fmt:check
pnpm run lint:strict
pnpm run build:debug
pnpm test
```

## 致谢

这个项目建立在 Rust UI 和 Web Platform 生态的大量基础工作之上，尤其是 [Blitz](https://github.com/DioxusLabs/blitz)、[winit](https://github.com/rust-windowing/winit)、[napi-rs](https://github.com/napi-rs/napi-rs)、[Servo](https://github.com/servo/servo)、[Stylo](https://github.com/servo/servo/tree/main/components/style)，以及 [Rust](https://github.com/rust-lang/rust) 本身。

## License

MIT
