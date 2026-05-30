import type { ZomboidMod } from "@/types/mod"

export function getActiveDependencyChain(
  mod: ZomboidMod,
  modsById: Map<string, ZomboidMod>,
  activeModIds: Set<string>,
) {
  const orderedModIds: string[] = []
  const visitingModIds = new Set<string>()
  const visitedModIds = new Set<string>()

  function visit(currentMod: ZomboidMod) {
    const currentModId = currentMod.id.toLowerCase()

    if (visitedModIds.has(currentModId) || visitingModIds.has(currentModId)) {
      return
    }

    visitingModIds.add(currentModId)

    for (const dependencyId of currentMod.dependencies ?? []) {
      const normalizedDependencyId = dependencyId.toLowerCase()

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

export function getWorkshopIdsForModIds(modIds: string[], mods: ZomboidMod[]) {
  const selectedModIds = new Set(modIds.map((modId) => modId.toLowerCase()))
  const seenWorkshopIds = new Set<string>()

  return modIds.flatMap((modId) => {
    const mod = mods.find((item) => item.id.toLowerCase() === modId.toLowerCase())
    const workshopId = mod?.workshopId?.trim()

    if (!workshopId) {
      return []
    }

    const normalizedWorkshopId = workshopId.toLowerCase()

    if (seenWorkshopIds.has(normalizedWorkshopId) || !selectedModIds.has(modId.toLowerCase())) {
      return []
    }

    seenWorkshopIds.add(normalizedWorkshopId)
    return [workshopId]
  })
}
