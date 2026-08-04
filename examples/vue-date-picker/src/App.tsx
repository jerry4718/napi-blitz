import { defineComponent, ref } from 'vue'
import { DatePicker } from './components/DatePicker.tsx'
import { ensureCss } from './utils/useCss.ts'
import { WEEKDAYS_LONG, MONTH_NAMES } from './constants.ts'

function formatLongDate(d: Date): string {
  return `${WEEKDAYS_LONG[d.getDay()]}, ${MONTH_NAMES[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`
}

const injectStyles = ensureCss(`
  .app-root {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    // justify-content: center;
    gap: 24px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    font-family: system-ui, sans-serif;
  }
  .app-title {
    margin: 0;
    font-size: 32px;
    color: #fff;
    text-shadow: 0 2px 8px rgba(0,0,0,0.2);
  }
  .app-result {
    padding: 12px 24px;
    background: rgba(255,255,255,0.15);
    border-radius: 8px;
    color: #fff;
    font-size: 16px;
    border: 1px solid rgba(255,255,255,0.2);
  }
`)

export const App = defineComponent({
  setup() {
    injectStyles()

    const selectedDate = ref<Date | null>(null)

    function onSelect(d: unknown) {
      selectedDate.value = d as Date
    }

    return () => (
      <div class="app-root">
        <h1 class="app-title">Vue Date Picker</h1>
        <DatePicker onSelect={onSelect} />
        <div class="app-result">
          {selectedDate.value
            ? `Selected: ${formatLongDate(selectedDate.value)}`
            : 'Pick a date from the calendar'}
        </div>
      </div>
    )
  },
})
