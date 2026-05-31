import type { ZomboidMod } from "@/types/mod"

export function normalizeModId(modId: string) {
  return String(modId ?? "").trim().replace(/^\\+/, "").toLowerCase()
}

export function isLocalMod(mod: ZomboidMod) {
  return mod.isInstalled || mod.source === "local"
}

export function buildInstallDependencyPlan(mod: ZomboidMod, allMods: ZomboidMod[]) {
  const modsById = mapModsById(allMods)
  const dependenciesToInstall: ZomboidMod[] = []
  const installIds = new Set<string>()
  let missingDependencyId: string | null = null

  visitModDependencies(mod, modsById, (dependency, dependencyId) => {
    if (!isLocalMod(dependency) && !installIds.has(dependencyId)) {
      dependenciesToInstall.push(dependency)
      installIds.add(dependencyId)
    }
  }, (dependencyId) => {
    missingDependencyId = dependencyId
  })

  return {
    missingDependencyId,
    dependenciesToInstall,
  }
}

export function buildActivationDependencyPlan(
  mod: ZomboidMod,
  allMods: ZomboidMod[],
  activeModIds: Set<string>,
) {
  const modsById = mapModsById(allMods)
  const dependenciesToInstall: ZomboidMod[] = []
  const dependenciesToActivate: ZomboidMod[] = []
  const installIds = new Set<string>()
  const activateIds = new Set<string>()
  let missingDependencyId: string | null = null

  visitModDependencies(mod, modsById, (dependency, dependencyId) => {
    if (!isLocalMod(dependency) && !installIds.has(dependencyId)) {
      dependenciesToInstall.push(dependency)
      installIds.add(dependencyId)
    }

    if (!activeModIds.has(dependencyId) && !activateIds.has(dependencyId)) {
      dependenciesToActivate.push(dependency)
      activateIds.add(dependencyId)
    }
  }, (dependencyId) => {
    missingDependencyId = dependencyId
  })

  return {
    missingDependencyId,
    dependenciesToInstall,
    dependenciesToActivate,
  }
}

function mapModsById(mods: ZomboidMod[]) {
  return new Map(
    mods.flatMap((mod) => [
      [normalizeModId(mod.id), mod] as const,
      ...mod.variants.map((variant) => [
        normalizeModId(variant.id),
        { ...mod, id: variant.id, path: variant.path, dependencies: variant.dependencies, mapNames: variant.mapNames },
      ] as const),
    ]),
  )
}

function visitModDependencies(
  mod: ZomboidMod,
  modsById: Map<string, ZomboidMod>,
  onDependency: (dependency: ZomboidMod, dependencyId: string) => void,
  onMissingDependency: (dependencyId: string) => void,
) {
  const visitingIds = new Set<string>()
  const visitedIds = new Set<string>()
  let missingDependencyFound = false

  const visit = (currentMod: ZomboidMod) => {
    const currentId = normalizeModId(currentMod.id)

    if (visitedIds.has(currentId) || visitingIds.has(currentId) || missingDependencyFound) {
      return
    }

    visitingIds.add(currentId)

    for (const dependencyId of currentMod.dependencies ?? []) {
      const normalizedDependencyId = normalizeModId(dependencyId)
      const dependency = modsById.get(normalizedDependencyId)

      if (!dependency) {
        missingDependencyFound = true
        onMissingDependency(dependencyId)
        break
      }

      visit(dependency)

      if (missingDependencyFound) {
        break
      }

      onDependency(dependency, normalizedDependencyId)
    }

    visitingIds.delete(currentId)
    visitedIds.add(currentId)
  }

  visit(mod)
}
