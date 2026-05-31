import { useRef, useState } from "react"

import { getErrorMessage } from "@/lib/errors"
import { readModsLibraryCache, writeModsLibraryCache } from "@/lib/modsCache"
import { invokeTauri } from "@/lib/tauri"
import type { ZomboidMod } from "@/types/mod"

export function useModsLibrary() {
  const [cachedMods] = useState(readModsLibraryCache)
  const [mods, setMods] = useState<ZomboidMod[]>(cachedMods?.mods ?? [])
  const [modsCount, setModsCount] = useState(cachedMods?.totalModsCount ?? 0)
  const [modsError, setModsError] = useState<string | null>(null)
  const [isLoadingMods, setIsLoadingMods] = useState(false)
  const [isInstallingAllMods, setIsInstallingAllMods] = useState(false)
  const [hasLoadedMods, setHasLoadedMods] = useState(cachedMods !== null)
  const modsLoadPromiseRef = useRef<Promise<ZomboidMod[]> | null>(null)

  async function loadMods() {
    if (modsLoadPromiseRef.current) {
      return modsLoadPromiseRef.current
    }

    const loadPromise = (async () => {
      setIsLoadingMods(true)
      setModsError(null)

      try {
        const foundMods = await invokeTauri<ZomboidMod[]>("list_zomboid_mods")
        setMods(foundMods)
        setModsCount(foundMods.length)
        setHasLoadedMods(true)
        writeModsLibraryCache(foundMods)
        return foundMods
      } catch (error) {
        setModsError(getErrorMessage(error))
        return []
      } finally {
        setIsLoadingMods(false)
        modsLoadPromiseRef.current = null
      }
    })()

    modsLoadPromiseRef.current = loadPromise
    return loadPromise
  }

  async function ensureModsLoaded() {
    if (!hasLoadedMods && !isLoadingMods) {
      await loadMods()
    }
  }

  async function installMods(modsToInstall: ZomboidMod[]) {
    setModsError(null)

    try {
      const modsToMove = modsToInstall.filter((mod) => !mod.isInstalled && mod.source !== "local")

      for (const mod of modsToMove) {
        await invokeTauri<void>("install_zomboid_mod", {
          modPath: mod.path,
          modId: mod.id,
          workshopId: mod.workshopId,
        })
      }
      const installedModIds = new Set(modsToMove.map((mod) => mod.id.toLowerCase()))

      setMods((currentMods) => {
        const updatedMods = currentMods.map((mod) =>
          installedModIds.has(mod.id.toLowerCase())
            ? { ...mod, isInstalled: true, source: mod.source === "steam" ? "local" : mod.source }
            : mod,
        )

        writeModsLibraryCache(updatedMods)
        return updatedMods
      })
    } catch (error) {
      setModsError(getErrorMessage(error))
      throw error
    }
  }

  async function installAllUninstalledMods() {
    if (isInstallingAllMods) {
      return
    }

    setIsInstallingAllMods(true)

    try {
      const availableMods = hasLoadedMods ? mods : await loadMods()
      const modsToInstall = availableMods.filter((mod) => !mod.isInstalled && mod.source !== "local")

      if (modsToInstall.length > 0) {
        await installMods(modsToInstall)
      }
    } finally {
      setIsInstallingAllMods(false)
    }
  }

  async function loadModsInBackground() {
    await loadMods()
  }

  return {
    mods,
    modsCount,
    modsError,
    isLoadingMods,
    isInstallingAllMods,
    hasLoadedMods,
    loadMods,
    ensureModsLoaded,
    installMods,
    installAllUninstalledMods,
    loadModsInBackground,
  }
}
