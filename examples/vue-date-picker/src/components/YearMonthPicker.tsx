import { defineComponent } from 'vue'
import { Popup } from './Popup.tsx'
import { YearMonthPanel } from './panels/YearMonthPanel.tsx'

export const YearMonthPicker = defineComponent({
  props: {
    year: { type: Number, required: true },
    month: { type: Number, required: true },
    visible: { type: Boolean, default: false },
    x: { type: Number, default: 0 },
    y: { type: Number, default: 0 },
  },
  emits: ['confirm', 'cancel'],
  setup(props, { emit }) {
    function onConfirm(val: { year: number; month: number }) {
      emit('confirm', val)
    }

    function onCancel() {
      emit('cancel')
    }

    return () => (
      <Popup
        visible={props.visible}
        x={props.x}
        y={props.y}
        contentWidth={220}
        contentHeight={224}
        onClose={onCancel}
      >
        {{
          default: () => (
            <YearMonthPanel
              year={props.year}
              month={props.month}
              onConfirm={onConfirm}
              onCancel={onCancel}
            />
          ),
        }}
      </Popup>
    )
  },
})
