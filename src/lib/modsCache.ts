import type { ZomboidMod } from "@/types/mod"

const MODS_LIBRARY_CACHE_KEY = "pzmm:mods-library"
const MODS_LIBRARY_CACHE_VERSION = 1
const MODS_IMAGES_DATABASE_NAME = "pzmm-mods-cache"
const MODS_IMAGES_DATABASE_VERSION = 1
const MODS_IMAGES_STORE_NAME = "images"

type ModsLibraryCache = {
  version: number
  cachedAt: string
  mods: ZomboidMod[]
}

type CachedModImage = {
  key: string
  imageUrl: string
}

function getModCacheKey(mod: ZomboidMod) {
  return `${mod.source}:${mod.workshopId}:${mod.id}:${mod.path}`.toLowerCase()
}

function openModsImagesDatabase() {
  return new Promise<IDBDatabase>((resolve, reject) => {
    const request = window.indexedDB.open(MODS_IMAGES_DATABASE_NAME, MODS_IMAGES_DATABASE_VERSION)

    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve(request.result)
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(MODS_IMAGES_STORE_NAME)) {
        request.result.createObjectStore(MODS_IMAGES_STORE_NAME, { keyPath: "key" })
      }
    }
  })
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

export async function hydrateModsLibraryImages(mods: ZomboidMod[]) {
  if (mods.length === 0 || !window.indexedDB) {
    return mods
  }

  try {
    const database = await openModsImagesDatabase()
    const transaction = database.transaction(MODS_IMAGES_STORE_NAME, "readonly")
    const store = transaction.objectStore(MODS_IMAGES_STORE_NAME)
    const hydratedMods = await Promise.all(
      mods.map(
        (mod) =>
          new Promise<ZomboidMod>((resolve) => {
            const request = store.get(getModCacheKey(mod))

            request.onerror = () => resolve(mod)
            request.onsuccess = () => {
              const cachedImage = request.result as CachedModImage | undefined
              resolve(cachedImage?.imageUrl ? { ...mod, imageUrl: cachedImage.imageUrl } : mod)
            }
          }),
      ),
    )

    database.close()
    return hydratedMods
  } catch {
    return mods
  }
}

export async function writeModsLibraryImagesCache(mods: ZomboidMod[]) {
  if (!window.indexedDB) {
    return
  }

  try {
    const database = await openModsImagesDatabase()
    const transaction = database.transaction(MODS_IMAGES_STORE_NAME, "readwrite")
    const store = transaction.objectStore(MODS_IMAGES_STORE_NAME)

    store.clear()

    for (const mod of mods) {
      if (mod.imageUrl) {
        store.put({ key: getModCacheKey(mod), imageUrl: mod.imageUrl } satisfies CachedModImage)
      }
    }

    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve()
      transaction.onerror = () => reject(transaction.error)
      transaction.onabort = () => reject(transaction.error)
    })
    database.close()
  } catch {
    // The next live backend scan can repopulate images if IndexedDB is unavailable.
  }
}
