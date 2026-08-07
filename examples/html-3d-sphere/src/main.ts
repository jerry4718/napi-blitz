import {
  BlitzApp,
  HTMLElement as BlitzHTMLElement,
  HTMLDocument,
  Node as BlitzNode,
  WindowOptions,
} from '@ylcc/napi-blitz'
import process from 'node:process'
import { generateIcosaSphere, type TriangleFace, type Vec3 } from './sphere.ts'

const SPHERE_RADIUS_PX = 240
const PERSPECTIVE = 800
const LEV_MIN = 0
const LEV_MAX = 4

// ---------- 3D helpers ----------

function rotateY(v: Vec3, angle: number): Vec3 {
  const c = Math.cos(angle)
  const s = Math.sin(angle)
  return { x: v.x * c + v.z * s, y: v.y, z: -v.x * s + v.z * c }
}

function rotateX(v: Vec3, angle: number): Vec3 {
  const c = Math.cos(angle)
  const s = Math.sin(angle)
  return { x: v.x, y: v.y * c - v.z * s, z: v.y * s + v.z * c }
}

function project(v: Vec3): { x: number; y: number } {
  const scale = PERSPECTIVE / (PERSPECTIVE + v.z * SPHERE_RADIUS_PX + SPHERE_RADIUS_PX)
  return {
    x: SPHERE_RADIUS_PX + v.x * SPHERE_RADIUS_PX * scale,
    y: SPHERE_RADIUS_PX - v.y * SPHERE_RADIUS_PX * scale,
  }
}

// ---------- State ----------

let level = 0
let angleX = 0
let angleY = 0
let faces: TriangleFace[] = generateIcosaSphere(level)
let faceEls: BlitzHTMLElement[] = []
let globalDoc: HTMLDocument | null = null
let pendingLevelChange: number | null = null

// ---------- Bootstrap ----------

const BASE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<title>3D Sphere (HTML)</title>
<style>
  html, body { border: 0; margin: 0; padding: 0; overflow: hidden; }
