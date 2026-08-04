import { useDocument } from './useDocument.ts'

/** Track which CSS strings have already been injected. */
const injected = new Set<string>()

/**
 * Declare a CSS snippet at module level, inject it later in setup().
 *
 * @example
 * const dpStyles = ensureCss(`
 *   .dp-root { display: flex; }
 * `)
 *
 * // inside setup():
 * dpStyles()
 */
export function ensureCss(css: string): () => void {
  return () => {
    const doc = useDocument()
    if (injected.has(css)) return

    const style = doc.createElement('style')
    style.textContent = css
    doc.head!.appendChild(style)
    injected.add(css)
  }
}
