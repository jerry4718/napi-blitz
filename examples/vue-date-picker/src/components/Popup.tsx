import {defineComponent, Teleport, computed} from 'vue'
import {ensureCss} from '../utils/useCss.ts'

const injectStyles = ensureCss(`
  .popup-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    z-index: var(--popup-z);
    background: rgba(0,0,0,0.2)
  }
  .popup-content {
    position: absolute;
    z-index: calc(var(--popup-z) + 1);
    left: max(0px, min(var(--popup-x), calc(100vw - var(--popup-w))));
    top: max(0px, min(var(--popup-y), calc(100vh - var(--popup-h))));
  }
  .popup-hidden {
    display: none;
  }
`)

let zCounter = 1000

export const Popup = defineComponent({
  props: {
    visible: {type: Boolean, default: false},
    x: {type: Number, default: 0},
    y: {type: Number, default: 0},
    /** Content width in px, used for viewport clamping. */
    contentWidth: {type: Number, default: 260},
    /** Content height in px, used for viewport clamping. */
    contentHeight: {type: Number, default: 320},
    destroyOnClose: {type: Boolean, default: true},
  },
  emits: ['close'],
  setup(props, {slots, emit}) {
    injectStyles()

    const z = zCounter
    zCounter += 2

    const style = computed(() => ({
      '--popup-x': `${props.x}px`,
      '--popup-y': `${props.y}px`,
      '--popup-w': `${props.contentWidth}px`,
      '--popup-h': `${props.contentHeight}px`,
      '--popup-z': `${z}`,
    } as Record<string, string>))

    function inner() {
      if (props.destroyOnClose && !props.visible) return (<></>)

      const hidden = !props.visible

      return (
        <>
          <div
            class={['popup-overlay', hidden ? 'popup-hidden' : '']}
            style={{'--popup-z': `${z}`}}
            onClick={() => emit('close')}
          />
          <div
            class={['popup-content', hidden ? 'popup-hidden' : '']}
            style={style.value}
          >
            {slots.default?.()}
          </div>
        </>
      )
    }

    return () => (<Teleport to="body">{inner()}</Teleport>)
  },
})