</style>
</head>
<body></body>
</html>`

export async function bootstrap() {
  const app = BlitzApp.create()
  const document = HTMLDocument.create({ baseHtml: BASE_HTML })
  const options = WindowOptions.builder()
  options.title('3D Sphere (HTML)')
  app.openWindow(document, options)
  globalDoc = document

  installStyles(document)

  const body = document.body!
  const container = document.createElement('div') as BlitzHTMLElement
  container.setAttribute('id', 'sphere-container')
  body.appendChild(container)

  const panel = buildControlPanel(document)
  body.appendChild(panel)

  document.documentElement.addEventListener('click', (event) => {
    const target = event.target as BlitzHTMLElement | null
    console.log('document click target:', target?.tagName, target?.getAttribute?.('id'), target?.getAttribute?.('class'))
  })

  rebuildFaces(container, document)
  updatePanel(panel)

  // Animation loop
  let lastTime = performance.now()
  const loop = () => {
    const now = performance.now()
    const dt = Math.min((now - lastTime) / 1000, 0.1)
    lastTime = now
    angleX = (angleX + dt * 0.3) % (Math.PI * 2)
    angleY = (angleY + dt * 0.5) % (Math.PI * 2)
    const p0 = performance.now()
    renderFaces()
    console.log("renderFaces:", performance.now() - p0);
  }

  // Pump + animation
  while (true) {
    const result = app.pumpAppEvents(60)
    if (result.exit) process.exit(result.code ?? 0)
    applyLevelChange()
    loop()
    await new Promise((r) => setTimeout(r, 60))
  }
}

// ---------- Face DOM management ----------

function rebuildFaces(
  container: BlitzHTMLElement,
  document: HTMLDocument,
): void {
  // Remove old face divs
  for (const el of faceEls) {
    el.remove()
  }
  faceEls = []

  const p0 = performance.now()
  // Create new face divs
  for (let i = 0; i < faces.length; i++) {
    const div = document.createElement('div') as BlitzHTMLElement
    div.setAttribute('class', 'face')
    container.appendChild(div)
    faceEls.push(div)
  }
  console.log("rebuildFaces:", performance.now() - p0);
}

function renderFaces(): void {
  const size = 2 * SPHERE_RADIUS_PX + 10
  for (let i = 0; i < faces.length; i++) {
    const tri = faces[i]
    const el = faceEls[i]
    if (!el) continue

    const [va, vb, vc] = tri.map((v) =>
      rotateX(rotateY(v, angleY), angleX),
    )

    // Back-face culling
    const ux = vb.x - va.x, uy = vb.y - va.y, uz = vb.z - va.z
    const vx = vc.x - va.x, vy = vc.y - va.y, vz = vc.z - va.z
    const nx = uy * vz - uz * vy
    const ny = uz * vx - ux * vz
    const nz = ux * vy - uy * vx
    const visible = nz > 0

    const pa = project(va), pb = project(vb), pc = project(vc)
    const clip = `polygon(${pa.x}px ${pa.y}px, ${pb.x}px ${pb.y}px, ${pc.x}px ${pc.y}px)`

    // Lighting
    const lx = 0.4, ly = -0.6, lz = 0.6
    const nLen = Math.sqrt(nx * nx + ny * ny + nz * nz) || 1
    let light = (nx * lx + ny * ly + nz * lz) / nLen
    light = Math.max(0.2, light * 0.6 + 0.5)

    const hue = (i * 37 + level * 60) % 360
    const cr = Math.round(180 + 75 * light * Math.cos(((hue) * Math.PI) / 180))
    const cg = Math.round(180 + 75 * light * Math.cos(((hue + 120) * Math.PI) / 180))
    const cb = Math.round(180 + 75 * light * Math.cos(((hue + 240) * Math.PI) / 180))

    const s = el.style
    // s.position = 'absolute'
    s.width = `${size}px`
    s.height = `${size}px`
    // s.left = '0'
    // s.top = '0'
    s.background = `rgb(${cr},${cg},${cb})`
    s.clipPath = clip
    s.opacity = visible ? '1' : '0.15'
    // s.pointerEvents = 'none'
  }
}

// ---------- Control Panel ----------

function buildControlPanel(document: HTMLDocument): BlitzHTMLElement {
  const panel = document.createElement('div') as BlitzHTMLElement
  panel.setAttribute('id', 'control-panel')

  const title = document.createElement('div') as BlitzHTMLElement
  title.setAttribute('class', 'panel-title')
  title.textContent = 'Sphere Controls'
  panel.appendChild(title)

  // Level row
  const row = document.createElement('div') as BlitzHTMLElement
  row.setAttribute('class', 'panel-row')

  const label = document.createElement('span') as BlitzHTMLElement
  label.textContent = 'Level:'
  row.appendChild(label)

  const levelSpan = document.createElement('span') as BlitzHTMLElement
  levelSpan.setAttribute('id', 'level-display')
  levelSpan.textContent = String(level)
  row.appendChild(levelSpan)

  panel.appendChild(row)

  // Buttons
  const btnRow = document.createElement('div') as BlitzHTMLElement
  btnRow.setAttribute('class', 'panel-btn-row')

  const btnMinus = document.createElement('button') as BlitzHTMLElement
  btnMinus.setAttribute('id', 'btn-minus')
  btnMinus.textContent = '−'
  btnMinus.addEventListener('click', () => {
    console.log("btn-minus::click")
    changeLevel(-1)
  })
  btnRow.appendChild(btnMinus)

  const btnPlus = document.createElement('button') as BlitzHTMLElement
  btnPlus.setAttribute('id', 'btn-plus')
  btnPlus.textContent = '+'
  btnPlus.addEventListener('click', () => {
    console.log("btn-plus::click")
    changeLevel(1)
  })
  btnRow.appendChild(btnPlus)

  panel.appendChild(btnRow)

  // Stats
  const stats = document.createElement('div') as BlitzHTMLElement
  stats.setAttribute('id', 'stats')
  panel.appendChild(stats)

  return panel
}

function changeLevel(delta: number): void {
  const newLevel = Math.max(LEV_MIN, Math.min(LEV_MAX, level + delta))
  if (newLevel === level) return
  pendingLevelChange = newLevel
}

function applyLevelChange(): void {
  if (pendingLevelChange === null) return
  level = pendingLevelChange
  pendingLevelChange = null
  faces = generateIcosaSphere(level)

  const doc = globalDoc!
  const container = doc.querySelector('#sphere-container') as BlitzHTMLElement
  if (container) rebuildFaces(container, doc)

  const panel = doc.querySelector('#control-panel') as BlitzHTMLElement
  if (panel) updatePanel(panel)
}

function updatePanel(panel: BlitzHTMLElement): void {
  const levelDisp = panel.querySelector('#level-display') as BlitzHTMLElement | null
  if (levelDisp) levelDisp.textContent = String(level)

  const stats = panel.querySelector('#stats') as BlitzHTMLElement | null
  if (stats) stats.textContent = `Faces: ${faces.length}`
}

function installStyles(document: HTMLDocument): void {
  const style = document.createElement('style') as BlitzHTMLElement
  style.textContent = `
    body {
      width: 100vw;
      height: 100vh;
      background: radial-gradient(ellipse at center, #1a1a2e 0%, #0a0a14 100%);
      display: flex;
      align-items: center;
      justify-content: center;
      overflow: hidden;
      position: relative;
    }
    #sphere-container {
      perspective: ${PERSPECTIVE}px;
      perspective-origin: 50% 50%;
      width: ${2 * SPHERE_RADIUS_PX + 10}px;
      height: ${2 * SPHERE_RADIUS_PX + 10}px;
      position: relative;
    }
    #control-panel {
      position: absolute;
      top: 20px;
      right: 20px;
      background: rgba(0,0,0,0.75);
      color: #e0e0e0;
      padding: 16px 22px;
      border-radius: 12px;
      font-family: monospace;
      font-size: 14px;
      min-width: 220px;
      user-select: none;
      z-index: 10;
    }
    .panel-title { font-size: 16px; font-weight: bold; margin-bottom: 8px; }
    .panel-row { display: flex; align-items: center; gap: 10px; margin-bottom: 6px; }
    .panel-btn-row { display: flex; gap: 8px; margin-bottom: 10px; }
    .panel-btn-row button {
      flex: 1; padding: 6px 0; font-size: 18px;
      border: 1px solid #555; border-radius: 6px;
      background: #2a2a3a; color: #ccc; cursor: pointer;
    }
    #stats { color: #e0e0e0; }
    .face {
      position: absolute;
      left: 0;
      top: 0;
      pointer-events: none;
    }
  `
  document.head!.appendChild(style)
}
