import type { ZomboidServer } from "@/types/server"

const SERVERS_CACHE_KEY = "pzmm:servers"
const SERVERS_CACHE_VERSION = 1

export type ServersCache = {
  version: number
  cachedAt: string
  servers: ZomboidServer[]
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
}

function isCachedServer(value: unknown): value is ZomboidServer {
  if (!value || typeof value !== "object") {
    return false
  }

  const server = value as Record<string, unknown>

  return (
    typeof server.id === "string" &&
    typeof server.name === "string" &&
    typeof server.fileName === "string" &&
    typeof server.path === "string" &&
    typeof server.port === "string" &&
    typeof server.maxPlayers === "number" &&
    typeof server.modsCount === "number" &&
    isStringArray(server.activeModIds) &&
    (server.status === "online" || server.status === "offline") &&
    (server.gameBuild === "b41" || server.gameBuild === "b42")
  )
}

function resolveServersCacheKey(cacheKey = SERVERS_CACHE_KEY) {
  return cacheKey
}

export function readServersCache(cacheKey = SERVERS_CACHE_KEY): ServersCache | null {
  try {
    const key = resolveServersCacheKey(cacheKey)
    const rawCache = window.localStorage.getItem(key)

    if (!rawCache) {
      return null
    }

    const cache = JSON.parse(rawCache) as Partial<ServersCache>

    if (
      cache.version !== SERVERS_CACHE_VERSION ||
      typeof cache.cachedAt !== "string" ||
      !Array.isArray(cache.servers) ||
      !cache.servers.every(isCachedServer)
    ) {
      window.localStorage.removeItem(key)
      return null
    }

    return cache as ServersCache
  } catch {
    return null
  }
}

export function writeServersCache(servers: ZomboidServer[], cacheKey = SERVERS_CACHE_KEY) {
  try {
    const sortedServers = [...servers].sort((left, right) =>
      left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
    )

    window.localStorage.setItem(resolveServersCacheKey(cacheKey), JSON.stringify({
      version: SERVERS_CACHE_VERSION,
      cachedAt: new Date().toISOString(),
      servers: sortedServers,
    }))
  } catch {
    // The backend list remains the source of truth if browser storage is unavailable.
  }
}

export function clearServersCache(cacheKey = SERVERS_CACHE_KEY) {
  try {
    window.localStorage.removeItem(resolveServersCacheKey(cacheKey))
  } catch {
    // The backend list remains the source of truth if browser storage is unavailable.
  }
}
