import { defineComponent, ref } from 'vue'
import { Hello } from './Hello.tsx'

export const App = defineComponent({
  setup() {
    const hello = ref('Hello World!')
    return () => {
      return <div
        class="app-container"
        style={{ width: '100%', background: '#66ccff', color: '#443322', display: 'block' }}
        onClick={() => {
          hello.value = 'Hello Blitz!'
        }}
      >
        <Hello text={'Tom'}>{{
          default: () => 'Default Slot',
          someSlot: () => 'what\'s your name',
        }}</Hello>
        {hello.value}
      </div>
    }
  },
})