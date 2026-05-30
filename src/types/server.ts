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
}
