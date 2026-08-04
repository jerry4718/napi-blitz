import { computed, defineComponent, ref, watch } from 'vue'
import { ensureCss } from '../../utils/useCss.ts'
import { useDragUpdate } from '../../utils/useDragUpdate.ts'

const ITEM_HEIGHT = 36
const VISIBLE_COUNT = 5
const CONTAINER_HEIGHT = ITEM_HEIGHT * VISIBLE_COUNT
const CENTER_OFFSET = (Math.floor(VISIBLE_COUNT / 2)) * ITEM_HEIGHT

const injectStyles = ensureCss(`
  .sc-column {
    flex: 1;
    height: ${CONTAINER_HEIGHT}px;
    overflow: hidden;
    position: relative;
    cursor: grab;
  }
  .sc-column--dragging {
    cursor: grabbing;
  }
  .sc-item {
    height: ${ITEM_HEIGHT}px;
    line-height: ${ITEM_HEIGHT}px;
    text-align: center;
    font-size: 14px;
    color: #6b7280;
    transition: color 0.1s;
    user-select: none;
  }
  .sc-item--active {
    color: #4f46e5;
    font-weight: 700;
    font-size: 16px;
  }
  .sc-highlight {
    position: absolute;
    left: 8px;
    right: 8px;
    top: ${CENTER_OFFSET}px;
    height: ${ITEM_HEIGHT}px;
    border-top: 1px solid #e5e7eb;
    border-bottom: 1px solid #e5e7eb;
    pointer-events: none;
  }
  .sc-mask-top {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: ${CENTER_OFFSET}px;
    background: linear-gradient(to bottom, rgba(255,255,255,0.9), rgba(255,255,255,0));
    pointer-events: none;
  }
  .sc-mask-bottom {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: ${CENTER_OFFSET}px;
    background: linear-gradient(to top, rgba(255,255,255,0.9), rgba(255,255,255,0));
    pointer-events: none;
  }
`)

/**
 * A scroll column that supports drag and wheel to select an item.
 * Snaps to the nearest item on release.
 */
export const ScrollColumn = defineComponent({
  props: {
    items: { type: Array as () => Array<string | number>, required: true },
    initialIndex: { type: Number, default: 0 },
  },
  emits: ['change'],
  setup(props, { emit }) {
    injectStyles()

    const offset = ref(-props.initialIndex * ITEM_HEIGHT)
    const dragging = ref(false)
    let dragStartOffset = 0
    let suppressClick = false

    const currentIndex = computed(() => {
      return Math.round(-offset.value / ITEM_HEIGHT)
    })

    watch(currentIndex, (idx) => emit('change', idx))

    function clampOffset(val: number): number {
      const min = -(props.items.length - 1) * ITEM_HEIGHT
      const max = 0
      return Math.max(min, Math.min(max, val))
    }

    const onMouseDown = useDragUpdate((current, drag) => {
      const delta = current.clientY - drag.start.clientY
      if (!drag.end) {
        if (!dragging.value) {
          if (Math.abs(delta) < 3) return
          dragging.value = true
        }
        offset.value = clampOffset(dragStartOffset + delta)
      } else {
        if (!dragging.value) return
        dragging.value = false
        suppressClick = true
        setTimeout(() => { suppressClick = false }, 0)
        const idx = Math.round(-offset.value / ITEM_HEIGHT)
        offset.value = -idx * ITEM_HEIGHT
      }
    })

    function onDragStart(e: MouseEvent) {
      dragStartOffset = offset.value
      onMouseDown(e)
    }

    function onWheel(e: WheelEvent) {
      e.preventDefault()
      e.stopPropagation();
      const idx = Math.round(-offset.value / ITEM_HEIGHT)
      const direction = e.deltaY > 0 ? -1 : 1
      const newIdx = Math.max(0, Math.min(props.items.length - 1, idx + direction))
      offset.value = -newIdx * ITEM_HEIGHT
    }

    function onItemClick(idx: number) {
      if (suppressClick) return
      const clampedIdx = Math.max(0, Math.min(props.items.length - 1, idx))
      offset.value = -clampedIdx * ITEM_HEIGHT
    }

    return () => (
      <div
        class={['sc-column', dragging.value ? 'sc-column--dragging' : '']}
        onMousedown={onDragStart}
        onWheel={onWheel}
      >
        <div
          class="sc-list"
          style={{ transform: `translateY(${CENTER_OFFSET + offset.value}px)` }}
        >
          {props.items.map((item, i) => (
            <div
              class={['sc-item', currentIndex.value === i ? 'sc-item--active' : '']}
              onClick={() => onItemClick(i)}
            >
              {item}
            </div>
          ))}
        </div>
        <div class="sc-highlight" />
        <div class="sc-mask-top" />
        <div class="sc-mask-bottom" />
      </div>
    )
  },
})
