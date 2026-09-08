<script setup lang="ts">
import { computed } from 'vue'
import { MacButton, MacLabel } from '@macvue/core'
import type { IpInfo } from '../types'
import AppIcon from './ui/AppIcon.vue'

const props = defineProps<{
  regionKey: 'domestic' | 'international'
  title: string
  description: string
  info: IpInfo | null
  loading: boolean
  cached: boolean
  disabled: boolean
  copyState: 'idle' | 'copied' | 'error'
  copyError?: string
}>()
const emit = defineEmits<{ copy: [key: 'domestic' | 'international', ip: string]; use: [info: IpInfo] }>()
const location = computed(() => props.info
  ? [props.info.country, props.info.region, props.info.city].filter(Boolean).join('，') || '未知'
  : '未知')
</script>

<template>
  <article class="network-region" :aria-labelledby="`${regionKey}-network-title`">
    <div class="network-region-heading">
      <div>
        <MacLabel :id="`${regionKey}-network-title`" as="h3" variant="headline">{{ title }}</MacLabel>
        <p>{{ description }}</p>
      </div>
      <span v-if="loading" class="network-state">{{ cached && info ? '上次结果 · 更新中' : '正在查询…' }}</span>
    </div>

    <template v-if="info">
      <div class="ip-line">
        <strong>{{ info.ip }}</strong>
        <MacButton
          class="icon-button"
          size="small"
          :disabled="disabled"
          :aria-label="`复制${title} IP 地址`"
          :title="copyState === 'copied' ? '已复制' : '复制 IP 地址'"
          @click="emit('copy', regionKey, info.ip)"
        >
          <AppIcon :name="copyState === 'copied' ? 'check' : 'copy'" />
        </MacButton>
        <span v-if="copyState === 'copied'" class="copy-feedback">已复制</span>
      </div>
      <p v-if="copyState === 'error'" class="copy-error">复制失败：{{ copyError }}</p>
      <dl class="network-details">
        <div><dt>所在地</dt><dd>{{ location }}</dd></div>
        <div><dt>网络组织</dt><dd>{{ info.organization || '未知' }}</dd></div>
        <div><dt>时区</dt><dd class="timezone-value">{{ info.timezone }}</dd></div>
      </dl>
      <MacButton size="small" :disabled="disabled" @click="emit('use', info)">使用此时区</MacButton>
      <p class="data-source">数据来源：{{ info.source }} / IP125</p>
    </template>
    <div v-else class="network-empty">
      <span>{{ loading ? '正在查询公网出口与时区…' : '暂未获取到有效信息' }}</span>
    </div>
  </article>
</template>
