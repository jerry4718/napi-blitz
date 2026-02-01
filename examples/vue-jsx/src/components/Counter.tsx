import { defineComponent, ref } from 'vue'
import { Btn } from './Btn.tsx'
import { randomColor } from '../utils/color.ts'

export const Counter = defineComponent(() => {
  const count = ref(0)

  const color = ref(randomColor())

  return () => (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-around',
        height: '65vh',
      }}>
      <div
        style={{
          padding: '5px 15px',
          fontSize: '30px',
          lineHeight: '50px',
          textAlign: 'center',
          background: '#000',
          color: color.value.toRgbString(),
        }}
        onMousemove={() => color.value = randomColor()}>
        Count: {count.value}
      </div>
      <div style={{ display: 'flex', justifyContent: "center", width: '100%', gap: '10%', flexWrap: "wrap" }}>
        <Btn
          style={{ background: '#a00', color: '#fff' }}
          onClick={() => count.value++}>
          {{ default: () => 'Increment' }}
        </Btn>
        <Btn
          style={{ background: '#0a0', color: '#fff' }}
          onClick={() => count.value--}>
          {{ default: () => 'Decrement' }}
        </Btn>
      </div>
      <Btn
        style={{ background: '#aa0', color: '#fff' }}
        onClick={() => count.value = 0}>
        {{ default: () => 'Reset' }}
      </Btn>
    </div>
  )
})