<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { MacPopUpButton, MacPopUpButtonItem } from '@macvue/core'

export type AppSelectItem = {
  value: string | number
  label: string
  detail?: string
  disabled?: boolean
}

const props = withDefaults(defineProps<{
  modelValue: string | number
  items: AppSelectItem[]
  teleportTo: string
  accessibleName: string
  disabled?: boolean
  size?: 'mini' | 'small' | 'regular' | 'large'
  defaultOpen?: boolean
}>(), { disabled: false, size: 'regular', defaultOpen: false })

const emit = defineEmits<{ 'update:modelValue': [value: string | number] }>()
const root = ref<HTMLElement | null>(null)
let observer: ResizeObserver | undefined
let lastPortalWidth = 0

function updateValue(value: string | number) {
  emit('update:modelValue', value)
}

function updatePortalWidth() {
  const host = document.querySelector<HTMLElement>(props.teleportTo)
  const width = Math.round(root.value?.getBoundingClientRect().width || 0)
  if (!host || !width || width === lastPortalWidth) return
  lastPortalWidth = width
  host.style.setProperty('--app-select-trigger-width', `${width}px`)
}

onMounted(() => {
  updatePortalWidth()
  if (root.value) {
    observer = new ResizeObserver(updatePortalWidth)
    observer.observe(root.value)
  }
})
onBeforeUnmount(() => observer?.disconnect())
</script>

<template>
  <div ref="root" class="app-select">
    <MacPopUpButton
      :model-value="modelValue"
      :size="size"
      :disabled="disabled"
      :teleport-to="teleportTo"
      :default-open="defaultOpen"
      :aria-label="accessibleName"
      @update:model-value="updateValue"
      @update:open="updatePortalWidth"
    >
      <template #value="slotProps">
        <slot name="value" v-bind="slotProps" />
      </template>
      <MacPopUpButtonItem
        v-for="item in items"
        :key="String(item.value)"
        :value="item.value"
        :disabled="item.disabled"
        :text-value="item.detail ? `${item.label} ${item.detail}` : item.label"
      >
        <span class="app-select-item-label">{{ item.label }}</span>
        <span v-if="item.detail" class="app-select-item-detail">{{ item.detail }}</span>
      </MacPopUpButtonItem>
    </MacPopUpButton>
  </div>
</template>
