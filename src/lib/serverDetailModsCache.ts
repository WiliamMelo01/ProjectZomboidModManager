import type { ZomboidMod } from "@/types/mod"

const SERVER_DETAIL_MODS_CACHE_VERSION = 1
const SERVER_DETAIL_MODS_CACHE_LIMIT = 160

type ServerDetailModsCache = {
  version: number
  cachedAt: string
  mods: ZomboidMod[]
}

function key(cacheKey: string) {
  return `pzmm:server-detail-mods:${cacheKey}`
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
}

function isCachedMod(value: unknown): value is ZomboidMod {
  if (!value || typeof value !== "object") return false
  const mod = value as Record<string, unknown>

  return typeof mod.id === "string" &&
    typeof mod.name === "string" &&
    typeof mod.workshopId === "string" &&
    typeof mod.source === "string" &&
    typeof mod.path === "string" &&
    typeof mod.packagePath === "string" &&
    isStringArray(mod.compatibleBuilds) &&
    Array.isArray(mod.variants)
}

export function readServerDetailModsCache(cacheKey: string): ZomboidMod[] {
  try {
    const raw = window.localStorage.getItem(key(cacheKey))
    if (!raw) return []

    const cache = JSON.parse(raw) as Partial<ServerDetailModsCache>
    if (cache.version !== SERVER_DETAIL_MODS_CACHE_VERSION || !Array.isArray(cache.mods) || !cache.mods.every(isCachedMod)) {
      window.localStorage.removeItem(key(cacheKey))
      return []
    }

    return cache.mods as ZomboidMod[]
  } catch {
    return []
  }
}

export function writeServerDetailModsCache(cacheKey: string, mods: ZomboidMod[]) {
  try {
    const cache: ServerDetailModsCache = {
      version: SERVER_DETAIL_MODS_CACHE_VERSION,
      cachedAt: new Date().toISOString(),
      mods: mods.slice(0, SERVER_DETAIL_MODS_CACHE_LIMIT),
    }
    window.localStorage.setItem(key(cacheKey), JSON.stringify(cache))
  } catch {
    // Cache is an optimization only.
  }
}
