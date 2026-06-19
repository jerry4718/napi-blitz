// Per-document Vue 3 custom renderer factory.
//
// `vue-jsx-dom` keeps a single module-level `nodeGlobalDoc` cell so its
// renderer closures can reach the live HTMLDocument. That works for one
// window but breaks the moment we have two: every `createElement`
// would race on which document it targets.
//
// Here we instead build one renderer *per* document. `createApp(...)`
// closes over the document handed in, and the resulting Vue app
// always allocates DOM nodes on the right document.

import {
    ComponentInternalInstance,
    createRenderer,
    ElementNamespace,
    VNodeProps,
} from 'vue'
import { Element, HTMLDocument, HTMLElement, Node } from '@ylcc/napi-blitz'

/**
 * The renderer's host types. `HostNode` is `Node` (which is what the
 * `insert` / `nextSibling` / `parentNode` operations work with), and
 * `HostElement` is `Element` — Vue uses `HostElement` for things like
 * `createElement` results and the mount container.
 *
 * `HTMLElement` is what we actually create; it `extends Element` so
 * the assignment is sound.
 */
export type AppCreator = ReturnType<
    typeof createRenderer<Node, Element>
>['createApp']

/**
 * Build a Vue renderer bound to `document`. The returned `createApp`
 * function mounts a Vue tree into nodes belonging to that document.
 */
export function createRendererFor(document: HTMLDocument): AppCreator {
    const { createApp } = createRenderer<Node, Element>({
        cloneNode(node: Node): Node {
            return node.cloneNode(true)
        },

        createComment(text: string): Node {
            return document.createComment(text)
        },

        createElement(
            type: string,
            _namespace: ElementNamespace | undefined,
            _isCustomizedBuiltIn: string | undefined,
            _vNodeProps: (VNodeProps & { [p: string]: any }) | null | undefined,
        ): Element {
            return document.createElement(type)
        },

        createText(text: string): Node {
            return document.createTextNode(text)
        },

        insert(el: Node, parent: Element, anchor: Node | null | undefined): void {
            if (anchor) {
                parent.insertBefore(el, anchor)
            } else {
                parent.appendChild(el)
            }
        },

        nextSibling(node: Node): Node | null {
            return node.nextSibling
        },

        parentNode(node: Node): Element | null {
            const parent = node.parentNode
            return parent && parent.nodeType === 1 ? (parent as Element) : null
        },

        patchProp(
            el: Element,
            key: string,
            prevValue: any,
            nextValue: any,
            _namespace: ElementNamespace | undefined,
            _parentComponent: ComponentInternalInstance | null | undefined,
        ): void {
            if (prevValue === nextValue) return

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

            // Vue passes event listeners as `onClick`, `onMousemove`, etc.
            if (/^on[A-Z]/.test(key)) {
                const event = key.replace(/^on/, '').toLowerCase()
                if (prevValue) el.removeEventListener(event, prevValue as EventListener)
                if (nextValue) el.addEventListener(event, nextValue as EventListener)
                return
            }

            // String/boolean attributes via the standard setAttribute path.
            if (typeof nextValue === 'string' || typeof nextValue === 'boolean') {
                el.setAttribute(key, String(nextValue))
                return
            }

            if (nextValue == null) {
                el.removeAttribute(key)
            } else {
                el.setAttribute(key, String(nextValue))
            }
        },

        querySelector(selector: string): Element | null {
            return document.querySelector(selector)
        },

        remove(el: Node): void {
            el.remove()
        },

        setElementText(node: Node, text: string): void {
            node.textContent = text
        },

        setScopeId(el: Element, id: string): void {
            el.setAttribute(id, '')
        },

        setText(node: Node, text: string): void {
            node.textContent = text
        },
    })

    return createApp
}
