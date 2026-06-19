// Multi-window bootstrap.
//
// Demonstrates the multi-window flow on top of napi-blitz:
//
//   1. Open the main window.
//   2. Install a `close` listener on the main window that prevents
//      default while child windows are still alive — implementing the
//      requirement "the main window cannot close until every child
//      window is closed".
//   3. Mount Vue with provide/inject so deeply-nested components can
//      reach the host (HTMLDocument, Window, BlitzApp) and a
//      reactive list of currently-open child windows.
//   4. Pump the event loop. Exit when native reports the loop has
//      shut down (which only happens after every window is gone).

import { shallowRef } from 'vue'
import process from 'node:process'

import {
    BlitzApp,
    type Window as BlitzWindow,
} from '@ylcc/napi-blitz'

import { MainApp } from './components/MainApp.tsx'
import { createRendererFor } from './renderer.ts'
import {
    BlitzAppKey,
    BlitzDocumentKey,
    BlitzWindowKey,
    ChildWindowsKey,
} from './keys.ts'

const MAIN_BASE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>Main · napi-blitz multi-window</title>
<style>
  html, body { border: 0; margin: 0; padding: 0; font-family: sans-serif; }
</style>
</head>
<body></body>
</html>`

export async function bootstrap() {
    const app = BlitzApp.create()

    const main = app.openWindow({
        baseHtml: MAIN_BASE_HTML,
        title: 'Main · napi-blitz multi-window',
        width: 720,
        height: 520,
    })
    const mainDocument = main.document

    const childWindows = shallowRef<BlitzWindow[]>([])

    // Main window close-guard. Fires for *both* OS-initiated closes
    // (user clicked X) and programmatic ones (`main.close()` from JS).
    main.addEventListener('close', (event) => {
        if (childWindows.value.length > 0) {
            event.preventDefault()
            ;(globalThis as any).__notifyMainCloseBlocked?.()
        }
    })

    // Mount the Vue app for the main window, with all the host
    // injections components might need.
    const body = mainDocument.body!
    const mountEl = mainDocument.createElement('div')
    mountEl.setAttribute('id', 'app')
    body.appendChild(mountEl)

    const createApp = createRendererFor(mainDocument)
    const vueApp = createApp(MainApp)
    vueApp.provide(BlitzAppKey, app)
    vueApp.provide(BlitzWindowKey, main)
    vueApp.provide(BlitzDocumentKey, mainDocument)
    vueApp.provide(ChildWindowsKey, childWindows)
    vueApp.mount(mountEl)

    main.addEventListener('closed', () => {
        vueApp.unmount()
    })

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
