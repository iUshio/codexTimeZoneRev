<script setup lang="ts">
import { computed } from 'vue'
import { MacLabel } from '@macvue/core'
import type { AppearanceMode } from '../types'
import AppIcon from './ui/AppIcon.vue'
import AppSelect, { type AppSelectItem } from './ui/AppSelect.vue'

const props = defineProps<{ appearanceMode: AppearanceMode }>()
const emit = defineEmits<{ 'update:appearanceMode': [value: AppearanceMode] }>()
const appearanceItems: AppSelectItem[] = [
  { value: 'system', label: '跟随系统' },
  { value: 'light', label: '浅色模式' },
  { value: 'dark', label: '深色模式' },
]
const appearanceLabel = computed(() => ({ system: '跟随系统', light: '浅色模式', dark: '深色模式' })[props.appearanceMode])
const previewMenu = import.meta.env.DEV && new URLSearchParams(window.location.search).get('ui-menu') === 'appearance'

function updateAppearance(value: string | number) {
  if (value === 'system' || value === 'light' || value === 'dark') emit('update:appearanceMode', value)
}
</script>

<template>
  <header class="app-toolbar">
    <div class="toolbar-inner">
      <div class="brand-mark"><AppIcon name="clock" :size="20" /></div>
      <div class="toolbar-title">
        <MacLabel as="h1" variant="headline">Codex 时区启动器</MacLabel>
        <p>仅影响 Codex，系统时区保持不变</p>
      </div>
      <div class="toolbar-appearance">
        <span>外观</span>
        <AppSelect
          :model-value="appearanceMode"
          :items="appearanceItems"
          teleport-to="#appearance-menu-root"
          accessible-name="外观模式"
          :default-open="previewMenu"
          @update:model-value="updateAppearance"
        >
          <template #value>{{ appearanceLabel }}</template>
        </AppSelect>
      </div>
    </div>
  </header>
</template>
