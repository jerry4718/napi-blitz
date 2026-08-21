// Main window: shows a "spawn child window" button, lists open
// children, and displays a banner when the user tries to close while
// children are still alive.
//
// Demonstrates the full `provide/inject` chain: `BlitzDocumentKey`
// gives a child component the live HTMLDocument; `BlitzAppKey` lets
// the spawn button open a new window via `app.openWindow`;
// `ChildWindowsKey` is a reactive `Ref<Window[]>` shared between the
// list and the spawn button.

import { computed, defineComponent, inject, ref } from 'vue'
import { HTMLDocument, pickFile, WindowOptions, type Window as BlitzWindow } from '@ylcc/napi-blitz'
import { BlitzAppKey, BlitzWindowKey, ChildWindowsKey } from './keys.ts'
import { childBaseHtml, mountChild } from './Child.tsx'

export const App = defineComponent({
    setup() {
        const app = inject(BlitzAppKey)!
        const mainWindow = inject(BlitzWindowKey)!
        const children = inject(ChildWindowsKey)!
        const closeBlocked = ref(false)
        const pickedFile = ref<string | null>(null)
        let nextChildId = 1

        async function spawn() {
            closeBlocked.value = false

            const id = nextChildId++
            const document = HTMLDocument.create({ baseHtml: childBaseHtml() })
            const options = WindowOptions.builder()
            options.title(`Child #${id}`).size(480, 320)
            // `openWindow` is async: when called from a click handler the
            // window is created by the current/next pump, and the promise
            // resolves once the OS window exists.
            const child = await app.openWindow(document, options)
            mountChild(app, child, id)
            children.value = [...children.value, child]

            // Drop the child from our registry once it dies — whether
            // by user X-click, programmatic close, or app shutdown.
            child.addEventListener('closed', () => {
                children.value = children.value.filter((w) => w !== child)
            })
        }

        async function spawnChild() {
            closeBlocked.value = false

            const id = nextChildId++
            const document = HTMLDocument.create({ baseHtml: childBaseHtml() })
            const handle = mainWindow.windowHandle()
            const options = WindowOptions.builder()
            options.title(`Child #${id}`).size(480, 320).parentWindow(handle)
            const child = await app.openWindow(document, options)
            mountChild(app, child, id)
            children.value = [...children.value, child]

            child.addEventListener('closed', () => {
                children.value = children.value.filter((w) => w !== child)
            })
        }

        async function pickFileWithParent() {
            const handle = mainWindow.windowHandle()
            const result = await pickFile(null, handle)
            pickedFile.value = result
        }

        function closeChild(child: BlitzWindow) {
            void child.close()
        }

        function flashCloseBlocked() {
            closeBlocked.value = true
            setTimeout(() => (closeBlocked.value = false), 1500)
        }

        // Surface a banner when something tries to close the main
        // window while children are open. The actual prevention lives
        // on the main window's `close` listener in `bootstrap.ts`,
        // but it calls back here so we can react in the UI.
        ;(globalThis as any).__notifyMainCloseBlocked = flashCloseBlocked

        const subtitle = computed(() => {
            const n = children.value.length
            if (n === 0) return 'No child windows open. The main window can close.'
            return `${n} child window${n === 1 ? '' : 's'} open. Close them first to close the main window.`
        })

        return () => (
            <div
                style={{
                    height: 'calc(100vh - 2px)',
                    border: '1px solid #888',
                    padding: '16px 24px',
                    fontFamily: 'sans-serif',
                    color: '#222',
                    background: '#fafafa',
                }}>
                <h1 style={{ marginTop: '0', fontSize: '28px' }}>napi-blitz · multi-window</h1>
                <p style={{ marginTop: '4px', color: '#555' }}>{subtitle.value}</p>

                {closeBlocked.value
                    ? (
                        <div
                            style={{
                                marginTop: '8px',
                                padding: '8px 12px',
                                background: '#ffe9b6',
                                border: '1px solid #d49d2d',
                                borderRadius: '6px',
                                color: '#7a5012',
                            }}>
                            Close blocked — child windows are still open.
                        </div>
                    )
                    : null}

                <div style={{ display: 'flex', gap: '12px', marginTop: '16px' }}>
                    <button
                        style={{
                            padding: '10px 18px',
                            fontSize: '16px',
                            background: '#3a78f0',
                            color: '#fff',
                            border: '0',
                            borderRadius: '6px',
                        }}
                        onClick={spawn}>
                        Spawn child window
                    </button>
                    <button
                        style={{
                            padding: '10px 18px',
                            fontSize: '16px',
                            background: '#27ae60',
                            color: '#fff',
                            border: '0',
                            borderRadius: '6px',
                        }}
                        onClick={spawnChild}>
                        Spawn child window (parented)
                    </button>
                    <button
                        style={{
                            padding: '10px 18px',
                            fontSize: '16px',
                            background: '#e67e22',
                            color: '#fff',
                            border: '0',
                            borderRadius: '6px',
                        }}
                        onClick={pickFileWithParent}>
                        Pick file (parented)
                    </button>
                </div>

                {pickedFile.value
                    ? <p style={{ marginTop: '12px', color: '#555', fontSize: '14px' }}>Picked: {pickedFile.value}</p>
                    : null}

                <h2 style={{ fontSize: '18px', marginTop: '24px', marginBottom: '8px' }}>
                    Open child windows
                </h2>
                <ul style={{ paddingLeft: '0', listStyle: 'none' }}>
                    {children.value.length === 0
                        ? <li style={{ color: '#888' }}>(none)</li>
                        : children.value.map((child, i) => (
                            <li
                                key={i}
                                style={{
                                    padding: '8px 12px',
                                    margin: '4px 0',
                                    background: '#fff',
                                    border: '1px solid #ddd',
                                    borderRadius: '6px',
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                }}>
                                <span>{child.document.title || '(untitled)'}</span>
                                <button
                                    style={{
                                        padding: '4px 10px',
                                        background: '#e74c3c',
                                        color: '#fff',
                                        border: '0',
                                        borderRadius: '4px',
                                    }}
                                    onClick={() => closeChild(child)}>
                                    Close
                                </button>
                            </li>
                        ))}
                </ul>
            </div>
        )
    },
})
