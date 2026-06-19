import {
  BlitzApp,
  HTMLElement as BlitzHTMLElement,
  HTMLDocument,
  Node as BlitzNode,
  type OpenWindowInit,
} from '@ylcc/napi-blitz'
import process from 'node:process'

const BASE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>Blitz HTML Tag Matrix</title>
<style>
  html, body { margin: 0; padding: 0; }
</style>
</head>
<body></body>
</html>`

interface TagCase {
  readonly tag: string
  readonly note?: string
  readonly build?: (el: BlitzHTMLElement, document: HTMLDocument) => void
}

interface TagSection {
  readonly title: string
  readonly source: string
  readonly cases: readonly TagCase[]
}

// Keep this list aligned with Blitz's upstream UA stylesheet sections in
// packages/blitz-dom/assets/default.css, plus the special layout branches for
// input/textarea/svg in packages/blitz-dom/src/layout/construct.rs.
const SECTIONS: readonly TagSection[] = [
  {
    title: 'Sectioning and block roots',
    source: 'default.css: blocks',
    cases: tags(
      'article',
      'aside',
      'details',
      'div',
      'footer',
      'form',
      'header',
      'hgroup',
      'main',
      'nav',
      'search',
      'section',
      'summary',
      'p',
      'blockquote',
      'figure',
      'figcaption',
      'address',
      'center',
    ),
  },
  {
    title: 'Headings and preformatted blocks',
    source: 'default.css: h1-h6, pre/listing/xmp/plaintext',
    cases: tags('h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'listing', 'xmp', 'plaintext'),
  },
  {
    title: 'Inline phrasing',
    source: 'default.css: inlines',
    cases: tags(
      'a',
      'abbr',
      'acronym',
      'b',
      'bdi',
      'bdo',
      'big',
      'br',
      'cite',
      'code',
      'data',
      'del',
      'dfn',
      'em',
      'i',
      'ins',
      'kbd',
      'mark',
      'nobr',
      'q',
      's',
      'samp',
      'small',
      'span',
      'strike',
      'strong',
      'sub',
      'sup',
      'time',
      'tt',
      'u',
      'var',
      'wbr',
    ),
  },
  {
    title: 'Lists',
    source: 'default.css: lists',
    cases: [
      { tag: 'ul', build: (el, document) => appendListItems(el, document, ['disc one', 'disc two']) },
      { tag: 'ol', build: (el, document) => appendListItems(el, document, ['decimal one', 'decimal two']) },
      { tag: 'menu', build: (el, document) => appendListItems(el, document, ['menu one', 'menu two']) },
      { tag: 'dir', build: (el, document) => appendListItems(el, document, ['dir one', 'dir two']) },
      { tag: 'dl', build: (el, document) => appendDefinitionList(el, document) },
      { tag: 'li', note: 'standalone list item' },
      { tag: 'dt', note: 'definition term' },
      { tag: 'dd', note: 'definition description' },
    ],
  },
  {
    title: 'Tables',
    source: 'default.css: tables',
    cases: [
      { tag: 'table', build: buildTable },
      { tag: 'caption', note: 'shown inside table in the table case' },
      { tag: 'colgroup', note: 'shown inside table in the table case' },
      { tag: 'col', note: 'shown inside table in the table case' },
      { tag: 'thead', note: 'shown inside table in the table case' },
      { tag: 'tbody', note: 'shown inside table in the table case' },
      { tag: 'tfoot', note: 'shown inside table in the table case' },
      { tag: 'tr', note: 'shown inside table in the table case' },
      { tag: 'th', note: 'shown inside table in the table case' },
      { tag: 'td', note: 'shown inside table in the table case' },
    ],
  },
  {
    title: 'Forms and interactive controls',
    source: 'construct.rs: input/textarea special layout, default form controls',
    cases: [
      inputCase('text'),
      inputCase('password'),
      inputCase('email'),
      inputCase('number'),
      inputCase('search'),
      inputCase('tel'),
      inputCase('url'),
      inputCase('checkbox'),
      inputCase('radio'),
      inputCase('button'),
      inputCase('submit'),
      { tag: 'textarea', build: (el) => (el.textContent = 'textarea text') },
      { tag: 'button', build: (el) => (el.textContent = 'button text') },
      { tag: 'label', build: (el) => (el.textContent = 'label text') },
      { tag: 'select', build: appendOptions },
      { tag: 'option', note: 'standalone option' },
      { tag: 'optgroup', build: appendOptions },
      { tag: 'fieldset', build: appendLegend },
      { tag: 'legend', note: 'standalone legend' },
      { tag: 'output', note: 'output text' },
      { tag: 'progress', build: (el) => el.setAttribute('value', '0.6') },
      { tag: 'meter', build: (el) => el.setAttribute('value', '0.6') },
    ],
  },
  {
    title: 'Media, replaced, and embedded',
    source: 'default.css: leafs/media, construct.rs: svg special branch',
    cases: [
      { tag: 'img', build: (el) => el.setAttribute('alt', 'broken image alt text') },
      { tag: 'canvas', note: 'canvas element' },
      { tag: 'svg', build: buildSvg },
      { tag: 'picture', build: appendImageFallback },
      { tag: 'video', build: (el) => el.setAttribute('controls', '') },
      { tag: 'audio', build: (el) => el.setAttribute('controls', '') },
      { tag: 'iframe', note: 'iframe border only' },
      { tag: 'object', note: 'object element' },
      { tag: 'embed', note: 'embed element' },
      { tag: 'map', build: appendArea },
      { tag: 'area', note: 'display:none in UA sheet' },
      { tag: 'source', note: 'media child tag' },
      { tag: 'track', note: 'media child tag' },
      { tag: 'param', note: 'display:none in UA sheet' },
      { tag: 'hr', note: 'horizontal rule' },
    ],
  },
  {
    title: 'Details, dialog, ruby, slots, legacy',
    source: 'default.css: details/dialog/ruby/marquee/slot',
    cases: [
      { tag: 'details', build: appendDetails },
      { tag: 'dialog', build: (el) => el.setAttribute('open', '') },
      { tag: 'marquee', note: 'legacy marquee box' },
      { tag: 'ruby', build: appendRuby },
      { tag: 'rb', note: 'ruby base' },
      { tag: 'rt', note: 'ruby text' },
      { tag: 'rtc', note: 'ruby text container' },
      { tag: 'rp', note: 'display:none fallback parentheses' },
      { tag: 'slot', note: 'display:contents' },
      { tag: 'spacer', note: 'legacy spacer' },
      { tag: 'frame', note: 'legacy frame' },
      { tag: 'frameset', note: 'legacy frameset' },
    ],
  },
  {
    title: 'Metadata and hidden-by-UA elements',
    source: 'default.css: hidden elements',
    cases: tags(
      'base',
      'basefont',
      'datalist',
      'head',
      'link',
      'meta',
      'noembed',
      'noframes',
      'noscript',
      'script',
      'style',
      'template',
      'title',
    ),
  },
]

export async function bootstrap(): Promise<void> {
  const app = BlitzApp.create()
  const windowInit: OpenWindowInit = {
    baseHtml: BASE_HTML,
    title: 'Blitz HTML Tag Matrix',
    width: 1200,
    height: 900,
  }
  const window = app.openWindow(windowInit)
  const document = window.document

  installStyles(document)
  renderTagMatrix(document)

  document.documentElement.addEventListener('keydown', () => {
    console.log(document.body?.outerHTML ?? '<no body>')
  })

  await pump(app)
}

function renderTagMatrix(document: HTMLDocument): void {
  const body = document.body!
  body.appendChild(createHeader(document))

  const main = document.createElement('main') as BlitzHTMLElement
  main.setAttribute('class', 'matrix')
  body.appendChild(main)

  for (const section of SECTIONS) {
    const sectionEl = document.createElement('section') as BlitzHTMLElement
    sectionEl.setAttribute('class', 'tag-section')

    const heading = document.createElement('h2') as BlitzHTMLElement
    heading.textContent = section.title
    sectionEl.appendChild(heading)

    const source = document.createElement('p') as BlitzHTMLElement
    source.setAttribute('class', 'source')
    source.textContent = section.source
    sectionEl.appendChild(source)

    const grid = document.createElement('div') as BlitzHTMLElement
    grid.setAttribute('class', 'tag-grid')
    sectionEl.appendChild(grid)

    for (const tagCase of section.cases) {
      grid.appendChild(createTagCard(tagCase, document))
    }

    main.appendChild(sectionEl)
  }
}

function createHeader(document: HTMLDocument): BlitzHTMLElement {
  const header = document.createElement('header') as BlitzHTMLElement
  header.setAttribute('class', 'page-header')

  const title = document.createElement('h1') as BlitzHTMLElement
  title.textContent = 'Blitz HTML Tag Matrix'
  header.appendChild(title)

  const summary = document.createElement('p') as BlitzHTMLElement
  summary.textContent =
    'Manual visual smoke test for tags listed in Blitz\'s UA stylesheet and special layout branches. Press any key to dump body.outerHTML.'
  header.appendChild(summary)

  return header
}

function createTagCard(tagCase: TagCase, document: HTMLDocument): BlitzHTMLElement {
  const card = document.createElement('article') as BlitzHTMLElement
  card.setAttribute('class', 'tag-card')
  card.setAttribute('data-tag', tagCase.tag)

  const label = document.createElement('div') as BlitzHTMLElement
  label.setAttribute('class', 'tag-label')
  label.textContent = `<${tagCase.tag}>`
  card.appendChild(label)

  const sampleWrap = document.createElement('div') as BlitzHTMLElement
  sampleWrap.setAttribute('class', 'tag-sample')
  card.appendChild(sampleWrap)

  const sample = document.createElement(tagCase.tag) as BlitzHTMLElement
  sample.setAttribute('data-created-tag', tagCase.tag)
  sample.textContent = tagCase.note ?? `sample ${tagCase.tag}`
  tagCase.build?.(sample, document)
  sampleWrap.appendChild(sample)

  const status = document.createElement('code') as BlitzHTMLElement
  status.setAttribute('class', 'tag-status')
  status.textContent = sample.tagName.toLowerCase() === tagCase.tag ? 'created' : `created as ${sample.tagName}`
  card.appendChild(status)

  return card
}

function tags(...names: readonly string[]): readonly TagCase[] {
  return names.map((tag) => ({ tag }))
}

function inputCase(type: string): TagCase {
  return {
    tag: 'input',
    note: `input type=${type}`,
    build: (el) => {
      el.setAttribute('type', type)
      el.setAttribute('value', type === 'password' ? 'secret' : `value ${type}`)
      if (type === 'checkbox' || type === 'radio') {
        el.setAttribute('checked', '')
      }
      if (type === 'radio') {
        el.setAttribute('name', 'tag-matrix-radio')
      }
    },
  }
}

function appendListItems(parent: BlitzHTMLElement, document: HTMLDocument, items: readonly string[]): void {
  parent.textContent = ''
  for (const text of items) {
    const li = document.createElement('li') as BlitzHTMLElement
    li.textContent = text
    parent.appendChild(li)
  }
}

function appendDefinitionList(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  const dt = document.createElement('dt') as BlitzHTMLElement
  dt.textContent = 'term'
  const dd = document.createElement('dd') as BlitzHTMLElement
  dd.textContent = 'description'
  parent.appendChild(dt)
  parent.appendChild(dd)
}

function buildTable(table: BlitzHTMLElement, document: HTMLDocument): void {
  table.textContent = ''
  table.setAttribute('border', '1')

  const caption = document.createElement('caption') as BlitzHTMLElement
  caption.textContent = 'caption'
  table.appendChild(caption)

  const colgroup = document.createElement('colgroup') as BlitzHTMLElement
  colgroup.appendChild(document.createElement('col'))
  colgroup.appendChild(document.createElement('col'))
  table.appendChild(colgroup)

  const thead = document.createElement('thead') as BlitzHTMLElement
  const headRow = document.createElement('tr') as BlitzHTMLElement
  for (const text of ['A', 'B']) {
    const th = document.createElement('th') as BlitzHTMLElement
    th.textContent = text
    headRow.appendChild(th)
  }
  thead.appendChild(headRow)
  table.appendChild(thead)

  const tbody = document.createElement('tbody') as BlitzHTMLElement
  const bodyRow = document.createElement('tr') as BlitzHTMLElement
  for (const text of ['one', 'two']) {
    const td = document.createElement('td') as BlitzHTMLElement
    td.textContent = text
    bodyRow.appendChild(td)
  }
  tbody.appendChild(bodyRow)
  table.appendChild(tbody)

  const tfoot = document.createElement('tfoot') as BlitzHTMLElement
  const footRow = document.createElement('tr') as BlitzHTMLElement
  const td = document.createElement('td') as BlitzHTMLElement
  td.setAttribute('colspan', '2')
  td.textContent = 'footer'
  footRow.appendChild(td)
  tfoot.appendChild(footRow)
  table.appendChild(tfoot)
}

function appendOptions(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  const option = document.createElement('option') as BlitzHTMLElement
  option.textContent = 'option text'
  parent.appendChild(option)
}

function appendLegend(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  const legend = document.createElement('legend') as BlitzHTMLElement
  legend.textContent = 'legend'
  parent.appendChild(legend)
  parent.appendChild(document.createTextNode('fieldset content') as BlitzNode)
}

function buildSvg(svg: BlitzHTMLElement, document: HTMLDocument): void {
  svg.textContent = ''
  svg.setAttribute('width', '80')
  svg.setAttribute('height', '40')
  svg.setAttribute('viewBox', '0 0 80 40')
  const rect = document.createElement('rect') as BlitzHTMLElement
  rect.setAttribute('x', '4')
  rect.setAttribute('y', '4')
  rect.setAttribute('width', '72')
  rect.setAttribute('height', '32')
  rect.setAttribute('fill', '#5b8def')
  svg.appendChild(rect)
}

function appendImageFallback(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  const img = document.createElement('img') as BlitzHTMLElement
  img.setAttribute('alt', 'picture fallback')
  parent.appendChild(img)
}

function appendArea(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  const area = document.createElement('area') as BlitzHTMLElement
  area.setAttribute('alt', 'map area')
  parent.appendChild(area)
}

function appendDetails(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  parent.setAttribute('open', '')
  const summary = document.createElement('summary') as BlitzHTMLElement
  summary.textContent = 'summary text'
  parent.appendChild(summary)
  parent.appendChild(document.createTextNode('details content') as BlitzNode)
}

function appendRuby(parent: BlitzHTMLElement, document: HTMLDocument): void {
  parent.textContent = ''
  const rb = document.createElement('rb') as BlitzHTMLElement
  rb.textContent = '漢'
  const rt = document.createElement('rt') as BlitzHTMLElement
  rt.textContent = 'kan'
  parent.appendChild(rb)
  parent.appendChild(rt)
}

function installStyles(document: HTMLDocument): void {
  const style = document.createElement('style') as BlitzHTMLElement
  style.textContent = `
    body {
      background: #f6f7fb;
      color: #1f2937;
      font-family: system-ui, sans-serif;
      line-height: 1.35;
    }
    .page-header {
      background: #111827;
      color: white;
      padding: 20px 24px;
    }
    .page-header h1 { margin: 0 0 8px; }
    .page-header p { margin: 0; color: #d1d5db; }
    .matrix { padding: 16px 24px 32px; }
    .tag-section {
      background: white;
      border: 1px solid #d6dbe6;
      border-radius: 10px;
      margin: 0 0 18px;
      padding: 16px;
    }
    .tag-section h2 { margin: 0 0 4px; }
    .source { color: #64748b; margin: 0 0 12px; }
    .tag-grid {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 10px;
    }
    .tag-card {
      border: 1px solid #e2e8f0;
      border-radius: 8px;
      padding: 10px;
      min-height: 96px;
      background: #fbfdff;
    }
    .tag-label {
      color: #0f766e;
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-weight: 700;
      margin-bottom: 8px;
    }
    .tag-sample {
      border: 1px dashed #cbd5e1;
      border-radius: 6px;
      min-height: 32px;
      padding: 6px;
      overflow: hidden;
      background: white;
    }
    .tag-status {
      display: block;
      margin-top: 8px;
      color: #64748b;
      font-size: 11px;
    }
    table { width: 100%; }
    svg { display: block; }
    input, textarea, button, select { max-width: 100%; }
  `
  document.head!.appendChild(style)
}

async function pump(app: BlitzApp): Promise<void> {
  while (true) {
    const result = app.pumpAppEvents(0)
    if (result.exit) {
      process.exit(result.code ?? 0)
    }
    await new Promise((resolve) => setTimeout(resolve, 16))
  }
}
