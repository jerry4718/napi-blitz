import { computed, defineComponent, ref } from 'vue'
import { ensureCss } from '../../utils/useCss.ts'
import { WEEKDAYS_SHORT, MONTH_NAMES } from '../../constants.ts'

const WEEKDAYS = WEEKDAYS_SHORT

function formatDate(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function isSameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear()
  && a.getMonth() === b.getMonth()
  && a.getDate() === b.getDate()
}

interface CalendarCell {
  date: Date
  inCurrentMonth: boolean
  isToday: boolean
}

/** Build a 6-row x 7-col calendar grid for the given year/month. */
function buildCalendarGrid(year: number, month: number): CalendarCell[][] {
  const firstOfMonth = new Date(year, month, 1)
  const startOffset = firstOfMonth.getDay()
  const gridStart = new Date(year, month, 1 - startOffset)
  const today = new Date()

  const rows: CalendarCell[][] = []
  for (let row = 0; row < 6; row++) {
    const week: CalendarCell[] = []
    for (let col = 0; col < 7; col++) {
      const date = new Date(gridStart)
      date.setDate(gridStart.getDate() + row * 7 + col)
      week.push({
        date,
        inCurrentMonth: date.getMonth() === month,
        isToday: isSameDay(date, today),
      })
    }
    rows.push(week)
  }
  return rows
}

const injectStyles = ensureCss(`
  .dp-root {
    display: flex;
    flex-direction: column;
    width: 260px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 4px 24px rgba(0,0,0,0.12);
    border: 1px solid #e0e0e0;
    font-family: system-ui, sans-serif;
    user-select: none;
    overflow: hidden;
  }
  .dp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: #4f46e5;
    color: #fff;
  }
  .dp-nav {
    cursor: pointer;
    font-size: 18px;
    padding: 4px 8px;
    border-radius: 4px;
  }
  .dp-month-label {
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    padding: 2px 8px;
    border-radius: 4px;
  }
  .dp-month-label:hover {
    background: rgba(255,255,255,0.15);
  }
  .dp-weekdays {
    display: flex;
    background: #f5f3ff;
  }
  .dp-weekday {
    flex: 1;
    text-align: center;
    padding: 8px 0;
    font-size: 12px;
    font-weight: 600;
    color: #6b7280;
  }
  .dp-week {
    display: flex;
  }
  .dp-cell {
    flex: 1;
    text-align: center;
    padding: 8px 0;
    font-size: 14px;
    cursor: pointer;
    border-radius: 6px;
    margin: 1px;
  }
  .dp-cell--selected {
    background: #4f46e5;
    color: #fff;
  }
  .dp-cell--today {
    background: #eef2ff;
    font-weight: 700;
  }
  .dp-cell--other-month {
    color: #d1d5db;
  }
  .dp-cell--in-month {
    color: #1f2937;
  }
  .dp-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-top: 1px solid #e5e7eb;
    background: #fafafa;
  }
  .dp-selected-label {
    font-size: 13px;
    color: #4b5563;
  }
  .dp-actions {
    display: flex;
    gap: 8px;
  }
  .dp-btn {
    cursor: pointer;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
  }
  .dp-btn--clear {
    background: #e5e7eb;
    color: #374151;
  }
  .dp-btn--today {
    background: #6366f1;
    color: #fff;
  }
`)

export const DatePanel = defineComponent({
  emits: ['select', 'labelClick'],
  setup(_, { emit, slots }) {
    injectStyles()

    const today = new Date()
    const viewYear = ref(today.getFullYear())
    const viewMonth = ref(today.getMonth())
    const selectedDate = ref<Date | null>(null)

    const monthLabel = computed(() =>
      `${MONTH_NAMES[viewMonth.value]} ${viewYear.value}`,
    )

    const grid = computed(() =>
      buildCalendarGrid(viewYear.value, viewMonth.value),
    )

    function prevMonth() {
      if (viewMonth.value === 0) {
        viewMonth.value = 11
        viewYear.value--
      } else {
        viewMonth.value--
      }
    }

    function nextMonth() {
      if (viewMonth.value === 11) {
        viewMonth.value = 0
        viewYear.value++
      } else {
        viewMonth.value++
      }
    }

    function pickDate(d: Date) {
      selectedDate.value = d
      emit('select', d)
    }

    function clearSelection() {
      selectedDate.value = null
    }

    function goToToday() {
      const now = new Date()
      viewYear.value = now.getFullYear()
      viewMonth.value = now.getMonth()
      pickDate(now)
    }

    const selectedLabel = computed(() =>
      selectedDate.value ? formatDate(selectedDate.value) : 'No date selected',
    )

    function onLabelClick(e: { clientX: number; clientY: number }) {
      emit('labelClick', { year: viewYear.value, month: viewMonth.value, clientX: e.clientX, clientY: e.clientY })
    }

    return () => (
      <div class="dp-root">
        <div class="dp-header">
          <div class="dp-nav" onClick={prevMonth}>‹</div>
          <div class="dp-month-label" onClick={onLabelClick}>{monthLabel.value}</div>
          <div class="dp-nav" onClick={nextMonth}>›</div>
        </div>

        <div class="dp-weekdays">
          {WEEKDAYS.map((wd) => (
            <div class="dp-weekday">{wd}</div>
          ))}
        </div>

        <div>
          {grid.value.map((week) => (
            <div class="dp-week">
              {week.map((cell) => {
                const isSelected = selectedDate.value && isSameDay(cell.date, selectedDate.value)

                const classes = [
                  'dp-cell',
                  isSelected ? 'dp-cell--selected'
                    : cell.isToday ? 'dp-cell--today'
                    : cell.inCurrentMonth ? 'dp-cell--in-month'
                    : 'dp-cell--other-month',
                ]

                return (
                  <div class={classes} onClick={() => pickDate(cell.date)}>
                    {cell.date.getDate()}
                  </div>
                )
              })}
            </div>
          ))}
        </div>

        <div class="dp-footer">
          <div class="dp-selected-label">{selectedLabel.value}</div>
          <div class="dp-actions">
            <div class="dp-btn dp-btn--clear" onClick={clearSelection}>Clear</div>
            <div class="dp-btn dp-btn--today" onClick={goToToday}>Today</div>
          </div>
        </div>

        {slots.monthPicker?.({ year: viewYear.value, month: viewMonth.value })}
      </div>
    )
  },
})
