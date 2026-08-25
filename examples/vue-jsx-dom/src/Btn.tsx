import { defineComponent, StyleValue } from 'vue'

const defaultStyle: StyleValue = {
  padding: '7px 21px',
  fontSize: '25px',
  background: '#eee',
  color: '#111',
  borderRadius: '8px',
}

export const Btn = defineComponent({
  emits: ['click'],
  setup(_, ctx) {
    return () => (
      <div
        style={defaultStyle}
        onClick={() => ctx.emit('click')}>
        {ctx.slots.default?.()}
      </div>
    )
  },
})