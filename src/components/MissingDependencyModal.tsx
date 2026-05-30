import { AlertCircle, AlertTriangle, Download, ExternalLink, Hash, RefreshCw, Settings, X } from "lucide-react"
import { useEffect, useState, type FormEvent } from "react"

import { invokeTauri } from "@/lib/tauri"
import type { WorkshopDownloadResult } from "@/types/download"
import type { ZomboidMod } from "@/types/mod"

type MissingDependencyModalProps = {
  mod: ZomboidMod
  dependencyId: string
  onClose: () => void
  onDownloaded?: (dependencyId: string) => Promise<void> | void
  onOpenSettings?: () => void
}

type AppSettings = {
  isSteamcmdConfigured: boolean
}

function onlyDigits(value: string) {
  return /^\d+$/.test(value.trim())
}

export function MissingDependencyModal({ mod, dependencyId, onClose, onDownloaded, onOpenSettings }: MissingDependencyModalProps) {
  const [workshopId, setWorkshopId] = useState(onlyDigits(dependencyId) ? dependencyId : "")
  const [isDownloading, setIsDownloading] = useState(false)
  const [isCheckingSettings, setIsCheckingSettings] = useState(true)
  const [isSteamcmdConfigured, setIsSteamcmdConfigured] = useState(false)
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [downloadSuccess, setDownloadSuccess] = useState<string | null>(null)
  const canDownload = onlyDigits(workshopId)

  const openSteamWorkshop = async () => {
    setDownloadError(null)

    try {
      await invokeTauri<void>("open_steam_workshop", {
        itemIdOrSearch: workshopId.trim() || dependencyId,
      })
    } catch (error) {
      setDownloadError(getErrorMessage(error))
    }
  }

  const openSteamWorkshopExternal = async () => {
    setDownloadError(null)

    try {
      await invokeTauri<void>("open_steam_workshop_external", {
        itemIdOrSearch: workshopId.trim() || dependencyId,
      })
    } catch (error) {
      setDownloadError(getErrorMessage(error))
    }
  }

  const openSettings = () => {
    onClose()
    onOpenSettings?.()
  }

  const downloadWorkshopItem = async (event?: FormEvent) => {
    event?.preventDefault()

    if (!canDownload) {
      setDownloadError("Informe o Workshop ID numerico do item para baixar com SteamCMD.")
      return
    }

    if (!isSteamcmdConfigured) {
      setDownloadError("Configure o SteamCMD antes de baixar dependencias da Steam Workshop.")
      return
    }

    setIsDownloading(true)
    setDownloadError(null)
    setDownloadSuccess(null)

    try {
      await invokeTauri<WorkshopDownloadResult>("download_steam_workshop_item", {
        workshopId: workshopId.trim(),
        forceValidate: false,
      })
      setDownloadSuccess("Download concluido. Atualizando a biblioteca de mods...")
      await onDownloaded?.(dependencyId)
    } catch (error) {
      setDownloadError(getErrorMessage(error))
    } finally {
      setIsDownloading(false)
    }
  }

  useEffect(() => {
    let isMounted = true

    async function checkSteamcmdSettings() {
      setIsCheckingSettings(true)

      try {
        const settings = await invokeTauri<AppSettings>("get_app_settings")

        if (isMounted) {
          setIsSteamcmdConfigured(settings.isSteamcmdConfigured)
        }
      } catch (error) {
        if (isMounted) {
          setIsSteamcmdConfigured(false)
          setDownloadError(getErrorMessage(error))
        }
      } finally {
        if (isMounted) {
          setIsCheckingSettings(false)
        }
      }
    }

    void checkSteamcmdSettings()

    return () => {
      isMounted = false
    }
  }, [])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-red-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 border-b border-white/5 flex justify-between items-center">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-red-500/20 text-red-500 rounded-xl">
              <AlertCircle size={24} />
            </div>
            <h3 className="text-xl font-bold text-white">Mod nao encontrado</h3>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors">
            <X size={20} />
          </button>
        </div>

        <div className="p-6">
          <p className="text-gray-400 text-sm mb-6">
            O mod <span className="text-white font-bold">{mod.name}</span> requer uma dependencia que nao esta em sua
            biblioteca:
          </p>

          <div className="flex items-center gap-3 p-4 bg-red-500/5 border border-red-500/10 rounded-2xl mb-5">
            <div className="p-3 bg-[#1e2327] rounded-xl text-orange-400">
              <Hash size={20} />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-xs text-gray-500 uppercase font-bold tracking-widest">ID da Dependencia</p>
              <p className="text-lg font-mono font-black text-white truncate">{dependencyId}</p>
            </div>
          </div>

          {!isCheckingSettings && !isSteamcmdConfigured && (
            <div className="mb-5 rounded-2xl border border-orange-500/20 bg-orange-500/10 p-4">
              <div className="flex gap-3">
                <AlertTriangle size={20} className="text-orange-400 shrink-0 mt-0.5" />
                <div className="min-w-0">
                  <p className="text-sm font-bold text-white">SteamCMD nao configurado</p>
                  <p className="mt-1 text-xs leading-relaxed text-gray-400">
                    Para baixar esta dependencia da Steam Workshop, configure primeiro o caminho do steamcmd.exe.
                  </p>
                </div>
              </div>

              <div className="mt-4 flex flex-col gap-2 sm:flex-row">
                <button
                  onClick={openSettings}
                  className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-orange-500 px-4 py-3 text-sm font-bold text-white transition-all hover:bg-orange-600"
                >
                  <Settings size={17} />
                  Ir para configuracoes
                </button>
                <button
                  onClick={onClose}
                  className="flex-1 rounded-xl border border-white/10 px-4 py-3 text-sm font-bold text-gray-400 transition-all hover:bg-white/5 hover:text-white"
                >
                  Fechar
                </button>
              </div>
            </div>
          )}

          <form onSubmit={downloadWorkshopItem} className="mb-5">
            <label htmlFor="missing-workshop-id" className="block text-xs text-gray-500 uppercase font-bold tracking-widest mb-2">
              Workshop ID para SteamCMD
            </label>
            <input
              id="missing-workshop-id"
              value={workshopId}
              onChange={(event) => setWorkshopId(event.target.value)}
              inputMode="numeric"
              placeholder="Cole o ID numerico do item"
              className="w-full bg-[#1e2327] border border-white/10 rounded-xl py-3 px-4 text-sm font-mono text-white focus:outline-none focus:border-orange-400/50 transition-all placeholder:text-gray-600"
            />
            <p className="mt-2 text-xs text-gray-500">
              Se a dependencia veio como Mod ID, abra a busca na Workshop e cole aqui o ID numerico da pagina do item.
            </p>
          </form>

          {downloadError && (
            <div className="mb-4 rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-xs text-red-300">
              {downloadError}
            </div>
          )}

          {downloadSuccess && (
            <div className="mb-4 rounded-2xl border border-green-500/20 bg-green-500/10 px-4 py-3 text-xs text-green-300">
              {downloadSuccess}
            </div>
          )}

          <div className="flex flex-col gap-3">
            <button
              onClick={() => void openSteamWorkshop()}
              className="w-full py-3 bg-[#2b3238] hover:bg-[#353c42] border border-white/10 text-white font-bold rounded-xl transition-all flex items-center justify-center gap-2 group"
            >
              <ExternalLink size={18} className="group-hover:translate-x-0.5 group-hover:-translate-y-0.5 transition-transform" />
              Abrir Steam Workshop no app
            </button>
            <button
              onClick={() => void openSteamWorkshopExternal()}
              className="w-full py-3 bg-transparent hover:bg-white/5 border border-white/10 text-gray-300 hover:text-white font-bold rounded-xl transition-all flex items-center justify-center gap-2 group"
            >
              <ExternalLink size={18} className="group-hover:translate-x-0.5 group-hover:-translate-y-0.5 transition-transform" />
              Abrir no navegador
            </button>
            <button
              onClick={() => void downloadWorkshopItem()}
              disabled={!canDownload || isDownloading || isCheckingSettings || !isSteamcmdConfigured}
              className={`w-full py-3 font-bold rounded-xl transition-all shadow-lg flex items-center justify-center gap-2 ${
                canDownload && !isDownloading && !isCheckingSettings && isSteamcmdConfigured
                  ? "bg-orange-500 hover:bg-orange-600 text-white shadow-orange-500/20"
                  : "bg-white/5 text-gray-500 border border-white/5 cursor-not-allowed"
              }`}
            >
              {isDownloading ? <RefreshCw size={18} className="animate-spin" /> : <Download size={18} />}
              {isDownloading ? "Baixando com SteamCMD..." : "Baixar com SteamCMD"}
            </button>
            <button
              onClick={onClose}
              className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all"
            >
              Voltar
            </button>
          </div>
        </div>
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

  return "Nao foi possivel baixar o item da Steam Workshop."
}
