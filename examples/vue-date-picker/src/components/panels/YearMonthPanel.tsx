import { defineComponent, ref } from 'vue'
import { ensureCss } from '../../utils/useCss.ts'
import { MONTH_NAMES } from '../../constants.ts'
import { ScrollColumn } from './ScrollColumn.tsx'

const injectStyles = ensureCss(`
  .ym-root {
    width: 220px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.18);
    border: 1px solid #e0e0e0;
    overflow: hidden;
    font-family: system-ui, sans-serif;
  }
  .ym-columns {
    display: flex;
    position: relative;
  }
  .ym-footer {
    display: flex;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid #e5e7eb;
    background: #fafafa;
  }
  .ym-btn {
    flex: 1;
    cursor: pointer;
    font-size: 13px;
    padding: 6px 0;
    border-radius: 6px;
    text-align: center;
  }
  .ym-btn--cancel {
    background: #e5e7eb;
    color: #374151;
  }
  .ym-btn--confirm {
    background: #6366f1;
    color: #fff;
  }
`)

export const YearMonthPanel = defineComponent({
  props: {
    year: { type: Number, required: true },
    month: { type: Number, required: true },
  },
  emits: ['confirm', 'cancel'],
  setup(props, { emit }) {
    injectStyles()

    const YEAR_RANGE = 30
    const YEAR_START = new Date().getFullYear() - 10
    const years = Array.from({ length: YEAR_RANGE }, (_, i) => YEAR_START + i)
    const months = MONTH_NAMES

    const yearIndex = ref(years.indexOf(props.year))
    const monthIndex = ref(props.month)

    function confirm() {
      emit('confirm', {
        year: years[yearIndex.value],
        month: monthIndex.value,
      })
    }

    function cancel() {
      emit('cancel')
    }

    return () => (
      <div class="ym-root">
        <div class="ym-columns">
          <ScrollColumn
            items={years}
            initialIndex={yearIndex.value}
            onChange={(idx: number) => { yearIndex.value = idx }}
          />
          <ScrollColumn
            items={months}
            initialIndex={monthIndex.value}
            onChange={(idx: number) => { monthIndex.value = idx }}
          />
        </div>

        <div class="ym-footer">
          <div class="ym-btn ym-btn--cancel" onClick={cancel}>Cancel</div>
          <div class="ym-btn ym-btn--confirm" onClick={confirm}>Confirm</div>
        </div>
      </div>
    )
  },
})
