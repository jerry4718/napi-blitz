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
  type OpenWindowInit,
} from '@ylcc/napi-blitz'
import { App } from './App.tsx'
import process from 'node:process'

const BASE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>Blitz DOM Demo</title>
<style>
  html, body { border: 0; margin: 0; padding: 0; }
</style>
</head>
<body></body>
</html>`

/** Convert a camelCase style key to kebab-case. */
function styleKey(key: string): string {
  if (key.startsWith('--')) return key
  return key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)
}

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

    // Inline styles: Vue passes an object. Diff key-by-key against the
    // previous value, removing stale properties and setting new ones.
    if (key === 'style') {
      const htmlEl = el as unknown as HTMLElement
      const prev = (prevValue ?? {}) as Record<string, string>
      const next = (nextValue ?? {}) as Record<string, string>

      // Remove keys that disappeared or changed.
      for (const k of Object.keys(prev)) {
        if (next[k] === undefined) {
          htmlEl.removeStyle(styleKey(k))
        }
      }
      // Set new / changed keys.
      for (const [k, v] of Object.entries(next)) {
        if (v !== prev[k]) {
          htmlEl.setStyle(styleKey(k), String(v))
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

// `createRenderer` builds the renderer once, before `bootstrap()` opens a
// window. We stash the live `HTMLDocument` in a module-level cell so the
// `createElement` / `createText` / `querySelector` closures can reach it
// once `bootstrap` assigns it.
let nodeGlobalDoc: HTMLDocument | null = null

export async function bootstrap() {
  const app = BlitzApp.create()

  const windowInit: OpenWindowInit = {
    baseHtml: BASE_HTML,
    title: 'Blitz DOM Demo',
  }
  const window = app.openWindow(windowInit)
  const document = window.document
  nodeGlobalDoc = document

  const body = document.body!
  const mountEl = document.createElement('div')
  mountEl.setAttribute('id', 'app')
  body.appendChild(mountEl)

  createApp(App).mount(mountEl)

  // Print the live tree on keydown (handy for debugging the DOM).
  document.documentElement.addEventListener('keydown', () => {
    console.log(document.body?.outerHTML ?? '<no body>')
  })

  // Click anywhere outside the Vue app to spawn a random-colour block.
  // This exercises the standard DOM creation / mutation / query APIs.
  body.addEventListener('click', (ev) => {
    // Don't fire when the click lands inside the Vue app container.
    let target = ev.target as HTMLElement | null
    while (target && target !== body) {
      if (target.getAttribute('id') === 'app') return
      target = target.parentElement
    }

    const hex = randomHex()
    const className = `block-${hex}`

    // Create a <style> rule for this block.
    const style = document.createElement('style')
    style.textContent = `.${className} { background-color: #${hex}; height: 40px; margin: 4px; border-radius: 6px; }`
    document.head!.appendChild(style)

    const block = document.createElement('div')
    block.setAttribute('class', className)
    block.textContent = `Block #${document.getElementsByTagName('div').length}`
    body.appendChild(block)

    // Demonstrate element-scoped queries: count blocks inside body only.
    const blocksInBody = body.getElementsByClassName('block')
    console.log(`blocks in body: ${blocksInBody.length}`)
  })

  await pump(app)
}

function randomHex(): string {
  return Math.floor(Math.random() * 0xffffff)
    .toString(16)
    .padStart(6, '0')
}

async function pump(app: BlitzApp) {
  while (true) {
    const result = app.pumpAppEvents(0)
    if (result.exit) {
      process.exit(result.code)
    }
    await new Promise((resolve) => setTimeout(resolve, 16))
  }
}
