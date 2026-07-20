import type { ZomboidMod } from "@/types/mod"
import type { GameBuild } from "@/types/server"
import { findModForServerId } from "@/lib/modBuilds"

function normalizeModId(modId: string) {
  return modId.trim().replace(/^\\+/, "").toLowerCase()
}

export function getActiveDependencyChain(
  mod: ZomboidMod,
  modsById: Map<string, ZomboidMod>,
  activeModIds: Set<string>,
) {
  const orderedModIds: string[] = []
  const visitingModIds = new Set<string>()
  const visitedModIds = new Set<string>()

  function visit(currentMod: ZomboidMod) {
    const currentModId = normalizeModId(currentMod.id)

    if (visitedModIds.has(currentModId) || visitingModIds.has(currentModId)) {
      return
    }

    visitingModIds.add(currentModId)

    for (const dependencyId of currentMod.dependencies ?? []) {
      const normalizedDependencyId = normalizeModId(dependencyId)

      if (!activeModIds.has(normalizedDependencyId)) {
        continue
      }

      const dependency = modsById.get(normalizedDependencyId)

      if (dependency) {
        visit(dependency)
      }
    }

    visitingModIds.delete(currentModId)
    visitedModIds.add(currentModId)

    if (activeModIds.has(currentModId)) {
      orderedModIds.push(currentMod.id)
    }
  }

  visit(mod)
  return orderedModIds
}

export function getWorkshopIdsForModIds(
  modIds: string[],
  mods: ZomboidMod[],
  gameBuild: GameBuild,
  workshopMappings?: Record<string, string>
) {
  const selectedModIds = new Set(modIds.map(normalizeModId))
  const seenWorkshopIds = new Set<string>()

  return modIds.flatMap((modId) => {
    const mod = findModForServerId(mods, modId, gameBuild)
    let workshopId = mod?.workshopId?.trim()

    if (!workshopId && workshopMappings) {
      workshopId = workshopMappings[modId] || workshopMappings[modId.toLowerCase()]
    }

    if (!workshopId) {
      return []
    }

    const normalizedWorkshopId = workshopId.toLowerCase()

    if (seenWorkshopIds.has(normalizedWorkshopId) || !selectedModIds.has(normalizeModId(modId))) {
      return []
    }

    seenWorkshopIds.add(normalizedWorkshopId)
    return [workshopId]
  })
}

export function isModInCloud(mod: ZomboidMod, workshopMappings?: Record<string, string>): boolean {
  if (!workshopMappings) return false
  if (workshopMappings[mod.id] || workshopMappings[mod.id.toLowerCase()]) {
    return true
  }
  const cleanWsId = mod.workshopId?.trim()
  if (cleanWsId && /^\d+$/.test(cleanWsId)) {
    return Object.values(workshopMappings).includes(cleanWsId)
  }
  return false
}
