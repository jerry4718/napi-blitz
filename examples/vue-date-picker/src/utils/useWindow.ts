import { inject, type InjectionKey } from 'vue'
import type { Window } from '@ylcc/napi-blitz'

export const WINDOW_KEY: InjectionKey<Window> = Symbol('window')

export function useWindow(): Window {
  const win = inject(WINDOW_KEY)
  if (!win) {
    throw new Error('useWindow: window not provided. Call app.provide(WINDOW_KEY, window) in bootstrap().')
  }
  return win
}
