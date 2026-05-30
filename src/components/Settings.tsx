import { CheckCircle2, ChevronDown, Cpu, Folder, FolderOpen, FolderPlus, Lightbulb, Monitor, RefreshCw, Save, Search, XCircle } from "lucide-react"
import { useEffect, useState } from "react"

import { invokeTauri } from "@/lib/tauri"

type AppSettings = {
  steamcmdPath: string
  resolvedSteamcmdPath: string | null
  isSteamcmdConfigured: boolean
  gameExecutablePath: string
  clientRam: string
  serverRam: string
}

type ModLocation = {
  label: string
  path: string
  kind: string
  exists: boolean
}

type ZomboidInstallationStatus = {
  defaultGameDir: string
  detectedExecutablePath: string | null
  isGameDirFound: boolean
  isExecutableFound: boolean
  isClientConfigFound: boolean
  isServerConfigFound: boolean
}

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
                  <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-8 shadow-xl relative group">
                    <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />

                    <div className="flex items-center gap-3 mb-6 relative z-10">
                      <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
                        <Folder size={20} />
                      </div>
                      <div>
                        <h3 className="text-xl font-bold text-white">Integracao SteamCMD</h3>
                        <p className="text-xs text-gray-500">Usado para baixar itens da Workshop do Project Zomboid.</p>
                      </div>
                    </div>

                    <div className="mb-6 rounded-2xl border border-white/5 bg-[#1e2327] p-4 relative z-10">
                      <div className="flex items-start gap-3">
                        {isConfigured ? (
                          <CheckCircle2 size={20} className="text-green-400 shrink-0 mt-0.5" />
                        ) : (
                          <XCircle size={20} className="text-red-400 shrink-0 mt-0.5" />
                        )}
                        <div className="min-w-0">
                          <p className="text-sm font-bold text-white">
                            {isConfigured ? "SteamCMD configurado" : "SteamCMD nao configurado"}
                          </p>
                          <p className="text-xs text-gray-500 break-all">
                            {resolvedPath || "Informe o caminho do steamcmd.exe ou use a deteccao automatica."}
                          </p>
                        </div>
                      </div>
                    </div>

                    <div className="space-y-3 relative z-10">
                      <label htmlFor="steamcmd-path" className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                        Caminho do executavel
                      </label>
                      <div className="flex flex-col gap-3 md:flex-row">
                        <div className="relative flex-1 group/input">
                          <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within/input:text-orange-400 transition-colors">
                            <Folder size={18} />
                          </div>
                          <input
                            id="steamcmd-path"
                            type="text"
                            value={steamCmdPath}
                            onChange={(event) => setSteamCmdPath(event.target.value)}
                            placeholder="C:\steamcmd\steamcmd.exe"
                            className="w-full bg-[#1e2327] border border-white/5 rounded-2xl py-3.5 pl-12 pr-4 text-sm focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-700"
                          />
                        </div>
                        <button
                          className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95"
                          onClick={() => void browseSteamCmd()}
                        >
                          <Folder size={18} />
                          Procurar
                        </button>
                        <button
                          className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95"
                          onClick={() => void detectSteamCmd()}
                        >
                          <Search size={18} />
                          Detectar
                        </button>
                      </div>

                      <p className="text-xs text-gray-500">
                        Ao salvar vazio, o app tenta encontrar pelo STEAMCMD_PATH, PATH e locais comuns como C:\steamcmd.
                      </p>
                    </div>
                  </section>

                  <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-8 shadow-xl relative group">
                    <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />

                    <div className="flex items-center justify-between mb-6 relative z-10">
                      <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
                          <FolderPlus size={20} />
                        </div>
                        <div>
                          <h3 className="text-xl font-bold text-white">Bibliotecas de Mods</h3>
                          <p className="text-xs text-gray-500">Locais padrao salvos no arquivo settings.ini.</p>
                        </div>
                      </div>
                      <div className="flex flex-wrap justify-end gap-2">
                        <button
                          disabled={isAddingFolder}
                          onClick={() => void addModFolder()}
                          className="flex items-center gap-2 bg-orange-500/10 text-orange-400 hover:bg-orange-500 hover:text-white disabled:opacity-60 px-4 py-2 rounded-xl transition-all font-bold text-sm border border-orange-500/20 active:scale-95"
                        >
                          {isAddingFolder ? <RefreshCw size={18} className="animate-spin" /> : <FolderPlus size={18} />}
                          <span>Adicionar pasta</span>
                        </button>
                        <button
                          onClick={() => void refreshModLocations()}
                          className="flex items-center gap-2 bg-orange-500/10 text-orange-400 hover:bg-orange-500 hover:text-white px-4 py-2 rounded-xl transition-all font-bold text-sm border border-orange-500/20 active:scale-95"
                        >
                          <RefreshCw size={18} />
                          <span>Recarregar lista</span>
                        </button>
                      </div>
                    </div>

                    <div className="space-y-3 relative z-10">
                      <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                        Locais salvos
                      </label>
                      <div className="grid gap-2">
                        {modLocations.length === 0 ? (
                          <div className="bg-[#1e2327] border border-dashed border-white/5 rounded-2xl p-8 text-center">
                            <p className="text-sm text-gray-600">Nenhum local de mods salvo.</p>
                          </div>
                        ) : (
                          modLocations.map((location) => (
                            <div key={`${location.kind}:${location.path}`} className="group/path flex items-center gap-3 bg-[#1e2327] border border-white/5 rounded-2xl p-3 pl-4 transition-all hover:border-orange-500/20">
                              <Folder size={18} className="text-gray-500 group-hover/path:text-orange-400 transition-colors shrink-0" />
                              <div className="min-w-0 flex-1">
                                <div className="flex flex-wrap items-center gap-2">
                                  <span className="text-sm font-bold text-white">{location.label}</span>
                                  <span className={`rounded-full border px-2 py-0.5 text-[10px] font-bold uppercase ${
                                    location.exists
                                      ? "border-green-500/20 bg-green-500/10 text-green-300"
                                      : "border-red-500/20 bg-red-500/10 text-red-300"
                                  }`}>
                                    {location.exists ? "Encontrado" : "Nao existe"}
                                  </span>
                                </div>
                                <p className="mt-1 truncate font-mono text-xs text-gray-400">{location.path}</p>
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  </section>
                </div>
              )}

              {activeTab === "ram" && (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
                  <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-8 shadow-xl relative group">
                    <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />

                    <div className="flex items-center gap-3 mb-6 relative z-10">
                      <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
                        <Monitor size={20} />
                      </div>
                      <div>
                        <h3 className="text-xl font-bold text-white">Configuracao do Jogo</h3>
                        <p className="text-xs text-gray-500">Defina o executavel e a memoria alocada para o client.</p>
                      </div>
                    </div>

                    <div className="space-y-4 relative z-10">
                      <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
                        <div className="flex items-start gap-3">
                          {isScanningZomboid ? (
                            <RefreshCw size={20} className="text-orange-400 shrink-0 mt-0.5 animate-spin" />
                          ) : zomboidStatus?.isExecutableFound && zomboidStatus?.isClientConfigFound ? (
                            <CheckCircle2 size={20} className="text-green-400 shrink-0 mt-0.5" />
                          ) : (
                            <XCircle size={20} className="text-red-400 shrink-0 mt-0.5" />
                          )}
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center justify-between gap-3">
                              <p className="text-sm font-bold text-white">
                                {zomboidStatus?.isExecutableFound && zomboidStatus?.isClientConfigFound
                                  ? "Project Zomboid configurado"
                                  : "Project Zomboid nao configurado"}
                              </p>
                              <button
                                type="button"
                                onClick={() => void scanZomboidInstallation()}
                                className="flex items-center gap-2 rounded-xl border border-orange-500/20 bg-orange-500/10 px-3 py-1.5 text-xs font-bold text-orange-400 transition-all hover:bg-orange-500 hover:text-white"
                              >
                                <RefreshCw size={14} className={isScanningZomboid ? "animate-spin" : ""} />
                                Escanear
                              </button>
                            </div>
                            <p className="mt-1 text-xs text-gray-500 break-all">
                              {zomboidStatus?.detectedExecutablePath ||
                                "O app tenta usar a pasta padrao da Steam e localizar ProjectZomboid64.exe automaticamente."}
                            </p>
                            <div className="mt-3 grid gap-2 text-[11px] text-gray-500 md:grid-cols-3">
                              <span className={zomboidStatus?.isGameDirFound ? "text-green-300" : "text-red-300"}>
                                Pasta Steam: {zomboidStatus?.isGameDirFound ? "encontrada" : "nao encontrada"}
                              </span>
                              <span className={zomboidStatus?.isClientConfigFound ? "text-green-300" : "text-red-300"}>
                                Launcher: {zomboidStatus?.isClientConfigFound ? "ok" : "pendente"}
                              </span>
                              <span className={zomboidStatus?.isServerConfigFound ? "text-green-300" : "text-yellow-300"}>
                                Servidor: {zomboidStatus?.isServerConfigFound ? "ok" : "nao encontrado"}
                              </span>
                            </div>
                            <p className="mt-2 text-[11px] text-gray-600 break-all">
                              Pasta padrao: {zomboidStatus?.defaultGameDir || "C:\\Program Files (x86)\\Steam\\steamapps\\common\\ProjectZomboid"}
                            </p>
                          </div>
                        </div>
                      </div>

                      <div className="space-y-3">
                        <label htmlFor="game-path" className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                          Executavel do Jogo (.exe)
                        </label>
                        <div className="flex flex-col gap-3 md:flex-row">
                          <div className="relative flex-1 group/input">
                            <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within/input:text-orange-400 transition-colors">
                              <Folder size={18} />
                            </div>
                            <input
                              id="game-path"
                              type="text"
                              value={gameExecutablePath}
                              onChange={(event) => setGameExecutablePath(event.target.value)}
                              placeholder="C:\\SteamLibrary\\steamapps\\common\\ProjectZomboid\\ProjectZomboid64.exe"
                              className="w-full bg-[#1e2327] border border-white/5 rounded-2xl py-3.5 pl-12 pr-4 text-sm focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-700"
                            />
                          </div>
                          <button
                            className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95"
                            onClick={() => void browseGameExecutable()}
                          >
                            <Folder size={18} />
                            Procurar
                          </button>
                          <button
                            className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95"
                            onClick={() => void openSteamZomboidFolder()}
                          >
                            <FolderOpen size={18} />
                            Abrir pasta
                          </button>
                        </div>
                      </div>

                      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 pt-2">
                        <div className="space-y-3">
                          <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                            RAM do Client (Jogo)
                          </label>
                          <CustomDropdown
                            value={clientRam}
                            onChange={setClientRam}
                            options={ramOptions}
                          />
                        </div>

                        <div className="space-y-3">
                          <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                            RAM do Servidor
                          </label>
                          <CustomDropdown
                            value={serverRam}
                            onChange={setServerRam}
                            options={ramOptions}
                          />
                        </div>
                      </div>
                    </div>
                  </section>

                  <div className="p-4 bg-orange-400/5 border border-orange-400/10 rounded-2xl flex gap-3">
                    <Search size={20} className="text-orange-400 shrink-0 mt-0.5" />
                    <p className="text-[11px] text-gray-400 leading-relaxed italic">
                      O app usará o executável selecionado para localizar o arquivo de configuração e ajustar as flags de memória (-Xms e -Xmx). Certifique-se de selecionar o executável correto da versão que você utiliza (geralmente 64 bits).
                    </p>
                  </div>
                </div>
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
          <div className="hidden lg:block absolute top-24 right-0 w-72 animate-in fade-in slide-in-from-right-4 duration-500">
            <section className="bg-[#2b3238] rounded-3xl border border-orange-400/20 p-6 shadow-xl relative overflow-hidden group">
              <div className="absolute top-0 right-0 w-24 h-24 bg-orange-500/5 blur-3xl rounded-full -mr-12 -mt-12" />

              <div className="flex items-center gap-3 mb-4">
                <div className="p-2 bg-orange-500/10 text-orange-400 rounded-lg">
                  <Lightbulb size={20} />
                </div>
                <h3 className="font-bold text-white tracking-tight text-sm uppercase italic">Dicas de Alocação</h3>
              </div>

              <div className="space-y-6">
                <div>
                  <p className="text-[9px] font-black text-gray-500 uppercase tracking-widest mb-3">Client (Jogo)</p>
                  <ul className="space-y-3">
                    <li className="flex gap-2">
                      <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
                      <p className="text-[11px] text-gray-400"><span className="text-white font-bold">Vanilla:</span> 2GB a 4GB é o suficiente.</p>
                    </li>
                    <li className="flex gap-2">
                      <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
                      <p className="text-[11px] text-gray-400"><span className="text-white font-bold">Alguns Mods:</span> 4GB a 6GB recomendado.</p>
                    </li>
                    <li className="flex gap-2">
                      <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
                      <p className="text-[11px] text-gray-400"><span className="text-white font-bold">Muitos Mods:</span> 8GB+ para estabilidade.</p>
                    </li>
                  </ul>
                </div>

                <div className="h-px bg-white/5" />

                <div>
                  <p className="text-[9px] font-black text-gray-500 uppercase tracking-widest mb-3">Servidor</p>
                  <ul className="space-y-3">
                    <li className="flex gap-2">
                      <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
                      <p className="text-[11px] text-gray-400"><span className="text-white font-bold">Pequeno:</span> 2GB a 4GB dão conta.</p>
                    </li>
                    <li className="flex gap-2">
                      <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
                      <p className="text-[11px] text-gray-400"><span className="text-white font-bold">Médio + Mods:</span> 6GB a 8GB recomendados.</p>
                    </li>
                    <li className="flex gap-2">
                      <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
                      <p className="text-[11px] text-gray-400"><span className="text-white font-bold">Grandes:</span> 12GB+ para mapas extensos.</p>
                    </li>
                  </ul>
                </div>

                <div className="bg-[#1e2327] rounded-2xl p-4 border border-white/5">
                  <div className="flex items-center gap-2 mb-2">
                    <Lightbulb size={12} className="text-orange-400" />
                    <span className="text-[9px] font-bold text-white uppercase italic">Atenção</span>
                  </div>
                  <p className="text-[10px] text-gray-500 leading-relaxed italic">
                    Deixe sempre 2-4GB livres para o seu Windows funcionar sem travar.
                  </p>
                </div>
              </div>
            </section>
          </div>
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

function CustomDropdown({ value, onChange, options }: { value: string; onChange: (val: string) => void; options: string[] }) {
  const [isOpen, setIsOpen] = useState(false)

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full bg-[#1e2327] border rounded-2xl py-4 px-5 text-sm font-medium transition-all flex items-center justify-between text-white group ${
          isOpen ? "border-orange-400/50 ring-1 ring-orange-400/20" : "border-white/5 hover:border-white/10"
        }`}
      >
        <span>{value} GB</span>
        <ChevronDown
          size={18}
          className={`text-gray-500 group-hover:text-orange-400 transition-all ${isOpen ? "rotate-180 text-orange-400" : ""}`}
        />
      </button>

      {isOpen && (
        <>
          <div className="fixed inset-0 z-[60]" onClick={() => setIsOpen(false)} />
          <div className="absolute top-full left-0 right-0 mt-2 bg-[#1e2327] border border-white/10 rounded-2xl overflow-hidden shadow-2xl z-[70] animate-in fade-in zoom-in-95 duration-200">
            <div className="max-h-60 overflow-y-auto custom-scrollbar">
              {options.map((opt) => (
                <button
                  key={opt}
                  type="button"
                  onClick={() => {
                    onChange(opt)
                    setIsOpen(false)
                  }}
                  className={`w-full text-left px-5 py-3 text-sm transition-colors hover:bg-orange-500/10 hover:text-orange-400 ${
                    value === opt ? "text-orange-400 bg-orange-500/5 font-bold" : "text-gray-400"
                  }`}
                >
                  {opt} GB
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  )
}
