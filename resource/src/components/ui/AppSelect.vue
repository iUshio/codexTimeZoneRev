<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { MacPopUpButton, MacPopUpButtonItem } from '@macvue/core'

let activeMenuScrollLocks = 0

function syncWorkspaceScrollLock() {
  document.querySelector<HTMLElement>('.workspace-scroll')
    ?.classList.toggle('workspace-scroll--select-open', activeMenuScrollLocks > 0)
}

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
let menuOpen = false
let portalHost: HTMLElement | null = null

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

function handlePortalScroll(event: Event) {
  const target = event.target
  const host = portalHost ?? document.querySelector<HTMLElement>(props.teleportTo)
  if (!(target instanceof Element) || !host) return

  // MacVue 0.1.0 uses Reka's item-aligned select. Its viewport scroll handler
  // expands the fixed wrapper and resets scrollTop, which detaches the menu.
  if (target.matches('.macvue-pop-up-button-viewport') && host.contains(target)) {
    event.stopPropagation()
  }
}

function resolvePortalHost() {
  const nextHost = document.querySelector<HTMLElement>(props.teleportTo)
  if (nextHost === portalHost) return
  portalHost = nextHost
}

function handleOpenChange(open: boolean) {
  resolvePortalHost()
  updatePortalWidth()
  if (menuOpen === open) return
  menuOpen = open
  activeMenuScrollLocks = Math.max(0, activeMenuScrollLocks + (open ? 1 : -1))
  syncWorkspaceScrollLock()
}

onMounted(() => {
  window.addEventListener('scroll', handlePortalScroll, true)
  resolvePortalHost()
  updatePortalWidth()
  if (root.value) {
    observer = new ResizeObserver(updatePortalWidth)
    observer.observe(root.value)
  }
  if (props.defaultOpen) handleOpenChange(true)
})
onBeforeUnmount(() => {
  observer?.disconnect()
  window.removeEventListener('scroll', handlePortalScroll, true)
  portalHost = null
  if (menuOpen) {
    menuOpen = false
    activeMenuScrollLocks = Math.max(0, activeMenuScrollLocks - 1)
    syncWorkspaceScrollLock()
  }
})
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
      @update:open="handleOpenChange"
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
