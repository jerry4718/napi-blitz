// Vue 3 custom renderer that targets napi-blitz's standard DOM API.
//
// Each Vue renderer operation maps to the corresponding DOM method on
// `HTMLElement` / `Node` rather than the old flat `document.*` helpers.
// This mirrors how a real web renderer would talk to the DOM.

import {
  ComponentInternalInstance,
  createRenderer,
  ElementNamespace,
  VNodeProps,
} from 'vue'
import {
  BlitzApp,
  HTMLElement,
  HTMLDocument,
  Node,
  WindowOptions,
} from '@ylcc/napi-blitz'
import { App } from './App.tsx'
import { DOCUMENT_KEY } from './utils/useDocument.ts'
import { WINDOW_KEY } from './utils/useWindow.ts'
import process from 'node:process'

const BASE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>Blitz Date Picker Demo</title>
<style>
  html, body { border: 0; margin: 0; padding: 0; }
</style>
</head>
<body></body>
</html>`

let nodeGlobalDoc: HTMLDocument | null = null

const { createApp } = createRenderer<Node, Node>({
  cloneNode(node: Node): Node {
    return node.cloneNode(true)
  },

  createComment(text: string): Node {
    return (nodeGlobalDoc as HTMLDocument).createComment(text)
  },

  createElement(
    type: string,
    _namespace: ElementNamespace | undefined,
    _isCustomizedBuiltIn: string | undefined,
    _vNodeProps: (VNodeProps & { [p: string]: any }) | null | undefined,
  ): Node {
    return nodeGlobalDoc!.createElement(type)
  },

  createText(text: string): Node {
    return (nodeGlobalDoc as HTMLDocument).createTextNode(text)
  },

  insert(el: Node, parent: Node, anchor: Node | null | undefined): void {
    if (anchor) {
      parent.insertBefore(el, anchor)
    } else {
      parent.appendChild(el)
    }
  },

  nextSibling(node: Node): Node | null {
    return node.nextSibling
  },

  parentNode(node: Node): Node | null {
    return node.parentNode
  },

  patchProp(
    el: Node,
    key: string,
    prevValue: any,
    nextValue: any,
    _namespace: ElementNamespace | undefined,
    _parentComponent: ComponentInternalInstance | null | undefined,
  ): void {
    if (prevValue === nextValue) return

    // Skip patching on non-element nodes (text, comment, etc.)
    if (typeof (el as any).setAttribute !== 'function') return

    // Inline styles: Vue passes an object. Diff key-by-key against the
    // previous value, removing stale properties and setting new ones.
    if (key === 'style') {
      const htmlEl = el as unknown as HTMLElement
      const prev = (prevValue ?? {}) as Record<string, string>
      const next = (nextValue ?? {}) as Record<string, string>

      for (const k of Object.keys(prev)) {
        if (next[k] === undefined) {
          delete htmlEl.style[k]
        }
      }
      for (const [k, v] of Object.entries(next)) {
        if (v !== prev[k]) {
          htmlEl.style[k] = String(v)
        }
      }
      return
    }

    // Event listeners: `onClick` -> `click`.
    if (/^on[A-Z]/.test(key)) {
      const event = key.replace(/^on/, '').toLowerCase()
      if (prevValue) el.removeEventListener(event, prevValue as EventListener)
      if (nextValue) el.addEventListener(event, nextValue as EventListener)
      return
    }

    // String / boolean attributes via the standard `setAttribute` path.
    if (typeof nextValue === 'string' || typeof nextValue === 'boolean') {
      ;(el as unknown as HTMLElement).setAttribute(key, String(nextValue))
      return
    }

    // Unknown non-string prop: set as attribute with String() coercion,
    // or remove if null/undefined.
    if (nextValue == null) {
      ;(el as unknown as HTMLElement).removeAttribute(key)
    } else {
      ;(el as unknown as HTMLElement).setAttribute(key, String(nextValue))
    }
  },

  querySelector(selector: string): Node | null {
    return nodeGlobalDoc!.querySelector(selector)
  },

  remove(el: Node): void {
    el.remove()
  },

  setElementText(node: Node, text: string): void {
    node.textContent = text
  },

  setScopeId(el: Node, id: string): void {
    ;(el as unknown as HTMLElement).setAttribute(id, '')
  },

  setText(node: Node, text: string): void {
    node.textContent = text
  },
})

export async function bootstrap() {
  const app = BlitzApp.create()

  const document = HTMLDocument.create({ baseHtml: BASE_HTML })
  const options = WindowOptions.builder()
  options.title('Blitz Date Picker Demo')
  const window = app.openWindow(document, options)
  nodeGlobalDoc = document

  const body = document.body!
  const mountEl = document.createElement('div')
  mountEl.setAttribute('id', 'app')
  body.appendChild(mountEl)

  const vueApp = createApp(App)
  vueApp.provide(DOCUMENT_KEY, document)
  vueApp.provide(WINDOW_KEY, window)
  vueApp.mount(mountEl)

  await pump(app)
}

async function pump(app: BlitzApp) {
  while (true) {
    const result = app.pumpAppEvents(0)
    if (result.exit) {
      process.exit(result.code ?? 0)
    }
    await new Promise((resolve) => setTimeout(resolve, 16))
  }
}
