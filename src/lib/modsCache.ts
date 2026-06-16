import type { ZomboidMod } from "@/types/mod"

const MODS_LIBRARY_CACHE_KEY = "pzmm:mods-library"
const MODS_LIBRARY_CACHE_VERSION = 5
const MODS_LIBRARY_COMPATIBLE_CACHE_VERSIONS = new Set([MODS_LIBRARY_CACHE_VERSION])
const MODS_LIBRARY_CACHE_LIMIT = 30
const MOD_IMAGE_THUMBNAIL_WIDTH = 320
const MOD_IMAGE_THUMBNAIL_HEIGHT = 160

export type ModsLibraryCache = {
  version: number
  cachedAt: string
  totalModsCount: number
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
    typeof mod.packagePath === "string" &&
    isStringArray(mod.compatibleBuilds) &&
    Array.isArray(mod.variants) &&
    mod.variants.every((variant) => {
      if (!variant || typeof variant !== "object") return false
      const item = variant as Record<string, unknown>
      return typeof item.gameBuild === "string" && typeof item.id === "string" && typeof item.path === "string" &&
        isStringArray(item.dependencies) && isStringArray(item.mapNames)
    }) &&
    (mod.imageUrl === undefined || typeof mod.imageUrl === "string") &&
    (mod.dependencies === undefined || isStringArray(mod.dependencies)) &&
    (mod.mapNames === undefined || isStringArray(mod.mapNames))
  )
}

export function readModsLibraryCache(): ModsLibraryCache | null {
  try {
    const rawCache = window.localStorage.getItem(MODS_LIBRARY_CACHE_KEY)

    if (!rawCache) {
      return null
    }

    const cache = JSON.parse(rawCache) as Partial<ModsLibraryCache>

    if (
      !MODS_LIBRARY_COMPATIBLE_CACHE_VERSIONS.has(cache.version ?? 0) ||
      typeof cache.cachedAt !== "string" ||
      typeof cache.totalModsCount !== "number" ||
      !Array.isArray(cache.mods) ||
      cache.mods.length > MODS_LIBRARY_CACHE_LIMIT ||
      !cache.mods.every(isCachedMod)
    ) {
      window.localStorage.removeItem(MODS_LIBRARY_CACHE_KEY)
      return null
    }

    return cache as ModsLibraryCache
  } catch {
    return null
  }
}

function createImageThumbnail(imageUrl: string) {
  if (!imageUrl.startsWith("data:image/")) {
    return Promise.resolve(imageUrl)
  }

  return new Promise<string | undefined>((resolve) => {
    const image = new Image()

    image.onerror = () => resolve(undefined)
    image.onload = () => {
      const scale = Math.min(
        1,
        MOD_IMAGE_THUMBNAIL_WIDTH / image.width,
        MOD_IMAGE_THUMBNAIL_HEIGHT / image.height,
      )
      const width = Math.max(1, Math.round(image.width * scale))
      const height = Math.max(1, Math.round(image.height * scale))
      const canvas = document.createElement("canvas")
      const context = canvas.getContext("2d")

      if (!context) {
        resolve(undefined)
        return
      }

      try {
        canvas.width = width
        canvas.height = height
        context.drawImage(image, 0, 0, width, height)
        resolve(canvas.toDataURL("image/jpeg", 0.7))
      } catch {
        resolve(undefined)
      }
    }
    image.src = imageUrl
  })
}

async function compactCachedMods(mods: ZomboidMod[]) {
  return Promise.all(
    mods.slice(0, MODS_LIBRARY_CACHE_LIMIT).map(async (mod) => ({
      ...mod,
      imageUrl: mod.imageUrl ? await createImageThumbnail(mod.imageUrl) : undefined,
    })),
  )
}

function persistCache(cache: ModsLibraryCache) {
  try {
    window.localStorage.setItem(MODS_LIBRARY_CACHE_KEY, JSON.stringify(cache))
    return true
  } catch {
    return false
  }
}

export async function writeModsLibraryCache(mods: ZomboidMod[]) {
  try {
    const cache: ModsLibraryCache = {
      version: MODS_LIBRARY_CACHE_VERSION,
      cachedAt: new Date().toISOString(),
      totalModsCount: mods.length,
      mods: await compactCachedMods(mods),
    }

    if (persistCache(cache)) {
      return
    }

    // Preserve the instant first page even when the WebView exposes an unusually small quota.
    for (let index = cache.mods.length - 1; index >= 0; index -= 1) {
      cache.mods[index] = { ...cache.mods[index], imageUrl: undefined }

      if (persistCache(cache)) {
        return
      }
    }
  } catch {
    // The live backend scan remains the source of truth if browser storage is unavailable or full.
  }
}

export function clearModsLibraryCache() {
  try {
    window.localStorage.removeItem(MODS_LIBRARY_CACHE_KEY)
  } catch {
    // The backend scan remains the source of truth if browser storage is unavailable.
  }
}
