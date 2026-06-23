/**
 * ControlPanel — UI overlay for adjusting sphere subdivision level
 * and reading the live face count.
 */
import { defineComponent, PropType } from 'vue'

export const ControlPanel = defineComponent({
  props: {
    level: { type: Number, required: true },
    faceCount: { type: Number, required: true },
    onLevelChange: { type: Function as PropType<(v: number) => void>, required: true },
  },
  setup(props) {
    const LEV_MIN = 0
    const LEV_MAX = 8

    function clampAndSet(v: number) {
      const clamped = Math.max(LEV_MIN, Math.min(LEV_MAX, v))
      props.onLevelChange(clamped)
    }

    return () => (
      <div
        style={{
          position: 'absolute',
          top: '20px',
          right: '20px',
          background: 'rgba(0,0,0,0.75)',
          color: '#e0e0e0',
          padding: '16px 22px',
          borderRadius: '12px',
          fontFamily: 'monospace',
          fontSize: '14px',
          lineHeight: '1.6',
          minWidth: '220px',
          userSelect: 'none',
          zIndex: 10,
        }}>
        <div style={{ fontSize: '16px', fontWeight: 'bold', marginBottom: '8px' }}>
          Sphere Controls
        </div>

        {/* Level slider */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '6px' }}>
          <span>Level:</span>
          <input
            type="range"
            min={LEV_MIN}
            max={LEV_MAX}
            value={props.level}
            onInput={(e: Event) => clampAndSet(Number((e.target as HTMLInputElement).value))}
            style={{ flex: 1 }}
          />
          <span style={{ minWidth: '22px', textAlign: 'right' }}>{props.level}</span>
        </div>

        {/* +/- buttons */}
        <div style={{ display: 'flex', gap: '8px', marginBottom: '10px' }}>
          <button
            class={"btn-minus"}
            onClick={() => clampAndSet(props.level - 1)}
            style={btnStyle}
            disabled={props.level <= LEV_MIN}>
            −
          </button>
          <button
            class={"btn-plus"}
            onClick={() => clampAndSet(props.level + 1)}
            style={btnStyle}
            disabled={props.level >= LEV_MAX}>
            +
          </button>
        </div>

        {/* Stats */}
        <div>
          Faces: <strong>{props.faceCount}</strong>
        </div>
        <div style={{ color: '#999', fontSize: '12px', marginTop: '4px' }}>
          Octa → 8×4<sup>N</sup> faces
        </div>
      </div>
    )
  },
})

const btnStyle: Record<string, string> = {
  flex: 1,
  padding: '6px 0',
  fontSize: '18px',
  border: '1px solid #555',
  borderRadius: '6px',
  background: '#2a2a3a',
  color: '#ccc',
  cursor: 'pointer',
}
