export type AppSettings = {
  steamcmdPath: string
  resolvedSteamcmdPath: string | null
  isSteamcmdConfigured: boolean
  gameExecutablePath: string
  clientRam: string
  serverRam: string
}

export type ModLocation = {
  label: string
  path: string
  kind: string
  exists: boolean
}

export type ZomboidInstallationStatus = {
  defaultGameDir: string
  detectedExecutablePath: string | null
  isGameDirFound: boolean
  isExecutableFound: boolean
  isClientConfigFound: boolean
  isServerConfigFound: boolean
}
