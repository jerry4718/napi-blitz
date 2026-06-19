// Vue provide/inject keys for the per-window context. Components inside
// a window's tree can call:
//
//   const document = inject(BlitzDocumentKey)!
//   const window   = inject(BlitzWindowKey)!
//   const app      = inject(BlitzAppKey)!
//
// to talk to the host. Every window's Vue app installs these in
// `bootstrap` before mounting.

import type { InjectionKey } from 'vue'
import type { BlitzApp, HTMLDocument, Window } from '@ylcc/napi-blitz'

export const BlitzDocumentKey: InjectionKey<HTMLDocument> = Symbol('BlitzDocument')
export const BlitzWindowKey: InjectionKey<Window> = Symbol('BlitzWindow')
export const BlitzAppKey: InjectionKey<BlitzApp> = Symbol('BlitzApp')

/**
 * Reactive registry of "child windows currently open". The main
 * window's close-guard listens to this; the spawn button updates it.
 *
 * Kept in a separate injection so child windows can inject only what
 * they need (e.g. they don't need to mutate the registry — just
 * dismiss themselves).
 */
import type { Ref } from 'vue'
export const ChildWindowsKey: InjectionKey<Ref<Window[]>> = Symbol('ChildWindows')
