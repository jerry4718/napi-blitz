/**
 * Sphere3D — renders a sphere made of <div> "face cards" projected in 3D
 * using CSS perspective + transform.  Each face is a flat triangle whose
 * vertices are translated into CSS `matrix3d()` transforms on a common
 * square <div> that gets clipped to the triangle shape via `clip-path`.
 *
 * Rotation is driven by a requestAnimationFrame loop, not CSS animation,
 * so we can swap the full face set on-the-fly when the level changes.
 */
import {computed, defineComponent, onMounted, onUnmounted, PropType, ref} from 'vue'
import { TriangleFace } from './sphere.ts'
import {HTMLElement} from "@ylcc/napi-blitz";
import { useWindow } from './useWindow.ts';

const SPHERE_RADIUS_PX = 240
const PERSPECTIVE = 800

export const Sphere3D = defineComponent({
  props: {
    faces: { type: Array as PropType<TriangleFace[]>, required: true },
    level: { type: Number, required: true },
  },
  setup(props) {
    const containerRef = ref<HTMLElement | null>(null)
    const angleX = ref(0)
    const angleY = ref(0)

    // ---- rotation loop, driven by the window's redraw (rAF) ----
    const win = useWindow()
    let frameId: number | null = null
    let lastTime = 0

    const loop = () => {
      const now = performance.now()
      const dt = lastTime ? Math.min((now - lastTime) / 1000, 0.1) : 0.016
      lastTime = now
      // Rotate ~30° per second around Y, ~18° around X.
      angleX.value = (angleX.value + dt * 0.3) % (Math.PI * 2)
      angleY.value = (angleY.value + dt * 0.5) % (Math.PI * 2)
      frameId = win.requestAnimationFrame(loop)
    }

    onMounted(() => {
      lastTime = performance.now()
      frameId = win.requestAnimationFrame(loop)
    })
    onUnmounted(() => {
      if (frameId !== null) win.cancelAnimationFrame(frameId)
    })

    // ---- Build the CSS for each face ----
    interface FaceCss {
      key: string
      style: Record<string, string>
    }

    const faceCssList = computed<FaceCss[]>(() => {
      return props.faces.map((tri, i) => {
        // Rotate vertices by current angles.
        const [a, b, c] = tri.map((v) => rotateX(rotateY(v, angleY.value), angleX.value))

        // Compute face normal for back-face culling.
        const ux = b.x - a.x
        const uy = b.y - a.y
        const uz = b.z - a.z
        const vx = c.x - a.x
        const vy = c.y - a.y
        const vz = c.z - a.z
        const nx = uy * vz - uz * vy
        const ny = uz * vx - ux * vz
        const nz = ux * vy - uy * vx
        // Dot with view direction (0,0,1): face is visible when nz > 0.
        const visible = nz > 0

        // Project vertices into screen space.
        const pa = project(a)
        const pb = project(b)
        const pc = project(c)

        // Build clip-path polygon from projected coords.
        const clip = `polygon(${pa.x}px ${pa.y}px, ${pb.x}px ${pb.y}px, ${pc.x}px ${pc.y}px)`

        // Lighting: dot(normal, light_dir). Light from top-right-front.
        const lx = 0.4
        const ly = -0.6
        const lz = 0.6
        const nLen = Math.sqrt(nx * nx + ny * ny + nz * nz) || 1
        let light = (nx * lx + ny * ly + nz * lz) / nLen
        light = Math.max(0.2, light * 0.6 + 0.5) // ambient + diffuse

        // Colour based on face index and light intensity.
        const hue = (i * 37 + props.level * 60) % 360
        const r = Math.round(180 + 75 * light * Math.cos(((hue + 0) * Math.PI) / 180))
        const g = Math.round(180 + 75 * light * Math.cos(((hue + 120) * Math.PI) / 180))
        const bCol = Math.round(180 + 75 * light * Math.cos(((hue + 240) * Math.PI) / 180))

        return {
          key: `f${i}`,
          style: {
            position: 'absolute',
            width: `${2 * SPHERE_RADIUS_PX + 10}px`,
            height: `${2 * SPHERE_RADIUS_PX + 10}px`,
            left: '0',
            top: '0',
            background: `rgb(${r},${g},${bCol})`,
            clipPath: clip,
            WebkitClipPath: clip,
            opacity: visible ? '1' : '0.15',
          },
        }
      })
    })

    return () => (
      <div
        ref={containerRef}
        style={{
          perspective: `${PERSPECTIVE}px`,
          perspectiveOrigin: '50% 50%',
          width: `${2 * SPHERE_RADIUS_PX + 10}px`,
          height: `${2 * SPHERE_RADIUS_PX + 10}px`,
          position: 'relative',
        }}>
        {faceCssList.value.map((f, fdx) => (
          <div class={`face=${fdx}`} key={f.key} style={f.style} />
        ))}
      </div>
    )
  },
})

// ---------- 3D helpers ----------

interface Vec3 {
  x: number
  y: number
  z: number
}

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

/** Perspective projection: z is distance from camera. */
function project(v: Vec3): { x: number; y: number } {
  const scale = PERSPECTIVE / (PERSPECTIVE + v.z * SPHERE_RADIUS_PX + SPHERE_RADIUS_PX)
  return {
    x: SPHERE_RADIUS_PX + v.x * SPHERE_RADIUS_PX * scale,
    y: SPHERE_RADIUS_PX - v.y * SPHERE_RADIUS_PX * scale,
  }
}
