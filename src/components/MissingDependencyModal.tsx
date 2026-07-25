import { AlertCircle, AlertTriangle, Check, Download, ExternalLink, Hash, RefreshCw, Settings, X } from "lucide-react"
import { useEffect, useState, type FormEvent } from "react"
import { useTranslation } from "react-i18next"

import { invokeTauri } from "@/lib/tauri"
import { i18n } from "@/i18n"
import type { WorkshopDownloadResult } from "@/types/download"
import type { ZomboidMod } from "@/types/mod"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"

type MissingDependencyModalProps = {
  mod: ZomboidMod
  dependencyId: string
  onClose: () => void
  onDownloaded?: (dependencyId: string, originalModId?: string) => Promise<void> | void
  onOpenSettings?: () => void
  remoteConnection?: RemoteConnectionDraft | null
}

type AppSettings = {
  isSteamcmdConfigured: boolean
}

function onlyDigits(value: string) {
  return /^\d+$/.test(value.trim())
}

export function MissingDependencyModal({ mod, dependencyId, onClose, onDownloaded, onOpenSettings, remoteConnection }: MissingDependencyModalProps) {
  const { t } = useTranslation()
  const cleanDependencyId = dependencyId.trim().replace(/^\\+/, "")
  const [workshopId, setWorkshopId] = useState(onlyDigits(cleanDependencyId) ? cleanDependencyId : "")
  const [isAutoResolved, setIsAutoResolved] = useState(onlyDigits(cleanDependencyId))
  const [downloadStep, setDownloadStep] = useState<"idle" | "downloading" | "installing" | "success" | "error">("idle")
  const [isCheckingSettings, setIsCheckingSettings] = useState(true)
  const [isSteamcmdConfigured, setIsSteamcmdConfigured] = useState(false)
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [downloadSuccess, setDownloadSuccess] = useState<string | null>(null)
  
  const canDownload = onlyDigits(workshopId)
  const isDownloading = downloadStep === "downloading" || downloadStep === "installing"

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
      setDownloadError(t("dependency.numericRequired"))
      return
    }

    if (!isSteamcmdConfigured) {
      setDownloadError(t("dependency.steamcmdRequired"))
      return
    }

    setDownloadStep("downloading")
    setDownloadError(null)
    setDownloadSuccess(null)

    const cmdName = remoteConnection ? "download_remote_steam_workshop_item" : "download_steam_workshop_item"
    const cmdArgs = remoteConnection
      ? { connection: remoteConnection, workshopId: workshopId.trim(), forceValidate: false }
      : { workshopId: workshopId.trim(), forceValidate: false }

    try {
      await invokeTauri<WorkshopDownloadResult>(cmdName, cmdArgs)
      setDownloadStep("installing")
      await onDownloaded?.(dependencyId, mod.id)
      setDownloadStep("success")
      setDownloadSuccess(t("dependency.completed"))
    } catch (error) {
      setDownloadStep("error")
      setDownloadError(getErrorMessage(error))
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

        const cleanId = dependencyId.trim().replace(/^\\+/, "").toLowerCase()
        if (!onlyDigits(cleanId)) {
          const mappings = await invokeTauri<Record<string, string>>("get_workshop_mappings")
          if (isMounted) {
            const foundKey = Object.keys(mappings).find(
              (key) => key.trim().replace(/^\\+/, "").toLowerCase() === cleanId
            )
            const mappedId = foundKey ? mappings[foundKey] : null
            if (mappedId) {
              setWorkshopId(mappedId)
              setIsAutoResolved(true)
            }
          }
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
  }, [dependencyId])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-red-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 border-b border-white/5 flex justify-between items-center">
          <div className="flex items-center gap-3">
            <div className={`p-2 rounded-xl ${isAutoResolved ? "bg-green-500/20 text-green-500" : "bg-red-500/20 text-red-500"}`}>
              <AlertCircle size={24} />
            </div>
            <h3 className="text-xl font-bold text-white">
              {isAutoResolved ? t("dependency.autoResolvedTitle") : t("dependency.missingTitle")}
            </h3>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors">
            <X size={20} />
          </button>
        </div>

        <div className="p-6">
          <p className="text-gray-400 text-sm mb-6">
            {isAutoResolved
              ? t("dependency.autoResolvedDescription")
              : t("dependency.missingDescription", { name: mod.name })}
          </p>

          <div className="flex flex-col gap-3 mb-5">
            <div className="flex items-center gap-3 p-4 bg-red-500/5 border border-red-500/10 rounded-2xl">
              <div className="p-3 bg-[#1e2327] rounded-xl text-orange-400">
                <Hash size={20} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-xs text-gray-500 uppercase font-bold tracking-widest">{t("dependency.dependencyId")}</p>
                <p className="text-lg font-mono font-black text-white truncate">{dependencyId}</p>
              </div>
            </div>

            {isAutoResolved && (
              <div className="flex items-center gap-3 p-4 bg-green-500/5 border border-green-500/10 rounded-2xl animate-in fade-in slide-in-from-bottom duration-200">
                <div className="p-3 bg-[#1e2327] rounded-xl text-green-400">
                  <Download size={20} />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-xs text-gray-500 uppercase font-bold tracking-widest">{t("dependency.workshopIdLabel")}</p>
                  <p className="text-lg font-mono font-black text-white truncate">{workshopId}</p>
                </div>
              </div>
            )}
          </div>

          {!isCheckingSettings && !isSteamcmdConfigured && (
            <div className="mb-5 rounded-2xl border border-orange-500/20 bg-orange-500/10 p-4">
              <div className="flex gap-3">
                <AlertTriangle size={20} className="text-orange-400 shrink-0 mt-0.5" />
                <div className="min-w-0">
                  <p className="text-sm font-bold text-white">{t("dependency.steamcmdMissing")}</p>
                  <p className="mt-1 text-xs leading-relaxed text-gray-400">
                    {t("dependency.steamcmdHint")}
                  </p>
                </div>
              </div>

              <div className="mt-4 flex flex-col gap-2 sm:flex-row">
                <button
                  onClick={openSettings}
                  className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-orange-500 px-4 py-3 text-sm font-bold text-white transition-all hover:bg-orange-600"
                >
                  <Settings size={17} />
                  {t("dependency.openSettings")}
                </button>
                <button
                  onClick={onClose}
                  className="flex-1 rounded-xl border border-white/10 px-4 py-3 text-sm font-bold text-gray-400 transition-all hover:bg-white/5 hover:text-white"
                >
                  {t("common.close")}
                </button>
              </div>
            </div>
          )}

          {!isAutoResolved && (
            <form onSubmit={downloadWorkshopItem} className="mb-5">
              <label htmlFor="missing-workshop-id" className="block text-xs text-gray-500 uppercase font-bold tracking-widest mb-2">
                {t("dependency.workshopIdLabel")}
              </label>
              <input
                id="missing-workshop-id"
                value={workshopId}
                onChange={(event) => setWorkshopId(event.target.value)}
                inputMode="numeric"
                placeholder={t("dependency.workshopIdPlaceholder")}
                className="w-full bg-[#1e2327] border border-white/10 rounded-xl py-3 px-4 text-sm font-mono text-white focus:outline-none focus:border-orange-400/50 transition-all placeholder:text-gray-600"
              />
              <p className="mt-2 text-xs text-gray-500">
                {t("dependency.workshopIdHint")}
              </p>
            </form>
          )}

          {isAutoResolved && (
            <p className="mt-2 text-xs text-green-400/80 mb-5 leading-relaxed">
              {t("dependency.autoResolvedHint")}
            </p>
          )}

          {downloadStep !== "idle" && (
            <div className={`p-4 rounded-2xl border flex items-center gap-3 mb-5 transition-all animate-in fade-in duration-300 ${
              downloadStep === "success"
                ? "bg-green-500/5 border-green-500/10 text-green-400"
                : downloadStep === "error"
                  ? "bg-red-500/5 border-red-500/10 text-red-400"
                  : "bg-orange-500/5 border-orange-500/10 text-orange-400"
            }`}>
              <div className={`p-2 rounded-xl ${
                downloadStep === "success"
                  ? "bg-[#1e2327] text-green-400"
                  : downloadStep === "error"
                    ? "bg-[#1e2327] text-red-400"
                    : "bg-[#1e2327] text-orange-400"
              }`}>
                {downloadStep === "success" ? (
                  <Check size={18} />
                ) : downloadStep === "error" ? (
                  <AlertCircle size={18} />
                ) : (
                  <RefreshCw size={18} className="animate-spin" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-xs text-gray-500 uppercase font-bold tracking-widest">
                  {downloadStep === "success"
                    ? t("dependency.successStep")
                    : downloadStep === "error"
                      ? t("common.error")
                      : t("dependency.downloadingShort")}
                </p>
                <p className="text-sm font-bold text-white mt-0.5 leading-relaxed">
                  {downloadStep === "downloading" && t("dependency.downloading")}
                  {downloadStep === "installing" && t("dependency.installingStep")}
                  {downloadStep === "success" && t("dependency.successDescription")}
                  {downloadStep === "error" && downloadError}
                </p>
              </div>
            </div>
          )}

          <div className="flex flex-col gap-3">
            <button
              onClick={() => void openSteamWorkshopExternal()}
              disabled={isDownloading}
              className={`w-full py-3 bg-transparent border border-white/10 font-bold rounded-xl transition-all flex items-center justify-center gap-2 group ${
                isDownloading ? "text-gray-600 cursor-not-allowed" : "text-gray-300 hover:text-white hover:bg-white/5"
              }`}
            >
              <ExternalLink size={18} className="group-hover:translate-x-0.5 group-hover:-translate-y-0.5 transition-transform" />
              {t("dependency.openBrowser")}
            </button>

            {downloadStep === "success" ? (
              <button
                onClick={onClose}
                className="w-full py-3 bg-green-500 hover:bg-green-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-green-500/20 flex items-center justify-center gap-2 animate-in zoom-in-95 duration-200"
              >
                <Check size={18} />
                {t("common.close")}
              </button>
            ) : (
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
                {isDownloading ? t("dependency.downloadingShort") : t("dependency.download")}
              </button>
            )}

            {downloadStep !== "success" && (
              <button
                onClick={onClose}
                disabled={isDownloading}
                className={`w-full py-3 bg-transparent border border-white/10 font-bold rounded-xl transition-all ${
                  isDownloading ? "text-gray-600 cursor-not-allowed" : "text-gray-400 hover:text-white hover:bg-white/5"
                }`}
              >
                {t("dependency.back")}
              </button>
            )}
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

  return i18n.t("dependency.fallbackError")
}
