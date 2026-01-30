import { ComponentInternalInstance, createRenderer, ElementNamespace, VNodeProps } from 'vue'
import { BlitzApp, Document, Node } from '@ylcc/napi-blitz'
import { App } from './App.tsx'
import process from 'node:process'
import fs from 'node:fs'
import path from 'node:path'

const HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>💗️Hello Blitz(napi-rs Demo)~~~</title>
<style>
    html, body { border: 0; margin: 0; padding: 0; }
</style>
</head>
<body>
</body>
</html>
`

const DEFAULT_CSS = fs.readFileSync(path.resolve('assets/default.css'), { encoding: 'utf-8' })

const document = new Document([
  DEFAULT_CSS,
  'body { background: #ccc }',
  'div { display: block }',
])
document.loadHtml(HTML)

function styleKey(key: string) {
  if (/^--/.test(key)) return key
  return key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)
}

const { createApp } = createRenderer<Node, Node>({
  cloneNode(node: Node): Node {
    console.log('cloneNode')
    node.printTree()
    return document.deepCloneNode(node)
  },
  createComment(text: string): Node {
    return document.createCommentNodeWithContent(text)
  },
  createElement(
    type: string,
    _namespace: ElementNamespace | undefined,
    _isCustomizedBuiltIn: string | undefined,
    _vNodeProps: (VNodeProps & { [p: string]: any }) | null | undefined,
  ): Node {
    return document.createElement(type, [])
  },
  createText(text: string): Node {
    return document.createTextNode(text)
  },
  insert(el: Node, parent: Node, anchor: Node | null | undefined): void {
    document.insert(el, parent, anchor)
  },
  // insertStaticContent(content: string, parent: Node, anchor: Node | null, namespace: ElementNamespace, start: Node | null | undefined, end: Node | null | undefined): [Node, Node] {
  //   console.log('insertStaticContent', { content, parent, anchor, namespace, start, end })
  //   return [0, 0]
  // },
  nextSibling(node: Node): Node | null {
    return document.nextSibling(node)
  },
  parentNode(node: Node): Node | null {
    return document.parentNode(node)
  },
  patchProp(
    el: Node,
    key: string,
    prevValue: any,
    nextValue: any,
    _namespace: ElementNamespace | undefined,
    _parentComponent: ComponentInternalInstance | null | undefined,
  ): void {
    if (key === 'style') {
      for (const [key, value] of Object.entries(nextValue)) {
        // console.log('setStyle', { key, value })
        document.setStyleProperty(el, styleKey(key), value as string)
      }
      return
    }
    if (/^on(Click|Mouse(move|down|up)|Key(press|down|up)|Input)$/.test(key)) {
      const event = key.replace(/^on/, '').toLowerCase()
      console.log('addEventListener', { event, listener: nextValue })
      if (prevValue) {
        el.removeEventListener(event, prevValue)
      }
      el.addEventListener(event, nextValue)
      return
    }
    if (typeof nextValue === 'string') {
      console.log('patchProp', { key, nextValue })
      document.patchProp(el, key, nextValue)
      return
    }
    console.log('unknownProp', { key, nextValue })
    el.selfProp(key, nextValue)
  },
  querySelector(selector: string): Node | null {
    return document.querySelector(selector)
  },
  remove(el: Node): void {
    document.remove(el)
  },
  setElementText(node: Node, text: string): void {
    document.setElementText(node, text)
  },
  setScopeId(el: Node, id: string): void {
    document.patchProp(el, id, '')
  },
  setText(node: Node, text: string): void {
    document.setText(node, text)
  },
})

function randomHex() {
  return Math.floor(Math.random() * 0xFFFFFF).toString(16).padStart(6, '0')
}

export async function bootstrap() {
  const blitz = BlitzApp.create()
  document.loadHtml(HTML)
  blitz.openWindow(document)

  const head = document.querySelector('head')
  const body = document.querySelector('body')
  const app = document.createElement('div', [{ name: 'id', value: 'app' }])
  document.insert(app, body)
  createApp(App).mount(app)

  document.querySelector('html')?.addEventListener('keydown', () => {
    document.getNode(0)?.printTree(0)
  })

  body?.addEventListener('click', () => {
    const hex = randomHex()
    const className = `class-${hex}`
    const div = document.createElement('div', [{ name: 'class', value: className }])
    const style = document.createElement('style', [])
    const styleText = document.createTextNode(
      `.${className} { background-color: #${hex}; height: 50px; }`,
    )
    document.insert(styleText, style)
    document.insert(style, head)
    document.insert(div, app)
  })

  while (true) {
    const pump = blitz.pumpAppEvents(0)
    if (pump.exit) {
      return process.exit(pump.code)
    }
    await new Promise(resolve => setTimeout(resolve, 16))
  }
}


