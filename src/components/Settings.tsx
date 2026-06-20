import { Cpu, Folder, RefreshCw, Save } from "lucide-react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { LanguageSettingsSection } from "@/components/settings/LanguageSettingsSection"
import { GamePerformanceSection } from "@/components/settings/GamePerformanceSection"
import { ModLocationsSection } from "@/components/settings/ModLocationsSection"
import { RamTips } from "@/components/settings/RamTips"
import { SteamCmdSettingsSection } from "@/components/settings/SteamCmdSettingsSection"
import { invokeTauri } from "@/lib/tauri"
import { setLanguagePreference } from "@/i18n"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import type { AppSettings, LanguagePreference, ModLocation, ZomboidInstallationStatus } from "@/types/settings"

const SETTINGS_VIEW_CACHE_KEY = "pzmm:settings-view"
const REMOTE_SETTINGS_VIEW_CACHE_PREFIX = "pzmm:settings-view:remote"
const SETTINGS_VIEW_CACHE_VERSION = 2

type SettingsViewCache = {
  version: number
  settings: AppSettings
  modLocations: ModLocation[]
  totalSystemRam: number
}

type SettingsProps = {
  onRescanMods?: () => Promise<void>
  remoteConnection?: RemoteConnectionDraft | null
}

export function Settings({ onRescanMods, remoteConnection = null }: SettingsProps) {
  const { t } = useTranslation()
  const isRemoteWorkspace = remoteConnection !== null
  const cacheKey = getSettingsViewCacheKey(remoteConnection)
  const [cachedView] = useState(() => readSettingsViewCache(cacheKey))
  const [activeTab, setActiveTab] = useState<"mods" | "ram">("mods")
  const [loadedSettings, setLoadedSettings] = useState<AppSettings | null>(cachedView?.settings ?? null)
  const [gameExecutablePath, setGameExecutablePath] = useState(cachedView?.settings.gameExecutablePath ?? "")
  const [clientRam, setClientRam] = useState(cachedView?.settings.clientRam ?? "4.00")
  const [serverRam, setServerRam] = useState(cachedView?.settings.serverRam ?? "4.00")
  const [maxConcurrentDownloads, setMaxConcurrentDownloads] = useState(cachedView?.settings.maxConcurrentDownloads ?? 1)
  const [languagePreference, setLanguagePreferenceState] = useState<LanguagePreference>(cachedView?.settings.languagePreference ?? "auto")
  const [totalSystemRam, setTotalSystemRam] = useState(cachedView?.totalSystemRam ?? 16)

  const [modLocations, setModLocations] = useState<ModLocation[]>(cachedView?.modLocations ?? [])
  const [resolvedPath, setResolvedPath] = useState<string | null>(cachedView?.settings.resolvedSteamcmdPath ?? null)
  const [isConfigured, setIsConfigured] = useState(Boolean(cachedView?.settings.isSteamcmdConfigured))
  const [isLoading, setIsLoading] = useState(true)
  const [isSaving, setIsSaving] = useState(false)
  const [isAddingFolder, setIsAddingFolder] = useState(false)
  const [isRescanningMods, setIsRescanningMods] = useState(false)
  const [isScanningZomboid, setIsScanningZomboid] = useState(false)
  const [zomboidStatus, setZomboidStatus] = useState<ZomboidInstallationStatus | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function loadSettings() {
    setIsLoading(true)
    setError(null)

    try {
      const [settings, locations, systemRam] = isRemoteWorkspace && remoteConnection
        ? await Promise.all([
          invokeTauri<AppSettings>("get_remote_app_settings", { connection: remoteConnection }),
          invokeTauri<ModLocation[]>("get_remote_mod_locations", { connection: remoteConnection }),
          invokeTauri<number>("get_remote_system_ram", { connection: remoteConnection }).catch(() => 16),
        ])
        : await Promise.all([
          invokeTauri<AppSettings>("get_app_settings"),
          invokeTauri<ModLocation[]>("get_mod_locations"),
          invokeTauri<number>("get_system_ram").catch(() => 16), // Fallback to 16 if not implemented yet
        ])

      applySettings(settings)
      setModLocations(locations)
      setTotalSystemRam(systemRam)
      writeSettingsViewCache(cacheKey, settings, locations, systemRam)
      if (isRemoteWorkspace) {
        applyRemoteZomboidStatus(settings.gameExecutablePath)
      } else {
        await scanZomboidInstallation(settings.gameExecutablePath)
      }
    } catch (loadError) {
      setError(getErrorMessage(loadError))
    } finally {
      setIsLoading(false)
    }
  }

  async function saveSettings() {
    setIsSaving(true)
    setMessage(null)
    setError(null)

    try {
      const settings = isRemoteWorkspace && remoteConnection
        ? await invokeTauri<AppSettings>("save_remote_app_settings", {
          request: {
            connection: remoteConnection,
            gameExecutablePath: gameExecutablePath.trim(),
            clientRam,
            serverRam,
          },
        })
        : await invokeTauri<AppSettings>("save_app_settings", {
          steamcmdPath: "",
          gameExecutablePath: gameExecutablePath.trim(),
          clientRam,
          serverRam,
          maxConcurrentDownloads,
        })

      applySettings(settings)
      if (isRemoteWorkspace) {
        applyRemoteZomboidStatus(settings.gameExecutablePath)
      } else {
        await scanZomboidInstallation(settings.gameExecutablePath)
      }
      const locations = isRemoteWorkspace && remoteConnection
        ? await invokeTauri<ModLocation[]>("get_remote_mod_locations", { connection: remoteConnection })
        : await invokeTauri<ModLocation[]>("get_mod_locations")
      setModLocations(locations)
      writeSettingsViewCache(cacheKey, settings, locations, totalSystemRam)
      setMessage(t("settings.saved"))
    } catch (saveError) {
      setError(getErrorMessage(saveError))
    } finally {
      setIsSaving(false)
    }
  }

  async function browseGameExecutable() {
    setMessage(null)
    setError(null)

    if (isRemoteWorkspace) {
      setMessage("Edit the remote server path directly, or configure it from remote setup.")
      return
    }

    try {
      const selectedPath = await invokeTauri<string | null>("select_game_executable")

      if (selectedPath) {
        setGameExecutablePath(selectedPath)
        await scanZomboidInstallation(selectedPath)
        setMessage(t("settings.selectedExecutable"))
      }
    } catch (browseError) {
      setError(getErrorMessage(browseError))
    }
  }

  async function openSteamZomboidFolder() {
    setMessage(null)
    setError(null)

    if (isRemoteWorkspace) {
      setMessage(`Remote path: ${gameExecutablePath || remoteConnection?.serverPath || ""}`)
      return
    }

    try {
      const openedPath = await invokeTauri<string>("open_steam_zomboid_folder")
      setMessage(t("settings.openedFolder", { path: openedPath }))
    } catch (openError) {
      setError(getErrorMessage(openError))
    }
  }

  async function scanZomboidInstallation(path = gameExecutablePath) {
    if (isRemoteWorkspace) {
      applyRemoteZomboidStatus(path)
      return
    }
    setIsScanningZomboid(true)

    try {
      const status = await invokeTauri<ZomboidInstallationStatus>("scan_zomboid_installation", {
        gameExecutablePath: path.trim() || null,
      })

      setZomboidStatus(status)

      if (!path.trim() && status.detectedExecutablePath) {
        setGameExecutablePath(status.detectedExecutablePath)
      }
    } catch (scanError) {
      setError(getErrorMessage(scanError))
    } finally {
      setIsScanningZomboid(false)
    }
  }

  function clearPath() {
    setGameExecutablePath("")
    setMessage(t("settings.clearedGamePath"))
    setError(null)
  }

  function applyRemoteZomboidStatus(path = gameExecutablePath) {
    const trimmedPath = path.trim()
    setZomboidStatus({
      defaultGameDir: remoteConnection?.serverPath ?? "",
      detectedExecutablePath: trimmedPath || null,
      isGameDirFound: Boolean(trimmedPath),
      isExecutableFound: Boolean(trimmedPath),
      isClientConfigFound: false,
      isServerConfigFound: Boolean(trimmedPath),
    })
  }

  function applySettings(settings: AppSettings) {
    setLoadedSettings(settings)
    setResolvedPath(settings.resolvedSteamcmdPath ?? null)
    setIsConfigured(Boolean(settings.isSteamcmdConfigured))
    setGameExecutablePath(settings.gameExecutablePath ?? "")
    setClientRam(settings.clientRam ?? "4.00")
    setServerRam(settings.serverRam ?? "4.00")
    setMaxConcurrentDownloads(settings.maxConcurrentDownloads ?? 1)
    setLanguagePreferenceState(settings.languagePreference ?? "auto")
  }

  async function changeLanguage(preference: LanguagePreference) {
    const previousPreference = languagePreference
    setLanguagePreferenceState(preference)
    setMessage(null)
    setError(null)

    try {
      await setLanguagePreference(preference)
      const locations = isRemoteWorkspace && remoteConnection
        ? await invokeTauri<ModLocation[]>("get_remote_mod_locations", { connection: remoteConnection })
        : await invokeTauri<ModLocation[]>("get_mod_locations")
      setModLocations(locations)
      writeSettingsViewCache(cacheKey, currentSettingsSnapshot(loadedSettings, {
        gameExecutablePath,
        clientRam,
        serverRam,
        maxConcurrentDownloads,
        languagePreference: preference,
      }), locations, totalSystemRam)
      setMessage(t("language.saved"))
    } catch (languageError) {
      setLanguagePreferenceState(previousPreference)
      setError(getErrorMessage(languageError))
    }
  }

  const ramOptions = Array.from({ length: totalSystemRam * 4 }, (_, i) => ((i + 1) * 0.25).toFixed(2))

  async function refreshModLocations() {
    setError(null)
    setMessage(null)

    try {
      const locations = isRemoteWorkspace && remoteConnection
        ? await invokeTauri<ModLocation[]>("get_remote_mod_locations", { connection: remoteConnection })
        : await invokeTauri<ModLocation[]>("get_mod_locations")
      setModLocations(locations)
      writeSettingsViewCache(cacheKey, currentSettingsSnapshot(loadedSettings, {
        gameExecutablePath,
        clientRam,
        serverRam,
        maxConcurrentDownloads,
        languagePreference,
      }), locations, totalSystemRam)
      setMessage(t("settings.modLocations.refreshed"))
    } catch (refreshError) {
      setError(getErrorMessage(refreshError))
    }
  }

  async function rescanAllMods() {
    if (!onRescanMods || isRescanningMods) {
      return
    }

    setIsRescanningMods(true)
    setError(null)
    setMessage(null)

    try {
      await onRescanMods()
      setModLocations(isRemoteWorkspace && remoteConnection
        ? await invokeTauri<ModLocation[]>("get_remote_mod_locations", { connection: remoteConnection })
        : await invokeTauri<ModLocation[]>("get_mod_locations"))
      setMessage(t("settings.modLocations.rescanned"))
    } catch (rescanError) {
      setError(getErrorMessage(rescanError))
    } finally {
      setIsRescanningMods(false)
    }
  }

  async function openModLocation(path: string) {
    setError(null)

    try {
      if (isRemoteWorkspace && remoteConnection) {
        await invokeTauri<void>("open_remote_mod_location", {
          request: { connection: remoteConnection, path },
        })
        setMessage(`Remote folder is reachable: ${path}`)
        return
      }

      await invokeTauri<void>("open_mod_location", { path })
    } catch (openError) {
      setError(getErrorMessage(openError))
    }
  }

  async function addModFolder() {
    setIsAddingFolder(true)
    setError(null)
    setMessage(null)

    try {
      const selectedPath = isRemoteWorkspace
        ? window.prompt("Remote mod folder path")
        : await invokeTauri<string | null>("select_mod_folder")

      if (!selectedPath) {
        return
      }

      const locations = isRemoteWorkspace && remoteConnection
        ? await invokeTauri<ModLocation[]>("add_remote_mod_location", {
          request: { connection: remoteConnection, path: selectedPath },
        })
        : await invokeTauri<ModLocation[]>("add_mod_location", {
          path: selectedPath,
        })
      setModLocations(locations)
      writeSettingsViewCache(cacheKey, currentSettingsSnapshot(loadedSettings, {
        gameExecutablePath,
        clientRam,
        serverRam,
        maxConcurrentDownloads,
        languagePreference,
      }), locations, totalSystemRam)
      setMessage(t("settings.modLocations.added"))
    } catch (addError) {
      setError(getErrorMessage(addError))
    } finally {
      setIsAddingFolder(false)
    }
  }

  useEffect(() => {
    const nextCachedView = readSettingsViewCache(cacheKey)

    if (nextCachedView) {
      applySettings(nextCachedView.settings)
      setModLocations(nextCachedView.modLocations)
      setTotalSystemRam(nextCachedView.totalSystemRam)
    } else {
      setLoadedSettings(null)
      setGameExecutablePath("")
      setClientRam("4.00")
      setServerRam("4.00")
      setMaxConcurrentDownloads(1)
      setLanguagePreferenceState("auto")
      setTotalSystemRam(16)
      setModLocations([])
      setResolvedPath(null)
      setIsConfigured(false)
      setZomboidStatus(null)
    }

    void loadSettings()
  }, [cacheKey])

  useEffect(() => {
    if (activeTab === "ram") {
      void scanZomboidInstallation()
    }
  }, [activeTab])

  return (
    <div className="h-full bg-[#22272b] p-8 text-white overflow-y-auto custom-scrollbar">
      <div className="max-w-6xl mx-auto relative">
        {/* Main Settings Column */}
        <div className={`transition-all duration-500 ${activeTab === "mods" || activeTab === "ram" ? "lg:pr-80" : ""}`}>
          <div className="max-w-3xl">
            <div className="mb-8">
              <h2 className="text-3xl font-black tracking-tight text-white uppercase italic">{t("settings.title")}</h2>
              <p className="text-gray-400 mt-1">{t("settings.description")}</p>
            </div>

            <LanguageSettingsSection preference={languagePreference} onChange={(preference) => void changeLanguage(preference)} />

            {/* Tab Navigation */}
            <div className="flex gap-4 p-1 bg-[#1e2327] rounded-2xl border border-white/5 mb-8">
              <button
                onClick={() => setActiveTab("mods")}
                className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                  activeTab === "mods" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                }`}
              >
                <Folder size={18} />
                {t("settings.modsDownloads")}
              </button>
              <button
                onClick={() => setActiveTab("ram")}
                className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                  activeTab === "ram" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                }`}
              >
                <Cpu size={18} />
                {t("settings.performance")}
              </button>
            </div>

            <div className="space-y-6">
              {activeTab === "mods" && (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                  <SteamCmdSettingsSection
                    resolvedPath={resolvedPath}
                    isConfigured={isConfigured}
                    maxConcurrentDownloads={maxConcurrentDownloads}
                  />
                  <ModLocationsSection
                    locations={modLocations}
                    isAddingFolder={isAddingFolder}
                    isRescanning={isRescanningMods}
                    onAddFolder={() => void addModFolder()}
                    onRefresh={() => void refreshModLocations()}
                    onRescan={() => void rescanAllMods()}
                    onOpenLocation={(path) => void openModLocation(path)}
                  />
                </div>
              )}

              {activeTab === "ram" && (
                <GamePerformanceSection
                  path={gameExecutablePath}
                  clientRam={clientRam}
                  serverRam={serverRam}
                  ramOptions={ramOptions}
                  status={zomboidStatus}
                  isScanning={isScanningZomboid}
                  onPathChange={setGameExecutablePath}
                  onClientRamChange={setClientRam}
                  onServerRamChange={setServerRam}
                  onBrowse={() => void browseGameExecutable()}
                  onOpenFolder={() => void openSteamZomboidFolder()}
                  onScan={() => void scanZomboidInstallation()}
                  isRemoteWorkspace={isRemoteWorkspace}
                />
              )}

              {error && (
                <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
                  {error}
                </div>
              )}

              {message && (
                <div className="rounded-2xl border border-green-500/20 bg-green-500/10 px-5 py-4 text-sm text-green-300">
                  {message}
                </div>
              )}

              <div className="flex flex-col justify-end gap-3 pt-4 sm:flex-row">
                {activeTab === "ram" && (
                  <button
                    onClick={clearPath}
                    className="rounded-2xl border border-white/10 px-6 py-4 text-sm font-bold text-gray-400 transition-all hover:bg-white/5 hover:text-white"
                  >
                    {t("settings.clearPath")}
                  </button>
                )}
                <button
                  disabled={isLoading || isSaving}
                  onClick={() => void saveSettings()}
                  className="flex items-center justify-center gap-2 bg-gradient-to-r from-orange-500 to-orange-600 hover:from-orange-400 hover:to-orange-500 disabled:from-white/10 disabled:to-white/10 disabled:text-gray-500 text-white px-8 py-4 rounded-2xl font-black uppercase italic tracking-wider transition-all shadow-lg shadow-orange-500/20 active:scale-95"
                >
                  {isSaving ? <RefreshCw size={20} className="animate-spin" /> : <Save size={20} />}
                  <span>{isSaving ? t("settings.saving") : t("settings.save")}</span>
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* Tips Sidebar */}
        {activeTab === "ram" && (
          <RamTips />
        )}
      </div>
    </div>
  )
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  return "Could not load settings."
}

