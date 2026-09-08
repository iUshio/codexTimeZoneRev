<script setup lang="ts">
import { ref } from 'vue'
import { MacButton, MacSpinner } from '@macvue/core'
import type { LauncherCommand, UiMessage } from '../types'
import AppIcon from './ui/AppIcon.vue'

defineProps<{
  message: UiMessage
  dirty: boolean
  busy: boolean
  busyCommand: LauncherCommand | null
  initializing: boolean
  bootstrapFailed: boolean
  launchUnavailableReason: string
  previewMode: boolean
}>()
const emit = defineEmits<{ save: []; launch: []; shortcut: [] }>()
const showDetail = ref(false)
</script>

<template>
  <footer class="launch-bar">
    <div class="launch-bar-inner">
      <div class="status-area">
        <div
          class="status-summary"
          :class="`status-summary--${message.kind}`"
          :role="message.kind === 'error' ? 'alert' : 'status'"
          :aria-live="message.kind === 'error' ? 'assertive' : 'polite'"
        >
          <MacSpinner v-if="message.kind === 'pending'" size="small" />
          <AppIcon v-else :name="message.kind === 'error' ? 'alert' : message.kind === 'success' ? 'check' : 'info'" />
          <span>{{ dirty ? '有未保存修改 · ' : '' }}{{ message.summary }}</span>
          <button v-if="message.detail" class="detail-toggle" type="button" @click="showDetail = !showDetail">
            {{ showDetail ? '收起详情' : '查看详情' }}
          </button>
        </div>
        <div v-if="showDetail && message.detail" class="status-detail">{{ message.detail }}</div>
        <p class="running-tip">客户端已运行？请先保存工作并完全退出。</p>
      </div>
      <div class="launch-actions">
        <div class="primary-actions">
          <MacButton :disabled="busy || initializing || bootstrapFailed || previewMode" title="开发预览不执行保存" @click="emit('save')">
            <MacSpinner v-if="busyCommand === 'save'" size="small" />
            {{ busyCommand === 'save' ? '正在保存…' : '保存设置' }}
          </MacButton>
          <MacButton
            size="large"
            variant="prominent"
            :disabled="busy || initializing || bootstrapFailed || previewMode || Boolean(launchUnavailableReason)"
            :title="previewMode ? '开发预览不执行启动' : launchUnavailableReason || '保存设置并启动 Codex'"
            @click="emit('launch')"
          >
            <MacSpinner v-if="busyCommand === 'launch'" size="small" />
            {{ busyCommand === 'launch' ? '正在启动…' : '保存并启动 Codex' }}
          </MacButton>
        </div>
        <MacButton size="small" :disabled="busy || initializing || bootstrapFailed || previewMode" title="开发预览不创建快捷方式" @click="emit('shortcut')">
          创建桌面快捷方式
        </MacButton>
      </div>
    </div>
  </footer>
</template>
