import { defineComponent, ref } from 'vue'
import { Btn } from './Btn.tsx'

export const Counter = defineComponent(() => {
  const count = ref(0)

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
          width: '150px',
          height: '50px',
          fontSize: '30px',
          lineHeight: '50px',
          textAlign: 'center',
          background: '#000',
          color: '#fff',
        }}>
        Count: {count.value}
      </div>
      <div style={{ display: 'flex', justifyContent: 'space-around', width: '50%' }}>
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