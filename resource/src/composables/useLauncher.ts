import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { Bootstrap, LauncherCommand, Settings, UiMessage, Zone } from '../types'
import { isValidTimeZone } from './useClocks'

const defaultSettings: Settings = {
  mode: 'zone',
  zoneId: 'Asia/Shanghai',
  offset: 8,
  executable: '',
  dreamSkinCompatible: false,
}

function snapshot(settings: Settings) {
  return JSON.stringify(settings)
}

function cloneSettings(settings: Settings): Settings {
  return { ...settings }
}

export function useLauncher() {
  const previewMode = import.meta.env.DEV && new URLSearchParams(window.location.search).get('ui-preview') === '1'
  const platform = ref('windows')
  const settings = ref<Settings>(cloneSettings(defaultSettings))
  const zones = ref<Zone[]>([])
  const detected = ref('')
  const localZone = ref(Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC')
  const busy = ref(false)
  const busyCommand = ref<LauncherCommand | null>(null)
  const initializing = ref(true)
  const bootstrapFailed = ref(false)
  const savedSettings = ref('')
  const message = ref<UiMessage>({ kind: 'pending', summary: '正在读取设置并查找 Codex…' })

  const effectivePath = computed(() => settings.value.executable.trim() || detected.value)
  const dirty = computed(() => Boolean(savedSettings.value) && snapshot(settings.value) !== savedSettings.value)
  const zoneError = computed(() => settings.value.mode === 'zone' && !isValidTimeZone(settings.value.zoneId)
    ? '请输入或选择有效的 IANA 时区。'
    : '')
  const launchUnavailableReason = computed(() => {
    if (initializing.value) return '设置尚未读取完成。'
    if (bootstrapFailed.value) return '请先重新读取设置。'
    if (zoneError.value) return zoneError.value
    if (!effectivePath.value) return '未找到 Codex 客户端，请选择程序路径。'
    return ''
  })

  async function call<T>(command: string, payload: Record<string, unknown> = {}) {
    return invoke<T>('backend', { command, payload })
  }

  function setMessage(kind: UiMessage['kind'], summary: string, detail?: string, preserveError = false) {
    if (preserveError && message.value.kind === 'error') return
    message.value = { kind, summary, detail }
  }

  async function initialize() {
    if (initializing.value && !bootstrapFailed.value && savedSettings.value) return
    initializing.value = true
    bootstrapFailed.value = false
    setMessage('pending', '正在读取设置并查找 Codex…')
    if (previewMode) {
      const previewSettings = cloneSettings(defaultSettings)
      if (new URLSearchParams(window.location.search).get('ui-mode') === 'offset') previewSettings.mode = 'offset'
      settings.value = previewSettings
      zones.value = [
        { label: '北京', id: 'Asia/Shanghai', windowsId: 'China Standard Time' },
        { label: '纽约', id: 'America/New_York', windowsId: 'Eastern Standard Time' },
        { label: '加尔各答', id: 'Asia/Kolkata', windowsId: 'India Standard Time' },
        { label: '加德满都', id: 'Asia/Kathmandu', windowsId: 'Nepal Standard Time' },
        { label: '洛杉矶', id: 'America/Los_Angeles', windowsId: 'Pacific Standard Time' },
      ]
      detected.value = new URLSearchParams(window.location.search).get('ui-path') === 'missing'
        ? ''
        : 'C:\\Program Files\\Codex\\Codex.exe（演示）'
      savedSettings.value = snapshot(previewSettings)
      bootstrapFailed.value = false
      initializing.value = false
      setMessage('info', '开发预览使用固定演示数据；原生操作已禁用。')
      return
    }
    try {
      const data = await call<Bootstrap>('bootstrap')
      platform.value = data.platform || 'windows'
      settings.value = cloneSettings(data.settings)
      zones.value = data.zones
      localZone.value = Intl.DateTimeFormat().resolvedOptions().timeZone || data.localZone || 'UTC'
      detected.value = data.detected
      savedSettings.value = snapshot(settings.value)
      setMessage('success', data.detected ? '设置已读取，客户端已自动找到。' : '设置已读取，请选择 Codex 客户端。')
    } catch (error) {
      bootstrapFailed.value = true
      const detail = String(error)
      setMessage('error', '读取设置失败，请重试。', detail)
    } finally {
      initializing.value = false
    }
  }

  async function browse() {
    if (previewMode) {
      setMessage('info', '开发预览不打开系统文件选择器。')
      return
    }
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Codex 桌面客户端', extensions: platform.value === 'macos' ? ['app'] : ['exe'] }],
      })
      if (!selected) return
      const result = await call<{ path: string }>('validate', { path: selected })
      settings.value = { ...settings.value, executable: result.path }
      setMessage('success', '客户端路径已更新，尚未保存。')
    } catch (error) {
      setMessage('error', '无法使用所选客户端路径。', String(error))
    }
  }

  async function action(command: LauncherCommand) {
    if (previewMode) {
      setMessage('info', '开发预览不执行保存、启动或系统操作。')
      return false
    }
    if (busy.value || initializing.value || bootstrapFailed.value) return false
    if ((command === 'launch' || command === 'save') && zoneError.value) {
      setMessage('error', '时区设置无效。', zoneError.value)
      return false
    }
    if (command === 'launch' && !effectivePath.value) {
      setMessage('error', '无法启动 Codex。', launchUnavailableReason.value)
      return false
    }

    const pending: Record<LauncherCommand, string> = {
      save: '正在保存设置…',
      launch: '正在保存设置并启动 Codex…',
      create_shortcut: '正在创建桌面快捷方式…',
      launch_dream_skin: '正在打开 Dream Skin…',
    }
    const fallback: Record<LauncherCommand, string> = {
      save: '设置已保存。',
      launch: 'Codex 已启动。',
      create_shortcut: '桌面快捷方式已创建。',
      launch_dream_skin: 'Dream Skin 已打开。',
    }
    const submitted = cloneSettings(settings.value)
    busy.value = true
    busyCommand.value = command
    setMessage('pending', pending[command])
    try {
      const data = await call<Record<string, unknown>>(command, { settings: submitted })
      if (command === 'save' || command === 'launch') savedSettings.value = snapshot(submitted)
      setMessage('success', typeof data.message === 'string' ? data.message : fallback[command])
      return true
    } catch (error) {
      setMessage('error', `${fallback[command].replace(/[。]$/, '')}失败。`, String(error))
      return false
    } finally {
      busy.value = false
      busyCommand.value = null
    }
  }

  onMounted(() => void initialize())

  return {
    platform,
    settings,
    zones,
    detected,
    localZone,
    busy,
    busyCommand,
    initializing,
    bootstrapFailed,
    message,
    effectivePath,
    dirty,
    zoneError,
    launchUnavailableReason,
    previewMode,
    initialize,
    browse,
    action,
    setMessage,
  }
}
