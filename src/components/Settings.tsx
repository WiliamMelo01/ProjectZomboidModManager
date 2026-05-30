import { Cpu, Folder, RefreshCw, Save } from "lucide-react"
import { useEffect, useState } from "react"

import { GamePerformanceSection } from "@/components/settings/GamePerformanceSection"
import { ModLocationsSection } from "@/components/settings/ModLocationsSection"
import { RamTips } from "@/components/settings/RamTips"
import { SteamCmdSettingsSection } from "@/components/settings/SteamCmdSettingsSection"
import { invokeTauri } from "@/lib/tauri"
import type { AppSettings, ModLocation, ZomboidInstallationStatus } from "@/types/settings"

export function Settings() {
  const [activeTab, setActiveTab] = useState<"mods" | "ram">("mods")
  const [steamCmdPath, setSteamCmdPath] = useState("")
  const [gameExecutablePath, setGameExecutablePath] = useState("")
  const [clientRam, setClientRam] = useState("4.00")
  const [serverRam, setServerRam] = useState("4.00")
  const [totalSystemRam, setTotalSystemRam] = useState(16)

  const [modLocations, setModLocations] = useState<ModLocation[]>([])
  const [resolvedPath, setResolvedPath] = useState<string | null>(null)
  const [isConfigured, setIsConfigured] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [isSaving, setIsSaving] = useState(false)
  const [isAddingFolder, setIsAddingFolder] = useState(false)
  const [isScanningZomboid, setIsScanningZomboid] = useState(false)
  const [zomboidStatus, setZomboidStatus] = useState<ZomboidInstallationStatus | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function loadSettings() {
    setIsLoading(true)
    setError(null)

    try {
      const [settings, locations, systemRam] = await Promise.all([
        invokeTauri<AppSettings>("get_app_settings"),
        invokeTauri<ModLocation[]>("get_mod_locations"),
        invokeTauri<number>("get_system_ram").catch(() => 16), // Fallback to 16 if not implemented yet
      ])

      applySettings(settings)
      setModLocations(locations)
      setTotalSystemRam(systemRam)
      await scanZomboidInstallation(settings.gameExecutablePath)
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
      const settings = await invokeTauri<AppSettings>("save_app_settings", {
        steamcmdPath: steamCmdPath.trim(),
        gameExecutablePath: gameExecutablePath.trim(),
        clientRam,
        serverRam,
      })

      applySettings(settings)
      await scanZomboidInstallation(settings.gameExecutablePath)
      setModLocations(await invokeTauri<ModLocation[]>("get_mod_locations"))
      setMessage("Configuracoes salvas.")
    } catch (saveError) {
      setError(getErrorMessage(saveError))
    } finally {
      setIsSaving(false)
    }
  }

  async function browseGameExecutable() {
    setMessage(null)
    setError(null)

    try {
      const selectedPath = await invokeTauri<string | null>("select_game_executable")

      if (selectedPath) {
        setGameExecutablePath(selectedPath)
        await scanZomboidInstallation(selectedPath)
        setMessage("Executavel selecionado. Salve para confirmar.")
      }
    } catch (browseError) {
      setError(getErrorMessage(browseError))
    }
  }

  async function openSteamZomboidFolder() {
    setMessage(null)
    setError(null)

    try {
      const openedPath = await invokeTauri<string>("open_steam_zomboid_folder")
      setMessage(`Pasta aberta: ${openedPath}`)
    } catch (openError) {
      setError(getErrorMessage(openError))
    }
  }

  async function scanZomboidInstallation(path = gameExecutablePath) {
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

  async function detectSteamCmd() {
    setMessage(null)
    setError(null)

    try {
      const detectedPath = await invokeTauri<string | null>("detect_steamcmd_path")

      if (detectedPath) {
        setSteamCmdPath(detectedPath)
        setMessage("SteamCMD encontrado. Salve para usar este caminho nos downloads.")
      } else {
        setError("SteamCMD nao foi encontrado automaticamente.")
      }
    } catch (detectError) {
      setError(getErrorMessage(detectError))
    }
  }

  async function browseSteamCmd() {
    setMessage(null)
    setError(null)

    try {
      const selectedPath = await invokeTauri<string | null>("select_steamcmd_path")

      if (selectedPath) {
        setSteamCmdPath(selectedPath)
        setMessage("SteamCMD selecionado. Salve para usar este caminho nos downloads.")
      }
    } catch (browseError) {
      setError(getErrorMessage(browseError))
    }
  }

  function clearPath() {
    if (activeTab === "ram") {
      setGameExecutablePath("")
      setMessage("Caminho do jogo limpo. Salve para confirmar.")
    } else {
      setSteamCmdPath("")
      setMessage("Caminho limpo. Salve para voltar a usar deteccao automatica.")
    }

    setError(null)
  }

  function applySettings(settings: AppSettings) {
    setSteamCmdPath(settings.steamcmdPath ?? "")
    setResolvedPath(settings.resolvedSteamcmdPath ?? null)
    setIsConfigured(Boolean(settings.isSteamcmdConfigured))
    setGameExecutablePath(settings.gameExecutablePath ?? "")
    setClientRam(settings.clientRam ?? "4.00")
    setServerRam(settings.serverRam ?? "4.00")
  }

  const ramOptions = Array.from({ length: totalSystemRam * 4 }, (_, i) => ((i + 1) * 0.25).toFixed(2))

  async function refreshModLocations() {
    setError(null)
    setMessage(null)

    try {
      setModLocations(await invokeTauri<ModLocation[]>("get_mod_locations"))
      setMessage("Locais de mods atualizados.")
    } catch (refreshError) {
      setError(getErrorMessage(refreshError))
    }
  }

  async function addModFolder() {
    setIsAddingFolder(true)
    setError(null)
    setMessage(null)

    try {
      const selectedPath = await invokeTauri<string | null>("select_mod_folder")

      if (!selectedPath) {
        return
      }

      setModLocations(
        await invokeTauri<ModLocation[]>("add_mod_location", {
          path: selectedPath,
        }),
      )
      setMessage("Pasta de mods adicionada.")
    } catch (addError) {
      setError(getErrorMessage(addError))
    } finally {
      setIsAddingFolder(false)
    }
  }

  useEffect(() => {
    void loadSettings()
  }, [])

  useEffect(() => {
    if (activeTab === "ram") {
      void scanZomboidInstallation()
    }
  }, [activeTab])

  return (
    <div className="h-full bg-[#22272b] p-8 text-white overflow-y-auto custom-scrollbar">
      <div className="max-w-6xl mx-auto relative">
        {/* Main Settings Column */}
        <div className={`transition-all duration-500 ${activeTab === 'ram' ? 'lg:pr-80' : ''}`}>
          <div className="max-w-3xl">
            <div className="mb-8">
              <h2 className="text-3xl font-black tracking-tight text-white uppercase italic">Configuracoes</h2>
              <p className="text-gray-400 mt-1">Gerencie caminhos e desempenho do jogo e ferramentas.</p>
            </div>

            {/* Tab Navigation */}
            <div className="flex gap-4 p-1 bg-[#1e2327] rounded-2xl border border-white/5 mb-8">
              <button
                onClick={() => setActiveTab("mods")}
                className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                  activeTab === "mods" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                }`}
              >
                <Folder size={18} />
                Mods & Downloads
              </button>
              <button
                onClick={() => setActiveTab("ram")}
                className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                  activeTab === "ram" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                }`}
              >
                <Cpu size={18} />
                Desempenho (RAM)
              </button>
            </div>

            <div className="space-y-6">
              {activeTab === "mods" && (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                  <SteamCmdSettingsSection
                    path={steamCmdPath}
                    resolvedPath={resolvedPath}
                    isConfigured={isConfigured}
                    onPathChange={setSteamCmdPath}
                    onBrowse={() => void browseSteamCmd()}
                    onDetect={() => void detectSteamCmd()}
                  />
                  <ModLocationsSection
                    locations={modLocations}
                    isAddingFolder={isAddingFolder}
                    onAddFolder={() => void addModFolder()}
                    onRefresh={() => void refreshModLocations()}
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
                <button
                  onClick={clearPath}
                  className="rounded-2xl border border-white/10 px-6 py-4 text-sm font-bold text-gray-400 transition-all hover:bg-white/5 hover:text-white"
                >
                  Limpar caminho
                </button>
                <button
                  disabled={isLoading || isSaving}
                  onClick={() => void saveSettings()}
                  className="flex items-center justify-center gap-2 bg-gradient-to-r from-orange-500 to-orange-600 hover:from-orange-400 hover:to-orange-500 disabled:from-white/10 disabled:to-white/10 disabled:text-gray-500 text-white px-8 py-4 rounded-2xl font-black uppercase italic tracking-wider transition-all shadow-lg shadow-orange-500/20 active:scale-95"
                >
                  {isSaving ? <RefreshCw size={20} className="animate-spin" /> : <Save size={20} />}
                  <span>{isSaving ? "Salvando" : "Salvar configuracoes"}</span>
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

  return "Nao foi possivel carregar as configuracoes."
}
