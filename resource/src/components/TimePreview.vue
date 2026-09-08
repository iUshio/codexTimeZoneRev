<script setup lang="ts">
import { computed } from 'vue'
import { MacLabel } from '@macvue/core'
import type { ClockValue, Settings, Zone } from '../types'

const props = defineProps<{
  settings: Settings
  zones: Zone[]
  targetClock: ClockValue
  localClock: ClockValue
  localZone: string
  platform: string
}>()

const targetName = computed(() => {
  if (props.settings.mode === 'offset') return `固定偏移 ${props.targetClock.offset}`
  const zone = props.zones.find((item) => item.id === props.settings.zoneId)
  if (!zone || zone.label === zone.id) return props.settings.zoneId
  return `${zone.label} · ${zone.id}`
})
</script>

<template>
  <section class="surface preview-panel" aria-labelledby="preview-title">
    <div class="section-heading">
      <MacLabel id="preview-title" as="h2" variant="headline">时间预览</MacLabel>
      <p>目标时间与电脑本地时间实时对照。</p>
    </div>
    <div class="clock-block clock-block--target">
      <span class="clock-label">目标时间</span>
      <time :datetime="targetClock.time">{{ targetClock.time }}</time>
      <p>{{ targetClock.date }}</p>
      <strong>{{ targetName }}</strong>
      <small>{{ targetClock.offset }} · {{ settings.mode === 'zone' ? '地区时间规则' : '固定偏移' }}</small>
    </div>
    <div class="clock-block clock-block--local">
      <span class="clock-label">本地时间</span>
      <time :datetime="localClock.time">{{ localClock.time }}</time>
      <p>{{ localClock.date }}</p>
      <strong>{{ localZone }}</strong>
      <small>{{ localClock.offset }} · {{ platform === 'macos' ? 'macOS' : 'Windows' }} 本地时区</small>
    </div>
  </section>
</template>
