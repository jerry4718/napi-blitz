import { computed, defineComponent, ref } from 'vue'
import { Sphere3D } from './components/Sphere3D.tsx'
import { ControlPanel } from './components/ControlPanel.tsx'
import { generateIcosaSphere, type TriangleFace } from './utils/sphere.ts'

export const App = defineComponent({
  setup() {
    const level = ref(1)

    const faces = computed<TriangleFace[]>(() => generateIcosaSphere(level.value))

    return () => (
      <div
        style={{
          width: '100vw',
          height: '100vh',
          background: 'radial-gradient(ellipse at center, #1a1a2e 0%, #0a0a14 100%)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          overflow: 'hidden',
          position: 'relative',
        }}>
        <Sphere3D faces={faces.value} level={level.value} />
        <ControlPanel
          level={level.value}
          faceCount={faces.value.length}
          onLevelChange={(v: number) => (level.value = v)}
        />
      </div>
    )
  },
})
