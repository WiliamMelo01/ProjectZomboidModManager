import { CheckCircle2, Copy, Download, ExternalLink, RefreshCw, Search } from "lucide-react"
import { useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { i18n } from "@/i18n"
import { invokeTauri } from "@/lib/tauri"

type WorkshopDownloadResult = {
  downloadedItems: number
  failedItems: { workshopId: string; name: string; error: string }[]
}

function onlyDigits(value: string) {
  return /^\d+$/.test(value.trim())
}

export function WorkshopWindow() {
  const { t } = useTranslation()
  const params = useMemo(() => {
    const hash = window.location.hash
    const queryIndex = hash.indexOf("?")
    return new URLSearchParams(queryIndex >= 0 ? hash.slice(queryIndex + 1) : "")
  }, [])
  const target = params.get("target") ?? ""
  const url = params.get("url") ?? ""
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isDownloading, setIsDownloading] = useState(false)
  const canDownload = onlyDigits(target)

  async function openExternal() {
    setMessage(null)
    setError(null)

    try {
      await invokeTauri<void>("open_steam_workshop_external", {
        itemIdOrSearch: target,
      })
    } catch (openError) {
      setError(getErrorMessage(openError))
    }
  }

  async function openSteamClient() {
    setMessage(null)
    setError(null)

    try {
      await invokeTauri<void>("open_steam_workshop_steam_client", {
        itemIdOrSearch: target,
      })
    } catch (openError) {
      setError(getErrorMessage(openError))
    }
  }

  async function copyUrl() {
    setMessage(null)
    setError(null)

    try {
      await navigator.clipboard.writeText(url)
      setMessage(t("workshopWindow.copied"))
    } catch {
      setError(t("workshopWindow.copyError"))
    }
  }

  async function downloadWithSteamCmd() {
    if (!canDownload) {
      setError(t("workshopWindow.numericRequired"))
      return
    }

    setIsDownloading(true)
    setMessage(null)
    setError(null)

    try {
      const result = await invokeTauri<WorkshopDownloadResult>("download_steam_workshop_item", {
        workshopId: target.trim(),
        forceValidate: false,
      })
      setMessage(
        result.failedItems.length > 0
          ? t("workshopWindow.failed", { count: result.failedItems.length })
          : t("workshopWindow.downloaded", { count: result.downloadedItems }),
      )
    } catch (downloadError) {
      setError(getErrorMessage(downloadError))
    } finally {
      setIsDownloading(false)
    }
  }

  return (
    <main className="min-h-screen bg-[#22272b] p-8 text-white">
      <div className="mx-auto flex min-h-[calc(100vh-4rem)] max-w-2xl items-center">
        <section className="w-full rounded-3xl border border-white/5 bg-[#2b3238] p-8 shadow-xl">
          <div className="mb-7 flex items-start gap-4">
            <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl border border-orange-500/20 bg-orange-500/10 text-orange-400">
              {canDownload ? <Download size={24} /> : <Search size={24} />}
            </div>
            <div className="min-w-0">
              <h1 className="text-2xl font-black uppercase italic tracking-tight">Steam Workshop</h1>
              <p className="mt-1 text-sm leading-relaxed text-gray-400">
                {t("workshopWindow.description")}
              </p>
            </div>
          </div>

          <div className="mb-6 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
            <p className="text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
              {canDownload ? "Workshop ID" : t("workshopWindow.search")}
            </p>
            <p className="mt-2 break-all font-mono text-lg font-black text-white">{target || t("workshopWindow.empty")}</p>
            <p className="mt-3 break-all font-mono text-xs text-gray-500">{url}</p>
          </div>

          {message && (
            <div className="mb-4 flex items-center gap-2 rounded-2xl border border-green-500/20 bg-green-500/10 px-4 py-3 text-sm text-green-300">
              <CheckCircle2 size={18} />
              {message}
            </div>
          )}

          {error && (
            <div className="mb-4 rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-300">
              {error}
            </div>
          )}

          <div className="grid gap-3 sm:grid-cols-2">
            <button
              onClick={() => void openExternal()}
              className="flex items-center justify-center gap-2 rounded-xl bg-orange-500 px-4 py-3 text-sm font-black uppercase italic tracking-wide text-white transition-colors hover:bg-orange-600"
            >
              <ExternalLink size={18} />
              {t("workshopWindow.browser")}
            </button>
            <button
              onClick={() => void openSteamClient()}
              className="flex items-center justify-center gap-2 rounded-xl border border-white/10 bg-[#1e2327] px-4 py-3 text-sm font-bold text-gray-200 transition-colors hover:bg-white/5 hover:text-white"
            >
              <ExternalLink size={18} />
              Steam
            </button>
            <button
              onClick={() => void copyUrl()}
              className="flex items-center justify-center gap-2 rounded-xl border border-white/10 bg-[#1e2327] px-4 py-3 text-sm font-bold text-gray-200 transition-colors hover:bg-white/5 hover:text-white"
            >
              <Copy size={18} />
              {t("workshopWindow.copyLink")}
            </button>
            <button
              disabled={!canDownload || isDownloading}
              onClick={() => void downloadWithSteamCmd()}
              className="flex items-center justify-center gap-2 rounded-xl border border-white/10 bg-[#1e2327] px-4 py-3 text-sm font-bold text-gray-200 transition-colors hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:text-gray-600"
            >
              {isDownloading ? <RefreshCw size={18} className="animate-spin" /> : <Download size={18} />}
              SteamCMD
            </button>
          </div>
        </section>
      </div>
    </main>
  )
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  return i18n.t("workshopWindow.fallbackError")
}
