import test from 'ava'
import { BlitzApp } from '../packages/napi-blitz/dist/index.js'

test('JS-created element subtrees can match ancestor descendant selectors without panicking', (t) => {
  const app = BlitzApp.create()
  const window = app.openWindow({
    baseHtml:
      '<!doctype html><html><head><title>x</title><style>.page-header h1 { margin: 0 0 8px; }</style></head><body></body></html>',
    width: 320,
    height: 240,
  })
  const { document } = window

  const header = document.createElement('header')
  header.setAttribute('class', 'page-header')
  const h1 = document.createElement('h1')
  h1.textContent = 'Title'
  header.appendChild(h1)
  document.body!.appendChild(header)

  const result = app.pumpAppEvents(0)
  t.false(result.exit)
  window.close()
})
