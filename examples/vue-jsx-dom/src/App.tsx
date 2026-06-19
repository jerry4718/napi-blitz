import { computed, defineComponent, ref } from 'vue'
import { randomColor } from './utils/color.ts'
import { Counter } from './components/Counter.tsx'

export const App = defineComponent({
  setup() {
    const colors = ref([randomColor(), randomColor(), randomColor()])

    const background = computed(() => {
      const [color1, color2, color3] = colors.value

      return `linear-gradient(217deg, ${color1.alpha(0.8).toRgbString()}, ${color1.alpha(0.0).toRgbString()} 70.71%),
              linear-gradient(127deg, ${color2.alpha(0.8).toRgbString()}, ${color2.alpha(0.0).toRgbString()} 70.71%),
              linear-gradient(336deg, ${color3.alpha(0.8).toRgbString()}, ${color3.alpha(0.0).toRgbString()} 70.71%)`
    })

    function changeColors() {
      colors.value = [randomColor(), randomColor(), randomColor()]
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
        <h1 style={{ textAlign: 'center', fontSize: '50px' }}>
          Hello Blitz
        </h1>
        <Counter />
      </div>
    }
  },
})