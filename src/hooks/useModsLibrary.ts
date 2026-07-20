import { useRef, useState } from "react"

import { getErrorMessage } from "@/lib/errors"
import { readModsLibraryCache, writeModsLibraryCache } from "@/lib/modsCache"
import { invokeTauri } from "@/lib/tauri"
import type { ZomboidMod, ZomboidModInstallResult } from "@/types/mod"

type UseModsLibraryOptions = {
  listCommand?: string
  listArgs?: Record<string, unknown>
  installCommand?: string
  installArgs?: Record<string, unknown>
  clearCacheCommand?: string
  clearCacheArgs?: Record<string, unknown>
  reloadAfterInstall?: boolean
  backgroundReloadAfterInstall?: boolean
  useCache?: boolean
  cacheKey?: string
}

export function useModsLibrary({
  listCommand = "list_zomboid_mods",
  listArgs,
  installCommand = "install_zomboid_mod",
  installArgs,
  clearCacheCommand,
  clearCacheArgs,
  reloadAfterInstall = false,
  backgroundReloadAfterInstall = false,
  useCache = true,
  cacheKey,
}: UseModsLibraryOptions = {}) {
  const [cachedMods] = useState(() => useCache ? readModsLibraryCache(cacheKey) : null)
  const [mods, setMods] = useState<ZomboidMod[]>(cachedMods?.mods ?? [])
  const [modsCount, setModsCount] = useState(cachedMods?.totalModsCount ?? 0)
  const [modsError, setModsError] = useState<string | null>(null)
  const [isLoadingMods, setIsLoadingMods] = useState(false)
  const [isInstallingAllMods, setIsInstallingAllMods] = useState(false)
  const [hasLoadedMods, setHasLoadedMods] = useState(false)
  const modsLoadPromiseRef = useRef<Promise<ZomboidMod[]> | null>(null)

  async function loadMods() {
    if (modsLoadPromiseRef.current) {
      return modsLoadPromiseRef.current
    }

    const loadPromise = (async () => {
      setIsLoadingMods(true)
      setModsError(null)

      try {
        const foundMods = await invokeTauri<ZomboidMod[]>(listCommand, listArgs)
        setMods(foundMods)
        setModsCount(foundMods.length)
        setHasLoadedMods(true)
        if (useCache) {
          void writeModsLibraryCache(foundMods, cacheKey)
        }
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
      const installResults = new Map<string, ZomboidModInstallResult>()

      for (const mod of modsToMove) {
        const result = await invokeTauri<ZomboidModInstallResult | null>(installCommand, {
          ...(installArgs ?? {}),
          packagePath: mod.packagePath,
          modId: mod.id,
          workshopId: mod.workshopId,
        })

        if (result) {
          installResults.set(mod.id.toLowerCase(), result)
        }
      }

      // Envia os mapeamentos descobertos para a API central de forma assíncrona
      const mappingsToSend = modsToMove
        .filter((mod) => mod.id && mod.workshopId && mod.workshopId.trim() !== "")
        .map((mod) => ({
          modId: mod.id,
          workshopId: mod.workshopId.trim(),
        }))

      if (mappingsToSend.length > 0) {
        // Salva os mapeamentos localmente no banco de dados local para consistência offline
        for (const mapping of mappingsToSend) {
          invokeTauri("save_workshop_mapping", { modId: mapping.modId, workshopId: mapping.workshopId })
            .catch((err) => console.error("Falha ao salvar mapeamento local pós-instalação:", err));
        }

        fetch("http://52.67.72.177:8080/mappings/bulk", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ mappings: mappingsToSend }),
        }).catch((err) => {
          console.error("Erro ao reportar mapeamentos para a API:", err)
        })
      }

      if (clearCacheCommand) {
        await invokeTauri<void>(clearCacheCommand, clearCacheArgs)
      }

      const installedModIds = new Set(modsToMove.map((mod) => mod.id.toLowerCase()))
      const installedMods = modsToInstall.map((mod) => {
        const installKey = mod.id.toLowerCase()
        const installResult = installResults.get(installKey)

        if (!installedModIds.has(installKey)) {
          return mod
        }

        return markModInstalled(mod, installResult)
      })

      if (reloadAfterInstall) {
        await loadMods()
        return installedMods
      }

      setMods((currentMods) => {
        const updatedMods = currentMods.map((mod) => {
          const installKey = mod.id.toLowerCase()
          const installResult = installResults.get(installKey)

          if (!installedModIds.has(installKey)) {
            return mod
          }

          return markModInstalled(mod, installResult)
        })

        void writeModsLibraryCache(updatedMods, cacheKey)
        return updatedMods
      })

      if (backgroundReloadAfterInstall) {
        void loadMods()
      }

      return installedMods
    } catch (error) {
      setModsError(getErrorMessage(error))
      throw error
    }
  }

  function markModInstalled(mod: ZomboidMod, installResult?: ZomboidModInstallResult) {
    return {
      ...mod,
      isInstalled: true,
      source: mod.source === "steam" || mod.source === "steamcmd" ? "local" : mod.source,
      path: installResult?.targetPath ?? mod.path,
      variants: mod.variants.map((variant) => ({
        ...variant,
        path: installResult?.targetPath ?? variant.path,
      })),
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
