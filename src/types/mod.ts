import type { GameBuild } from "@/types/server"

export type ZomboidModVariant = {
  gameBuild: GameBuild
  id: string
  path: string
  dependencies: string[]
  mapNames: string[]
}

export type ZomboidMod = {
  id: string
  name: string
  author: string
  version: string
  workshopId: string
  description: string
  size: string
  isInstalled: boolean
  source: "local" | "steam" | "steamcmd" | string
  path: string
  imageUrl?: string
  dependencies?: string[]
  mapNames?: string[]
  compatibleBuilds: GameBuild[]
  variants: ZomboidModVariant[]
  packagePath: string
}