function getSettingsViewCacheKey(remoteConnection: RemoteConnectionDraft | null) {
  if (!remoteConnection) {
    return SETTINGS_VIEW_CACHE_KEY
  }

  const remoteId = [
    remoteConnection.name,
    remoteConnection.username,
    remoteConnection.host,
    remoteConnection.port,
  ].map((part) => encodeURIComponent(part.trim())).join(":")

  return `${REMOTE_SETTINGS_VIEW_CACHE_PREFIX}:${remoteId}`
}

function readSettingsViewCache(cacheKey: string): SettingsViewCache | null {
  try {
    const rawCache = window.localStorage.getItem(cacheKey)

    if (!rawCache) {
      return null
    }

    const cache = JSON.parse(rawCache) as Partial<SettingsViewCache>

    if (
      cache.version !== SETTINGS_VIEW_CACHE_VERSION ||
      !isAppSettings(cache.settings) ||
      !Array.isArray(cache.modLocations) ||
      !cache.modLocations.every(isModLocation) ||
      typeof cache.totalSystemRam !== "number"
    ) {
      window.localStorage.removeItem(cacheKey)
      return null
    }

    return cache as SettingsViewCache
  } catch {
    return null
  }
}

function writeSettingsViewCache(cacheKey: string, settings: AppSettings, modLocations: ModLocation[], totalSystemRam: number) {
  try {
    window.localStorage.setItem(cacheKey, JSON.stringify({
      version: SETTINGS_VIEW_CACHE_VERSION,
      settings,
      modLocations,
      totalSystemRam,
    }))
  } catch {
    // The backend remains the source of truth if browser storage is unavailable.
  }
}

