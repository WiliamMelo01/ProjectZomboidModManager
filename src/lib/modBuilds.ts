import type { ZomboidMod } from "@/types/mod"
import type { GameBuild } from "@/types/server"

export function getModVariant(mod: ZomboidMod, gameBuild: GameBuild) {
  return mod.variants.find((variant) => variant.gameBuild === gameBuild)
}

export function supportsBuild(mod: ZomboidMod, gameBuild: GameBuild) {
  return Boolean(getModVariant(mod, gameBuild))
}

export function resolveModForBuild(mod: ZomboidMod, gameBuild: GameBuild): ZomboidMod | null {
  const variant = getModVariant(mod, gameBuild)
  if (!variant) return null

  return {
    ...mod,
    id: variant.id,
    path: variant.path,
    dependencies: variant.dependencies,
    mapNames: variant.mapNames,
  }
}

export function findModForServerId(mods: ZomboidMod[], modId: string, gameBuild: GameBuild) {
  const normalizedId = normalizeModId(modId)
  return mods
    .map((mod) => resolveModForBuild(mod, gameBuild))
    .find((mod): mod is ZomboidMod => Boolean(mod) && normalizeModId(mod.id) === normalizedId)
}

function normalizeModId(modId: string) {
  return modId.trim().replace(/^\\+/, "").toLowerCase()
}
