export type ZomboidServer = {
  id: string
  name: string
  fileName: string
  path: string
  port: string
  maxPlayers: number
  modsCount: number
  activeModIds: string[]
  status: "online" | "offline"
  gameBuild: GameBuild
}

export type GameBuild = "b41" | "b42"

export type ServerIniSettings = {
  publicName: string
  publicDescription: string
  password: string
  maxPlayers: number
  defaultPort: string
  udpPort: string
  isPublic: boolean
  isOpen: boolean
  pvp: boolean
  pauseEmpty: boolean
  globalChat: boolean
  displayUserName: boolean
  safetySystem: boolean
  voiceEnable: boolean
  steamVac: boolean
  upnp: boolean
  pingLimit: number
  saveWorldEveryMinutes: number
  hoursForLootRespawn: number
  playerSafehouse: boolean
  adminSafehouse: boolean
  backupsCount: number
  backupsOnStart: boolean
  backupsPeriod: number
}

export type ServerLuaSetting = {
  path: string
  key: string
  section: string
  value: string
  valueKind: "number" | "boolean" | "string"
  defaultValue?: string | null
  options: ServerLuaSettingOption[]
}

export type ServerLuaSettings = {
  fileName: string
  settings: ServerLuaSetting[]
}

export type ServerLuaSettingOption = {
  value: string
  label: string
}
