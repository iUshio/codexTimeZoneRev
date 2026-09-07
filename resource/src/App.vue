<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { MacBadge, MacButton, MacCheckbox, MacGlassPanel, MacLabel, MacPopUpButton, MacPopUpButtonItem, MacSpinner, MacTextField } from '@macvue/core'

type Zone = { label: string; id: string; windowsId: string }
type Settings = { mode: 'zone' | 'offset'; zoneId: string; offset: number; executable: string; dreamSkinCompatible: boolean }
type Bootstrap = { settings: Settings; zones: Zone[]; localZone: string; detected: string }
const settings = ref<Settings>({ mode: 'zone', zoneId: 'Asia/Shanghai', offset: 8, executable: '', dreamSkinCompatible: false })
const zones = ref<Zone[]>([]), detected = ref(''), localZone = ref(Intl.DateTimeFormat().resolvedOptions().timeZone), now = ref(new Date())
const busy = ref(false), status = ref('正在读取设置并查找 Codex…'), appearance = ref<'light' | 'dark'>('light')
let clockTimer = 0; let media: MediaQueryList | undefined; let appearanceHandler: ((event: MediaQueryListEvent) => void) | undefined
const offsets = Array.from({ length: 27 }, (_, index) => index - 12)
const effectivePath = computed(() => settings.value.executable || detected.value)
const targetZone = computed(() => settings.value.mode === 'zone' ? settings.value.zoneId : 'UTC')
function parts(date: Date, zone: string) { return Object.fromEntries(new Intl.DateTimeFormat('zh-CN', { timeZone: zone, year: 'numeric', month: '2-digit', day: '2-digit', weekday: 'long', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).formatToParts(date).map(item => [item.type, item.value])) }
function offsetFor(date: Date, zone: string) { const v = Object.fromEntries(new Intl.DateTimeFormat('en-US', { timeZone: zone, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).formatToParts(date).map(item => [item.type, item.value])); return Math.round((Date.UTC(+v.year, +v.month - 1, +v.day, +v.hour, +v.minute, +v.second) - date.getTime()) / 60000) }
function offsetText(minutes: number) { const sign = minutes < 0 ? '−' : '+'; const value = Math.abs(minutes); return `UTC ${sign}${String(Math.floor(value / 60)).padStart(2, '0')}:${String(value % 60).padStart(2, '0')}` }
function clock(zone: string, fixed?: number) { const date = fixed === undefined ? now.value : new Date(now.value.getTime() + fixed * 3_600_000), p = parts(date, fixed === undefined ? zone : 'UTC'); return { time: `${p.hour}:${p.minute}:${p.second}`, date: `${p.year} 年 ${p.month} 月 ${p.day} 日　${p.weekday}`, offset: offsetText(fixed === undefined ? offsetFor(now.value, zone) : fixed * 60) } }
const targetClock = computed(() => clock(targetZone.value, settings.value.mode === 'offset' ? settings.value.offset : undefined)), localClock = computed(() => clock(localZone.value))
const targetName = computed(() => settings.value.mode === 'zone' ? settings.value.zoneId : `固定偏移 ${targetClock.value.offset}`)
async function call<T>(command: string, payload: Record<string, unknown> = {}) { return invoke<T>('backend', { command, payload }) }
async function initialize() { try { const data = await call<Bootstrap>('bootstrap'); settings.value = data.settings; zones.value = data.zones; localZone.value = Intl.DateTimeFormat().resolvedOptions().timeZone || data.localZone; detected.value = data.detected; status.value = data.detected ? `已找到客户端：${data.detected}` : '未自动找到客户端，可以点击“浏览”手动选择。' } catch (error) { status.value = String(error) } }
async function browse() { const selected = await open({ multiple: false, filters: [{ name: 'Codex 桌面客户端', extensions: ['exe'] }] }); if (!selected) return; try { const result = await call<{ path: string }>('validate', { path: selected }); settings.value.executable = result.path; status.value = '客户端路径已更新。' } catch (error) { status.value = String(error) } }
async function action(command: string, pending: string, success: string) { busy.value = true; status.value = pending; try { const data = await call<Record<string, unknown>>(command, { settings: settings.value }); status.value = typeof data.message === 'string' ? data.message : success } catch (error) { status.value = String(error) } finally { busy.value = false } }
const save = () => action('save', '正在保存设置…', '设置已保存到程序目录的 data 文件夹。'), launch = () => action('launch', '正在保存设置并启动 Codex…', 'Codex 已启动。'), launchSkin = () => action('launch_dream_skin', '正在查找并启动 Dream Skin…', '已启动 Dream Skin。'), shortcut = () => action('create_shortcut', '正在创建桌面快捷方式…', '桌面快捷方式已创建。')
onMounted(() => { media = window.matchMedia('(prefers-color-scheme: dark)'); appearance.value = media.matches ? 'dark' : 'light'; appearanceHandler = event => appearance.value = event.matches ? 'dark' : 'light'; media.addEventListener('change', appearanceHandler); const tick = () => { now.value = new Date(); clockTimer = window.setTimeout(tick, 1000 - (Date.now() % 1000) + 8) }; tick(); initialize() })
onBeforeUnmount(() => { window.clearTimeout(clockTimer); if (media && appearanceHandler) media.removeEventListener('change', appearanceHandler) })
</script>

<template>
  <main class="app-shell" :data-macvue-appearance="appearance" data-macvue-glass="on">
    <div class="ambient ambient-one" /><div class="ambient ambient-two" />
    <section class="content">
      <MacGlassPanel class="hero" material="regular"><div class="brand"><div class="app-icon">◷</div><div><MacLabel variant="large-title">Codex 时区启动器</MacLabel><p>为你的工作，选择合适的时间。</p></div></div><MacBadge class="safety">●　系统时区保持不变</MacBadge></MacGlassPanel>
      <div class="primary-grid">
        <MacGlassPanel class="settings-card" material="regular">
          <div class="section-heading"><MacLabel variant="title-2">时区设置</MacLabel><p>选择目标时区，预览会立即同步。</p></div>
          <label>设置方式</label><div class="select-field"><MacPopUpButton v-model="settings.mode" size="large"><MacPopUpButtonItem value="zone">使用时区名称（推荐，支持夏令时）</MacPopUpButtonItem><MacPopUpButtonItem value="offset">使用固定 UTC 偏移</MacPopUpButtonItem></MacPopUpButton></div>
          <template v-if="settings.mode === 'zone'"><label>时区名称</label><div class="select-field"><MacPopUpButton v-model="settings.zoneId" size="large"><MacPopUpButtonItem v-for="zone in zones" :key="zone.id" :value="zone.id">{{ zone.label }}　·　{{ zone.id }}</MacPopUpButtonItem></MacPopUpButton></div></template>
          <template v-else><label>UTC 固定偏移</label><div class="select-field"><MacPopUpButton v-model="settings.offset" size="large"><MacPopUpButtonItem v-for="offset in offsets" :key="offset" :value="offset">{{ offsetText(offset * 60) }}</MacPopUpButtonItem></MacPopUpButton></div></template>
          <div class="offset-line"><span>当前 UTC 偏移</span><strong>{{ targetClock.offset }}</strong></div><p class="hint">地区时区自动处理夏令时；固定偏移不随季节变化。<br>半小时和四十五分钟时区请使用时区名称。</p>
        </MacGlassPanel>
        <MacGlassPanel class="preview-card" material="clear">
          <div class="section-heading"><MacLabel variant="title-2">启动预览</MacLabel><p>目标时间与本地时间实时对照</p></div>
          <article class="clock-card primary-clock"><span>设置时区</span><time>{{ targetClock.time }}</time><p>{{ targetClock.date }}</p><strong>{{ targetName }}</strong><small>{{ targetClock.offset }}　·　{{ settings.mode === 'zone' ? '自动适用地区时间规则' : '固定偏移' }}</small></article>
          <article class="clock-card"><span>电脑本地时区</span><time>{{ localClock.time }}</time><p>{{ localClock.date }}</p><strong>{{ localZone }}</strong><small>{{ localClock.offset }}　·　Windows 本地时区</small></article>
        </MacGlassPanel>
      </div>
      <MacGlassPanel class="path-card" material="regular"><div class="section-heading"><MacLabel variant="title-3">客户端</MacLabel><p>路径留空时自动查找当前安装版本，也可手动选择。</p></div><div class="path-row"><MacTextField v-model="settings.executable" size="large" :placeholder="effectivePath || '自动查找 Codex'" /><MacButton size="large" @click="browse">浏览…</MacButton></div></MacGlassPanel>
      <MacGlassPanel class="option-card" material="regular"><MacCheckbox v-model="settings.dreamSkinCompatible" size="regular">Dream Skin 兼容启动并自动应用皮肤</MacCheckbox><p>保持时区设置，并通过 Dream Skin 官方流程建立当前注入会话。</p></MacGlassPanel>
      <div class="status"><MacSpinner v-if="busy" size="small" /><span>{{ status }}</span></div><p class="running-tip">客户端已运行？请先保存工作并完全退出，再启动。</p>
      <footer class="actions"><MacButton size="large" :disabled="busy" @click="shortcut">创建桌面快捷方式</MacButton><MacButton size="large" :disabled="busy" @click="save">保存设置</MacButton><MacButton size="large" :disabled="busy" @click="launchSkin">启动 Dream Skin</MacButton><MacButton size="large" variant="prominent" :disabled="busy" @click="launch">保存并启动 Codex　→</MacButton></footer>
    </section>
  </main>
</template>
