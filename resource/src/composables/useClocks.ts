import { computed, onBeforeUnmount, onMounted, ref, type Ref } from 'vue'
import type { ClockValue, Settings } from '../types'

const dateTimeFormatters = new Map<string, Intl.DateTimeFormat>()

function formatter(locale: 'zh-CN' | 'en-US', zone: string) {
  const key = `${locale}:${zone}`
  let value = dateTimeFormatters.get(key)
  if (!value) {
    value = new Intl.DateTimeFormat(locale, {
      timeZone: zone,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      weekday: locale === 'zh-CN' ? 'long' : undefined,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hourCycle: 'h23',
    })
    dateTimeFormatters.set(key, value)
  }
  return value
}

function parts(date: Date, zone: string) {
  return Object.fromEntries(formatter('zh-CN', zone).formatToParts(date).map((item) => [item.type, item.value]))
}

function offsetFor(date: Date, zone: string) {
  const value = Object.fromEntries(formatter('en-US', zone).formatToParts(date).map((item) => [item.type, item.value]))
  return Math.round((Date.UTC(+value.year, +value.month - 1, +value.day, +value.hour, +value.minute, +value.second) - date.getTime()) / 60_000)
}

export function offsetText(minutes: number) {
  const sign = minutes < 0 ? '−' : '+'
  const value = Math.abs(minutes)
  return `UTC ${sign}${String(Math.floor(value / 60)).padStart(2, '0')}:${String(value % 60).padStart(2, '0')}`
}

export function isValidTimeZone(zone: string) {
  try {
    new Intl.DateTimeFormat('zh-CN', { timeZone: zone }).format()
    return true
  } catch {
    return false
  }
}

export function useClocks(settings: Ref<Settings>, localZone: Ref<string>) {
  const previewMode = import.meta.env.DEV && new URLSearchParams(window.location.search).get('ui-preview') === '1'
  const now = ref(previewMode ? new Date('2026-09-08T09:24:08Z') : new Date())
  let clockTimer = 0

  function clock(zone: string, fixed?: number): ClockValue {
    const safeZone = isValidTimeZone(zone) ? zone : 'UTC'
    const date = fixed === undefined ? now.value : new Date(now.value.getTime() + fixed * 3_600_000)
    const value = parts(date, fixed === undefined ? safeZone : 'UTC')
    return {
      time: `${value.hour}:${value.minute}:${value.second}`,
      date: `${value.year} 年 ${value.month} 月 ${value.day} 日　${value.weekday}`,
      offset: offsetText(fixed === undefined ? offsetFor(now.value, safeZone) : fixed * 60),
    }
  }

  const targetClock = computed(() => settings.value.mode === 'offset'
    ? clock('UTC', settings.value.offset)
    : clock(settings.value.zoneId))
  const localClock = computed(() => clock(localZone.value))

  function tickClock() {
    window.clearTimeout(clockTimer)
    now.value = new Date()
    if (!document.hidden) clockTimer = window.setTimeout(tickClock, 1000 - (Date.now() % 1000) + 8)
  }

  function handleVisibilityChange() {
    if (document.hidden) window.clearTimeout(clockTimer)
    else tickClock()
  }

  onMounted(() => {
    if (previewMode) return
    document.addEventListener('visibilitychange', handleVisibilityChange)
    tickClock()
  })

  onBeforeUnmount(() => {
    if (previewMode) return
    window.clearTimeout(clockTimer)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  })

  return { targetClock, localClock }
}
