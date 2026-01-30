import { defineComponent, reactive, ref, toRefs } from 'vue'

function randomColor() {
  return `#${Math.floor(Math.random() * 0xffffff).toString(16).padStart(6, '0')}`
}

export const Hello = defineComponent({
  props: {
    text: { type: String },
  },
  emits: ['emit'],
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
      ctx.emit('emit')
    }

    return () => {
      return <div
        class={'hello-container'}
        style={{
          width: '100px',
          height: '100px',
          transition: '.25s',
          color: color.value,
          backgroundColor: backgroundColor.value,
        }}
        onClick={setRandomColor}
      >
        <button onClick={() => {}}>{ctx.slots.default?.()}</button>
        <checkbox></checkbox>
        {hello.value}{'=>'}{props.text},<br />
        {ctx.slots.someSlot?.()}
      </div>
    }
  },
})