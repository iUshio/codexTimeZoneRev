export type AppearanceMode = 'system' | 'light' | 'dark'
export type ResolvedAppearance = 'light' | 'dark'
export type LauncherCommand = 'save' | 'launch' | 'create_shortcut' | 'launch_dream_skin'
export type MessageKind = 'info' | 'success' | 'error' | 'pending'

export type Zone = {
  label: string
  id: string
  windowsId: string
}

export type Settings = {
  mode: 'zone' | 'offset'
  zoneId: string
  offset: number
  executable: string
  dreamSkinCompatible: boolean
}

export type Bootstrap = {
  settings: Settings
  zones: Zone[]
  localZone: string
  detected: string
  platform?: string
  settingsPath?: string
}

export type IpInfo = {
  ip: string
  timezone: string
  city?: string
  region?: string
  country?: string
  countryCode?: string
  organization?: string
  source: string
}

export type NetworkInfo = {
  domestic: IpInfo | null
  international: IpInfo | null
  errors?: string[]
}

export type UiMessage = {
  kind: MessageKind
  summary: string
  detail?: string
}

export type ClockValue = {
  time: string
  date: string
  offset: string
}
