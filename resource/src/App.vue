<script setup lang="ts">
import type { IpInfo, Settings } from './types'
import { useAppearance } from './composables/useAppearance'
import { useClocks } from './composables/useClocks'
import { useLauncher } from './composables/useLauncher'
import { useNetworkInfo } from './composables/useNetworkInfo'
import AppToolbar from './components/AppToolbar.vue'
import LauncherSettings from './components/LauncherSettings.vue'
import TimePreview from './components/TimePreview.vue'
import NetworkPanel from './components/NetworkPanel.vue'
import LaunchBar from './components/LaunchBar.vue'

const { appearanceMode } = useAppearance()
const launcher = useLauncher()
const clocks = useClocks(launcher.settings, launcher.localZone)
const networkInfo = useNetworkInfo()

function updateSettings(settings: Settings) {
  launcher.settings.value = settings
}

function useNetworkTimezone(info: IpInfo) {
  launcher.settings.value = { ...launcher.settings.value, mode: 'zone', zoneId: info.timezone }
  launcher.setMessage('info', `已选择 ${info.timezone}，保存并启动后生效。`)
}

async function refreshNetwork() {
  const result = await networkInfo.refreshNetwork()
  if (!result) return
  if (result.domestic || result.international) launcher.setMessage('success', '公网 IP 信息已更新。', undefined, true)
  else launcher.setMessage('error', '未查询到有效公网 IP。', result.errors?.join('\n'))
}
</script>

<template>
  <div id="appearance-menu-root" class="portal-host" />
  <div id="timezone-menu-root" class="portal-host" />
  <div id="offset-menu-root" class="portal-host" />

  <main class="app-shell" data-macvue-glass="off">
    <AppToolbar v-model:appearance-mode="appearanceMode" />
    <div class="workspace-scroll">
      <div class="workspace-content">
        <div class="primary-grid">
          <LauncherSettings
            :settings="launcher.settings.value"
            :zones="launcher.zones.value"
            :detected="launcher.detected.value"
            :platform="launcher.platform.value"
            :busy="launcher.busy.value"
            :initializing="launcher.initializing.value"
            :bootstrap-failed="launcher.bootstrapFailed.value"
            :zone-error="launcher.zoneError.value"
            :current-offset="clocks.targetClock.value.offset"
            :preview-mode="launcher.previewMode"
            @update:settings="updateSettings"
            @browse="launcher.browse"
            @open-skin="launcher.action('launch_dream_skin')"
            @retry="launcher.initialize"
          />
          <TimePreview
            :settings="launcher.settings.value"
            :zones="launcher.zones.value"
            :target-clock="clocks.targetClock.value"
            :local-clock="clocks.localClock.value"
            :local-zone="launcher.localZone.value"
            :platform="launcher.platform.value"
          />
          <NetworkPanel
            :network="networkInfo.network.value"
            :network-busy="networkInfo.networkBusy.value"
            :has-cached-network="networkInfo.hasCachedNetwork.value"
            :disabled="launcher.busy.value"
            :copy-feedback="networkInfo.copyFeedback.value"
            :copy-errors="networkInfo.copyErrors.value"
            @refresh="refreshNetwork"
            @copy="networkInfo.copyIp"
            @use="useNetworkTimezone"
          />
        </div>
      </div>
    </div>
    <LaunchBar
      :message="launcher.message.value"
      :dirty="launcher.dirty.value"
      :busy="launcher.busy.value"
      :busy-command="launcher.busyCommand.value"
      :initializing="launcher.initializing.value"
      :bootstrap-failed="launcher.bootstrapFailed.value"
      :launch-unavailable-reason="launcher.launchUnavailableReason.value"
      :preview-mode="launcher.previewMode"
      @save="launcher.action('save')"
      @launch="launcher.action('launch')"
      @shortcut="launcher.action('create_shortcut')"
    />
  </main>
</template>
