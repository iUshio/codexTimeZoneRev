import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { AppearanceMode, ResolvedAppearance } from '../types'

const storageKey = 'codex-appearance'

export function useAppearance() {
  const appearanceMode = ref<AppearanceMode>('system')
  const resolvedAppearance = ref<ResolvedAppearance>('light')
  let media: MediaQueryList | undefined

  function isAppearanceMode(value: string | null): value is AppearanceMode {
    return value === 'system' || value === 'light' || value === 'dark'
  }

  function resolveAppearance() {
    resolvedAppearance.value = appearanceMode.value === 'system'
      ? (media?.matches ? 'dark' : 'light')
      : appearanceMode.value
    document.documentElement.dataset.macvueAppearance = resolvedAppearance.value
    document.documentElement.style.colorScheme = resolvedAppearance.value
  }

  function handleSystemAppearance() {
    if (appearanceMode.value === 'system') resolveAppearance()
  }

  watch(appearanceMode, (value) => {
    localStorage.setItem(storageKey, value)
    resolveAppearance()
  })

  onMounted(() => {
    media = window.matchMedia('(prefers-color-scheme: dark)')
    const stored = localStorage.getItem(storageKey)
    const previewTheme = import.meta.env.DEV && new URLSearchParams(window.location.search).get('ui-preview') === '1'
      ? new URLSearchParams(window.location.search).get('ui-theme')
      : null
    appearanceMode.value = isAppearanceMode(previewTheme) ? previewTheme : isAppearanceMode(stored) ? stored : 'system'
    resolveAppearance()
    media.addEventListener('change', handleSystemAppearance)
  })

  onBeforeUnmount(() => media?.removeEventListener('change', handleSystemAppearance))

  return { appearanceMode, resolvedAppearance }
}
