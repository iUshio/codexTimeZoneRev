<script setup lang="ts">
import { computed } from 'vue'
import { MacButton, MacLabel, MacSegment, MacSegmentedControl, MacSwitch, MacTextField } from '@macvue/core'
import type { Settings, Zone } from '../types'
import { offsetText } from '../composables/useClocks'
import AppIcon from './ui/AppIcon.vue'
import AppSelect, { type AppSelectItem } from './ui/AppSelect.vue'
import SettingRow from './ui/SettingRow.vue'

const props = defineProps<{
  settings: Settings
  zones: Zone[]
  detected: string
  platform: string
  busy: boolean
  initializing: boolean
  bootstrapFailed: boolean
  zoneError: string
  currentOffset: string
  previewMode: boolean
}>()

const emit = defineEmits<{
  'update:settings': [settings: Settings]
  browse: []
  openSkin: []
  retry: []
}>()

const offsets = Array.from({ length: 27 }, (_, index) => index - 12)
const zoneItems = computed<AppSelectItem[]>(() => props.zones.map((zone) => ({
  value: zone.id,
  label: zone.label === zone.id ? zone.id : zone.label,
  detail: zone.label === zone.id ? undefined : zone.id,
})))
const offsetItems = computed<AppSelectItem[]>(() => offsets.map((offset) => ({ value: offset, label: offsetText(offset * 60) })))
const selectedZone = computed(() => props.zones.find((zone) => zone.id === props.settings.zoneId))
const selectedZoneLabel = computed(() => {
  const selected = selectedZone.value
  if (!selected || selected.label === selected.id) return props.settings.zoneId
  return `${selected.label} · ${selected.id}`
})
const previewMenu = import.meta.env.DEV ? new URLSearchParams(window.location.search).get('ui-menu') : null

function update(patch: Partial<Settings>) {
  emit('update:settings', { ...props.settings, ...patch })
}

function setMode(value: string | string[]) {
  if (value === 'zone' || value === 'offset') update({ mode: value })
}

function setZone(value: string | number) {
  if (typeof value === 'string') update({ zoneId: value })
}

function setOffset(value: string | number) {
  if (typeof value === 'number') update({ offset: value })
}
</script>

<template>
  <section class="surface settings-panel" aria-labelledby="settings-title">
    <div v-if="bootstrapFailed" class="inline-alert" role="alert">
      <AppIcon name="alert" />
      <span>设置读取失败，当前占位配置不会被保存。</span>
      <MacButton size="small" :disabled="initializing" @click="emit('retry')">重新读取</MacButton>
    </div>

    <div class="settings-section settings-section--first">
      <div class="section-heading">
        <MacLabel id="settings-title" as="h2" variant="headline">时区设置</MacLabel>
        <p>选择后，右侧时间预览会立即同步。</p>
      </div>
      <SettingRow label="设置方式">
        <MacSegmentedControl
          :model-value="settings.mode"
          type="single"
          size="regular"
          aria-label="设置方式"
          :disabled="busy || initializing || bootstrapFailed"
          @update:model-value="setMode"
        >
          <MacSegment value="zone">地区时区</MacSegment>
          <MacSegment value="offset">固定 UTC 偏移</MacSegment>
        </MacSegmentedControl>
      </SettingRow>

      <SettingRow
        v-if="settings.mode === 'zone'"
        label="时区名称"
        description="按地区规则自动处理夏令时。"
        :error="zoneError"
      >
        <AppSelect
          :model-value="settings.zoneId"
          :items="zoneItems"
          :disabled="busy || initializing || bootstrapFailed"
          teleport-to="#timezone-menu-root"
          accessible-name="时区名称"
          :default-open="previewMenu === 'timezone'"
          @update:model-value="setZone"
        >
          <template #value>{{ selectedZoneLabel }}</template>
        </AppSelect>
      </SettingRow>

      <SettingRow
        v-else
        label="UTC 固定偏移"
        description="固定偏移不随季节变化；半小时和四十五分钟时区请使用地区时区。"
      >
        <AppSelect
          :model-value="settings.offset"
          :items="offsetItems"
          :disabled="busy || initializing || bootstrapFailed"
          teleport-to="#offset-menu-root"
          accessible-name="UTC 固定偏移"
          :default-open="previewMenu === 'offset'"
          @update:model-value="setOffset"
        >
          <template #value>{{ offsetText(settings.offset * 60) }}</template>
        </AppSelect>
      </SettingRow>

      <div class="offset-summary">
        <span>当前 UTC 偏移</span>
        <strong>{{ currentOffset }}</strong>
      </div>
    </div>

    <div class="settings-section">
      <div class="section-heading">
        <MacLabel as="h2" variant="headline">客户端设置</MacLabel>
        <p>路径留空时使用自动查找结果。</p>
      </div>
      <SettingRow label="客户端路径" field-id="client-path">
        <div class="path-row">
          <MacTextField
            id="client-path"
            :model-value="settings.executable"
            size="regular"
            placeholder="留空时自动查找 Codex"
            :disabled="busy || initializing || bootstrapFailed"
            @update:model-value="update({ executable: $event })"
          />
          <MacButton :disabled="busy || initializing || bootstrapFailed || previewMode" title="开发预览不打开系统选择器" @click="emit('browse')">
            <AppIcon name="folder" />浏览…
          </MacButton>
        </div>
        <template #after>
          <p class="path-result" :class="{ 'path-result--warning': !detected && !settings.executable }">
            {{ settings.executable ? `已选择：${settings.executable}` : detected ? `已自动找到：${detected}` : '未找到客户端，请选择程序路径。' }}
          </p>
        </template>
      </SettingRow>
    </div>

    <div class="settings-section dream-skin-section">
      <div class="section-heading dream-skin-heading">
        <div>
          <MacLabel as="h2" variant="headline">Dream Skin 兼容启动</MacLabel>
          <p>{{ platform === 'macos' ? '适用于 macOS 客户端。' : '适用于 Windows 客户端。' }}</p>
        </div>
        <MacSwitch
          :model-value="settings.dreamSkinCompatible"
          aria-label="Dream Skin 兼容启动"
          :disabled="busy || initializing || bootstrapFailed"
          @update:model-value="update({ dreamSkinCompatible: $event })"
        />
      </div>
      <p class="skin-description">
        {{ settings.dreamSkinCompatible
          ? '启动时将应用并验证皮肤；已运行的客户端需完全退出后重启。'
          : '开启后，使用兼容模式启动 Codex 并按现有流程应用皮肤。' }}
      </p>
      <MacButton :disabled="busy || initializing || bootstrapFailed || previewMode" title="开发预览不执行原生操作" @click="emit('openSkin')">打开 Dream Skin</MacButton>
    </div>
  </section>
</template>
