import { defineComponent, reactive, ref, toRefs } from 'vue'

function randomColor() {
  return `#${Math.floor(Math.random() * 0xffffff).toString(16).padStart(6, '0')}`
}

export const Hello = defineComponent({
  props: {
    text: { type: String },
  },
  setup(props, ctx) {
    const hello = ref('hellp')

    const state = reactive({
      color: randomColor(),
      backgroundColor: randomColor(),
    })

    const { color, backgroundColor } = toRefs(state)

    function setRandomColor() {
      state.color = randomColor()
      state.backgroundColor = randomColor()
    }

    return () => {
      return <div
        class={'hello-container'}
        style={{
          color: color.value,
          backgroundColor: backgroundColor.value, display: 'inline-block',
          width: '100px',
          height: '100px',
        }}
        onClick={setRandomColor}
      >
        {ctx.slots.default?.()}
        {hello.value}{'=>'}{props.text},<br />
        {ctx.slots.someSlot?.()}
      </div>
    }
  },
})