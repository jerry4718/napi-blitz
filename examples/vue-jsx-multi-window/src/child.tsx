// Child window mount + the Vue component running inside it. Each
// child runs its own renderer (one per HTMLDocument) and provides the
// usual `BlitzDocument` / `BlitzWindow` / `BlitzApp` injections so
// nested components can talk to the host.

import { defineComponent, inject, ref } from 'vue'
import type { BlitzApp, Window } from '@ylcc/napi-blitz'
import { BlitzAppKey, BlitzDocumentKey, BlitzWindowKey } from './keys.ts'
import { createRendererFor } from './renderer.ts'
import { randomColor } from './utils/color.ts'

/** Boilerplate HTML that every child window starts from. */
export function childBaseHtml(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
<title>Child window</title>
<style>
  html, body { border: 0; margin: 0; padding: 0; font-family: sans-serif; }
</style>
</head>
<body></body>
</html>`
}

/**
 * Mount the child Vue app into `window.document.body`. The `id` is
 * just a label used in the title and the inner heading.
 */
export function mountChild(app: BlitzApp, window: Window, id: number): void {
    const document = window.document
    document.title = `Child #${id}`

    const body = document.body!
    const mountEl = document.createElement('div')
    mountEl.setAttribute('id', 'app')
    body.appendChild(mountEl)

    const createApp = createRendererFor(document)
    const vueApp = createApp(ChildApp, { id })

    // Inject the host context so deeply-nested children can reach it.
    vueApp.provide(BlitzDocumentKey, document)
    vueApp.provide(BlitzWindowKey, window)
    vueApp.provide(BlitzAppKey, app)

    vueApp.mount(mountEl)

    // When the OS / user requests close on this child we just let it
    // close — there is no cancel logic here. But unmount Vue first so
    // its destroy hooks run while the document is still live.
    window.addEventListener('close', () => {
        vueApp.unmount()
    })
}

const ChildApp = defineComponent({
    props: { id: { type: Number, required: true } },
    setup(props) {
        const document = inject(BlitzDocumentKey)!
        const window = inject(BlitzWindowKey)!
        const tint = ref(randomColor())

        function rerollTint() {
            tint.value = randomColor()
        }

        function bumpTitle() {
            // Demonstrates that mutating `document.title` updates the
            // live `<title>` element (and the OS window title).
            document.title = `Child #${props.id} · ${Date.now() % 1000}`
        }

        function closeMe() {
            window.close()
        }

        return () => (
            <div
                style={{
                    height: 'calc(100vh - 2px)',
                    border: '1px solid #aaa',
                    padding: '20px',
                    background: tint.value.alpha(0.25).toRgbString(),
                    color: '#222',
                }}>
                <h1 style={{ marginTop: '0', fontSize: '24px' }}>Child #{props.id}</h1>
                <p style={{ color: '#444' }}>
                    document.title: <strong>{document.title}</strong>
                </p>
                <p style={{ color: '#666', fontSize: '14px' }}>
                    docId: {window.document.documentElement.tagName === 'html'
                        ? '(have <html>)'
                        : '(no <html>)'}
                </p>
                <div style={{ display: 'flex', gap: '12px', marginTop: '16px' }}>
                    <button
                        style={btnStyle('#3498db')}
                        onClick={rerollTint}>
                        Reroll tint
                    </button>
                    <button
                        style={btnStyle('#9b59b6')}
                        onClick={bumpTitle}>
                        Mutate title
                    </button>
                    <button
                        style={btnStyle('#e74c3c')}
                        onClick={closeMe}>
                        Close window
                    </button>
                </div>
            </div>
        )
    },
})

function btnStyle(bg: string) {
    return {
        padding: '8px 14px',
        fontSize: '15px',
        background: bg,
        color: '#fff',
        border: '0',
        borderRadius: '6px',
    }
}
