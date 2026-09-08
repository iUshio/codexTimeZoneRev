import { onBeforeUnmount, onMounted, ref } from 'vue'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import type { IpInfo, NetworkInfo } from '../types'

const networkCacheKey = 'codex-timezone-network-v1'
type RegionKey = 'domestic' | 'international'
type IpProvider = { name: string; url: string; parse: (text: string) => string | undefined }

const textIp = (text: string) => text.trim().match(/^(?:\(?\s*)?(?:ip\s*[=:]\s*)?([0-9a-f:.]+)(?:\s*\)?)$/i)?.[1]
const jsonIp = (text: string) => {
  try {
    const value = JSON.parse(text.replace(/^\s*\(/, '').replace(/\)\s*$/, '')) as Record<string, unknown>
    return typeof value.ip === 'string' ? value.ip : undefined
  } catch { return undefined }
}
const traceIp = (text: string) => text.split(/\r?\n/).find((line) => line.startsWith('ip='))?.slice(3).trim()

const domesticProviders: IpProvider[] = [
  { name: '腾讯', url: 'https://r.inews.qq.com/api/ip2city?otype=jsonp', parse: jsonIp },
  { name: 'IPIP.net', url: 'https://myip.ipip.net/json', parse: jsonIp },
  { name: '又拍云', url: `https://pubstatic.b0.upaiyun.com/?_upnode&_t=${Date.now()}`, parse: textIp },
  { name: 'Cloudflare CN', url: 'https://www.cloudflare-cn.com/cdn-cgi/trace', parse: traceIp },
]
const internationalProviders: IpProvider[] = [
  { name: 'AWS', url: 'https://checkip.amazonaws.com/', parse: textIp },
  { name: 'Cloudflare Workers', url: 'https://workers.dev/cdn-cgi/trace', parse: traceIp },
  { name: 'Cloudflare US IPv4', url: 'https://1.0.0.1/cdn-cgi/trace', parse: traceIp },
  { name: 'IPify IPv4', url: 'https://api4.ipify.org?format=json', parse: jsonIp },
  { name: 'IP.SB', url: 'https://api.ip.sb/geoip', parse: jsonIp },
]

async function fetchProvider(provider: IpProvider, controller: AbortController) {
  const timer = window.setTimeout(() => controller.abort(), 9000)
  try {
    const response = await fetch(provider.url, { cache: 'no-store', signal: controller.signal })
    if (!response.ok) throw new Error(`${provider.name} 返回 HTTP ${response.status}`)
    const ip = provider.parse(await response.text())
    if (!ip) throw new Error(`${provider.name} 未返回有效 IP`)
    return { ip, source: provider.name }
  } finally { window.clearTimeout(timer) }
}

async function fetchGeo(ip: string, source: string, controller: AbortController): Promise<IpInfo> {
  const timer = window.setTimeout(() => controller.abort(), 9000)
  try {
    const response = await fetch(`https://ip125.com/api/geo/${encodeURIComponent(ip)}`, { cache: 'no-store', signal: controller.signal })
    if (!response.ok) throw new Error(`IP125 归属地服务返回 HTTP ${response.status}`)
    const value = await response.json() as Record<string, unknown>
    const timezone = typeof value.timezone === 'string' ? value.timezone : ''
    if (value.status !== 'success' || !timezone) throw new Error('IP125 未返回完整归属地')
    return {
      ip,
      timezone,
      city: typeof value.city === 'string' ? value.city : undefined,
      region: typeof value.regionName === 'string' ? value.regionName : undefined,
      country: typeof value.country === 'string' ? value.country : undefined,
      countryCode: typeof value.countryCode === 'string' ? value.countryCode : undefined,
      organization: typeof value.org === 'string' ? value.org : (typeof value.isp === 'string' ? value.isp : undefined),
      source,
    }
  } finally { window.clearTimeout(timer) }
}

async function queryRegion(providers: IpProvider[]) {
  const controllers = providers.map(() => new AbortController())
  const tasks = providers.map(async (provider, index) => {
    const candidate = await fetchProvider(provider, controllers[index])
    return fetchGeo(candidate.ip, candidate.source, controllers[index])
  })
  try {
    return await new Promise<IpInfo>((resolve, reject) => {
      let failures = 0
      tasks.forEach((task) => task.then(resolve).catch(() => {
        failures += 1
        if (failures === tasks.length) reject(new Error('所有 IP 查询源均不可用'))
      }))
    })
  } finally { controllers.forEach((controller) => controller.abort()) }
}

