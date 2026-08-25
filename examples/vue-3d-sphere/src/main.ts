// Vue 3 custom renderer targeting napi-blitz's standard DOM API.
// Same renderer as vue-jsx-dom — kept minimal for the sphere demo.

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

const BASE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>3D Sphere Demo</title>
<style>
  html, body { border: 0; margin: 0; padding: 0; overflow: hidden; }
</style>
</head>
<body></body>
</html>`

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

    if (key === 'style') {
      const next = (nextValue ?? {}) as Record<string, string>;

      const htmlEl = el as unknown as HTMLElement
      for (const [k, v] of Object.entries(next)) {
        htmlEl.style[k] = String(v)
      }

      // const htmlEl = el as unknown as HTMLElement
      // const prev = (prevValue ?? {}) as Record<string, string>
      // const next = (nextValue ?? {}) as Record<string, string>
      // for (const k of Object.keys(prev)) {
      //   if (next[k] === undefined) delete htmlEl.style[k]
      // }
      // for (const [k, v] of Object.entries(next)) {
      //   if (v !== prev[k]) htmlEl.style[k] = String(v)
      // }
      return
    }

    if (/^on[A-Z]/.test(key)) {
      const event = key.replace(/^on/, '').toLowerCase()
      if (prevValue) el.removeEventListener(event, prevValue)
      if (nextValue) el.addEventListener(event, nextValue)
      return
    }

    if (typeof nextValue === 'string' || typeof nextValue === 'boolean') {
      ;(el as unknown as HTMLElement).setAttribute(key, String(nextValue))
      return
    }

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

let nodeGlobalDoc: HTMLDocument | null = null

export async function bootstrap() {
  const app = BlitzApp.create()
  app.pumpLoop()

  const document = HTMLDocument.create({ baseHtml: BASE_HTML })
  const options = WindowOptions.builder()
  options.title('3D Sphere Demo')
  const window = await app.openWindow(document, options)
  nodeGlobalDoc = document

  const body = document.body!
  const mountEl = document.createElement('div')
  mountEl.setAttribute('id', 'app')
  body.appendChild(mountEl)

  const vueApp = createApp(App);

  vueApp.mount(mountEl)

  window.addEventListener("close", () => vueApp.unmount())
}

