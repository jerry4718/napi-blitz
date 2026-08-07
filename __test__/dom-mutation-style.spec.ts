import test from 'ava'
import { BlitzApp, HTMLDocument, WindowOptions } from '../dist/index.js'

// This test calls pumpAppEvents which triggers vello rendering.
// CI containers lack GPU support (vello shaders need float16 capabilities),
// so skip it in CI. Set CI=true in GitHub Actions by default.
const testFn = process.env.CI ? test.skip : test

testFn('JS-created element subtrees can match ancestor descendant selectors without panicking', (t) => {
  const app = BlitzApp.create()
  const document = HTMLDocument.create({
    baseHtml:
      '<!doctype html><html><head><title>x</title><style>.page-header h1 { margin: 0 0 8px; }</style></head><body></body></html>',
  })
  const options = WindowOptions.builder()
  options.size(320, 240)
  const window = app.openWindow(document, options)

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
