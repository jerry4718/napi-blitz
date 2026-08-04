import { defineComponent, ref } from 'vue'
import { ensureCss } from '../utils/useCss.ts'
import { Popup } from './Popup.tsx'
import { DatePanel } from './panels/DatePanel.tsx'
import { YearMonthPicker } from './YearMonthPicker.tsx'

function formatDate(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

const injectStyles = ensureCss(`
  .dp-input {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    width: 260px;
    background: #fff;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    font-family: system-ui, sans-serif;
    font-size: 14px;
    color: #1f2937;
    cursor: pointer;
  }
  .dp-input:hover {
    border-color: #6366f1;
  }
  .dp-input-icon {
    color: #6b7280;
    font-size: 16px;
  }
  .dp-input-text {
    flex: 1;
  }
  .dp-input-text--placeholder {
    color: #9ca3af;
  }
`)

export const DatePicker = defineComponent({
  emits: ['select'],
  setup(_, { emit }) {
    injectStyles()

    const selectedDate = ref<Date | null>(null)
    const visible = ref(false)
    const x = ref(0)
    const y = ref(0)

    // YearMonthPicker state
    const ymVisible = ref(false)
    const ymX = ref(0)
    const ymY = ref(0)
    const ymYear = ref(new Date().getFullYear())
    const ymMonth = ref(new Date().getMonth())

    function open(e: { clientX: number; clientY: number }) {
      x.value = e.clientX
      y.value = e.clientY + 36
      visible.value = true
    }

    function close() {
      visible.value = false
      ymVisible.value = false
    }

    function onSelect(d: unknown) {
      selectedDate.value = d as Date
      emit('select', d)
      close()
    }

    function onLabelClick(e: { year: number; month: number; clientX: number; clientY: number }) {
      ymYear.value = e.year
      ymMonth.value = e.month
      ymX.value = e.clientX
      ymY.value = e.clientY + 20
      ymVisible.value = true
    }

    function onYmConfirm({ year, month }: { year: number; month: number }) {
      ymYear.value = year
      ymMonth.value = month
      ymVisible.value = false
    }

    function onYmCancel() {
      ymVisible.value = false
    }

    return () => (
      <>
        <div class="dp-input" onClick={open}>
          <span class="dp-input-icon">📅</span>
          <span class={['dp-input-text', selectedDate.value ? '' : 'dp-input-text--placeholder']}>
            {selectedDate.value ? formatDate(selectedDate.value) : 'Select date'}
          </span>
        </div>

        <Popup visible={visible.value} x={x.value} y={y.value} onClose={close}>
          {{
            default: () => (
              <DatePanel
                onSelect={onSelect}
                onLabelClick={onLabelClick}
              >
                {{
                  monthPicker: ({ year, month }: { year: number; month: number }) => (
                    <YearMonthPicker
                      year={year}
                      month={month}
                      visible={ymVisible.value}
                      x={ymX.value}
                      y={ymY.value}
                      onConfirm={onYmConfirm}
                      onCancel={onYmCancel}
                    />
                  ),
                }}
              </DatePanel>
            ),
          }}
        </Popup>
      </>
    )
  },
})