function currentSettingsSnapshot(
  loadedSettings: AppSettings | null,
  values: Pick<AppSettings, "gameExecutablePath" | "clientRam" | "serverRam" | "maxConcurrentDownloads" | "languagePreference">,
): AppSettings {
  return {
    steamcmdPath: loadedSettings?.steamcmdPath ?? "",
    resolvedSteamcmdPath: loadedSettings?.resolvedSteamcmdPath ?? null,
    isSteamcmdConfigured: loadedSettings?.isSteamcmdConfigured ?? false,
    ...values,
  }
}

function isAppSettings(value: unknown): value is AppSettings {
  if (!value || typeof value !== "object") {
    return false
  }

  const settings = value as Record<string, unknown>

  return (
    typeof settings.steamcmdPath === "string" &&
    (settings.resolvedSteamcmdPath === null || typeof settings.resolvedSteamcmdPath === "string") &&
    typeof settings.isSteamcmdConfigured === "boolean" &&
    typeof settings.gameExecutablePath === "string" &&
    typeof settings.clientRam === "string" &&
    typeof settings.serverRam === "string" &&
    typeof settings.maxConcurrentDownloads === "number" &&
    isLanguagePreference(settings.languagePreference)
  )
}

function isLanguagePreference(value: unknown): value is LanguagePreference {
  return value === "auto" || value === "en" || value === "pt-BR"
}

function isModLocation(value: unknown): value is ModLocation {
  if (!value || typeof value !== "object") {
    return false
  }

  const location = value as Record<string, unknown>

  return (
    typeof location.label === "string" &&
    typeof location.path === "string" &&
    typeof location.kind === "string" &&
    typeof location.exists === "boolean"
  )
}
