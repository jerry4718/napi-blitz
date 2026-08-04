import { inject, type InjectionKey } from 'vue'
import type { HTMLDocument } from '@ylcc/napi-blitz'

export const DOCUMENT_KEY: InjectionKey<HTMLDocument> = Symbol('document')

export function useDocument(): HTMLDocument {
  const doc = inject(DOCUMENT_KEY)
  if (!doc) {
    throw new Error('useDocument: document not provided. Call app.provide(DOCUMENT_KEY, document) in bootstrap().')
  }
  return doc
}