function isValidNetwork(value: NetworkInfo) {
  const valid = (item: IpInfo | null) => !item || (
    typeof item.ip === 'string' && item.ip.length > 0 &&
    typeof item.timezone === 'string' && item.timezone.length > 0
  )
  return 'domestic' in value && 'international' in value && valid(value.domestic) && valid(value.international)
}

export function useNetworkInfo() {
  const previewMode = import.meta.env.DEV && new URLSearchParams(window.location.search).get('ui-preview') === '1'
  const network = ref<NetworkInfo | null>(null)
  const networkBusy = ref(false)
  const hasCachedNetwork = ref(false)
  const copyFeedback = ref<Record<RegionKey, 'idle' | 'copied' | 'error'>>({ domestic: 'idle', international: 'idle' })
  const copyErrors = ref<Partial<Record<RegionKey, string>>>({})
  const copyTimers: Partial<Record<RegionKey, number>> = {}

  function readNetworkCache() {
    const raw = localStorage.getItem(networkCacheKey)
    if (!raw) return
    try {
      const value = JSON.parse(raw) as NetworkInfo
      if (!value || !isValidNetwork(value)) throw new Error('invalid cache')
      network.value = value
      hasCachedNetwork.value = true
    } catch {
      localStorage.removeItem(networkCacheKey)
    }
  }

  async function loadNetwork(silent = false) {
    if (networkBusy.value) return network.value
    if (previewMode) return network.value
    networkBusy.value = true
    try {
      const results = await Promise.allSettled([queryRegion(domesticProviders), queryRegion(internationalProviders)])
      const errors: string[] = []
      const value: NetworkInfo = { domestic: null, international: null, errors }
      results.forEach((result, index) => {
        const key: RegionKey = index === 0 ? 'domestic' : 'international'
        if (result.status === 'fulfilled') value[key] = result.value
        else errors.push(result.reason instanceof Error ? result.reason.message : String(result.reason))
      })
      network.value = value
      hasCachedNetwork.value = false
      localStorage.setItem(networkCacheKey, JSON.stringify(value))
      return value
    } catch {
      if (!silent) network.value = { domestic: null, international: null, errors: ['网络查询失败'] }
      return network.value
    } finally {
      networkBusy.value = false
    }
  }

  function refreshNetwork() {
    return loadNetwork(false)
  }

  async function copyIp(key: RegionKey, ip: string) {
    window.clearTimeout(copyTimers[key])
    try {
      await writeText(ip)
      if (await readText() !== ip) throw new Error('系统剪贴板内容校验失败')
      copyFeedback.value = { ...copyFeedback.value, [key]: 'copied' }
      copyErrors.value = { ...copyErrors.value, [key]: undefined }
    } catch (error) {
      copyFeedback.value = { ...copyFeedback.value, [key]: 'error' }
      copyErrors.value = { ...copyErrors.value, [key]: String(error) }
    }
    copyTimers[key] = window.setTimeout(() => {
      copyFeedback.value = { ...copyFeedback.value, [key]: 'idle' }
    }, 1500)
  }

  onMounted(() => {
    if (previewMode) {
      const state = new URLSearchParams(window.location.search).get('ui-network')
      network.value = state === 'loading' ? null : {
        domestic: {
          ip: '192.0.2.10',
          timezone: 'Asia/Shanghai',
          city: '北京',
          region: '北京',
          country: '中国',
          organization: '示例网络组织（演示数据）',
          source: '演示数据',
        },
        international: state === 'partial' ? null : {
          ip: '203.0.113.42',
          timezone: 'America/Los_Angeles',
          city: '洛杉矶',
          region: '加利福尼亚州',
          country: '美国',
          organization: 'Example Network Services with a deliberately long organization name',
          source: '演示数据',
        },
        errors: state === 'partial' ? ['国际出口演示失败'] : [],
      }
      networkBusy.value = state === 'loading'
      return
    }
    readNetworkCache()
    void loadNetwork(true)
  })
  onBeforeUnmount(() => Object.values(copyTimers).forEach((timer) => window.clearTimeout(timer)))

  return { network, networkBusy, hasCachedNetwork, copyFeedback, copyErrors, refreshNetwork, copyIp }
}
