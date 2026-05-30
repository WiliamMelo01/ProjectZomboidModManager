import type { ZomboidMod } from "@/types/mod"

const MODS_LIBRARY_CACHE_KEY = "pzmm:mods-library"
const MODS_LIBRARY_CACHE_VERSION = 1

type ModsLibraryCache = {
  version: number
  cachedAt: string
  mods: ZomboidMod[]
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
}

function isCachedMod(value: unknown): value is ZomboidMod {
  if (!value || typeof value !== "object") {
    return false
  }

  const mod = value as Record<string, unknown>

  return (
    typeof mod.id === "string" &&
    typeof mod.name === "string" &&
    typeof mod.author === "string" &&
    typeof mod.version === "string" &&
    typeof mod.workshopId === "string" &&
    typeof mod.description === "string" &&
    typeof mod.size === "string" &&
    typeof mod.isInstalled === "boolean" &&
    typeof mod.source === "string" &&
    typeof mod.path === "string" &&
    (mod.dependencies === undefined || isStringArray(mod.dependencies))
  )
}

export function readModsLibraryCache(): ZomboidMod[] | null {
  try {
    const rawCache = window.localStorage.getItem(MODS_LIBRARY_CACHE_KEY)

    if (!rawCache) {
      return null
    }

    const cache = JSON.parse(rawCache) as Partial<ModsLibraryCache>

    if (
      cache.version !== MODS_LIBRARY_CACHE_VERSION ||
      !Array.isArray(cache.mods) ||
      !cache.mods.every(isCachedMod)
    ) {
      window.localStorage.removeItem(MODS_LIBRARY_CACHE_KEY)
      return null
    }

    return cache.mods
  } catch {
    return null
  }
}

export function writeModsLibraryCache(mods: ZomboidMod[]) {
  try {
    const lightweightMods = mods.map(({ imageUrl: _imageUrl, ...mod }) => mod)
    const cache: ModsLibraryCache = {
      version: MODS_LIBRARY_CACHE_VERSION,
      cachedAt: new Date().toISOString(),
      mods: lightweightMods,
    }

    window.localStorage.setItem(MODS_LIBRARY_CACHE_KEY, JSON.stringify(cache))
  } catch {
    // The live backend scan remains the source of truth if browser storage is unavailable or full.
  }
}
