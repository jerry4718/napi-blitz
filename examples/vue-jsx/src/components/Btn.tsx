import { computed, defineComponent, PropType, StyleValue } from 'vue'

const defaultStyle: StyleValue = {
  padding: '7px 21px',
  fontSize: '25px',
  background: '#eee',
  color: '#111',
  borderRadius: '8px',
}

export const Btn = defineComponent({
  props: {
    style: { type: Object as PropType<StyleValue>, default: () => ({}) },
  },
  emits: ['click'],
  setup(props, ctx) {
    const mergedStyle = computed(() => Object.assign({}, defaultStyle, props.style))

    return () => <>
      <div
        style={mergedStyle.value}
        onClick={() => ctx.emit('click')}>
        {ctx.slots.default?.()}
      </div>
    </>
  },
})