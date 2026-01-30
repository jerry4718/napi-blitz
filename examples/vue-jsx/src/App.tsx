import { computed, defineComponent, ref } from 'vue'
import { random as randomColord } from 'colord'
import { Counter } from './components/Counter.tsx'

export const App = defineComponent({
  setup() {
    const colors = ref([randomColord(), randomColord(), randomColord()])

    const background = computed(() => {
      const [color1, color2, color3] = colors.value

      return `linear-gradient(217deg, ${color1.alpha(0.8).toRgbString()}, ${color1.alpha(0.0).toRgbString()} 70.71%),
              linear-gradient(127deg, ${color2.alpha(0.8).toRgbString()}, ${color2.alpha(0.0).toRgbString()} 70.71%),
              linear-gradient(336deg, ${color3.alpha(0.8).toRgbString()}, ${color3.alpha(0.0).toRgbString()} 70.71%)`
    })

    function changeColors() {
      colors.value = [randomColord(), randomColord(), randomColord()]
    }

    return () => {
      return <div
        class="app-container"
        style={{
          height: 'calc(100vh - 2px)',
          border: '1px solid red',
          overflow: 'scroll',
          background: background.value,
          color: '#443322',
        }}
        onClick={changeColors}
      >
        <div style={{ textAlign: 'center', fontSize: '50px', fontWeight: 'bold' }}>
          Hello Blitz
        </div>
        <Counter />
      </div>
    }
  },
})