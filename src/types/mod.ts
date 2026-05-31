export type ZomboidMod = {
  id: string
  name: string
  author: string
  version: string
  workshopId: string
  description: string
  size: string
  isInstalled: boolean
  source: "local" | "steam" | string
  path: string
  imageUrl?: string
  dependencies?: string[]
  mapNames?: string[]
}
