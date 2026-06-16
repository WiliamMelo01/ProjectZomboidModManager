import {
  CheckCircle2,
  Clock3,
  Download,
  ExternalLink,
  Hash,
  Info,
  Layers3,
  RefreshCw,
  Settings,
  ShieldCheck,
  Square,
  Terminal,
  X,
  XCircle,
} from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import type { WorkshopDownloadManager } from "@/hooks/useWorkshopDownloadManager"
import { invokeTauri } from "@/lib/tauri"
import type { DownloadItemStatus, DownloadListItem, DownloadType, WorkshopDownloadResult, WorkshopDownloadStatus } from "@/types/download"

type AppSettings = {
  resolvedSteamcmdPath: string | null
  isSteamcmdConfigured: boolean
}

type DownloadModsProps = {
  manager: WorkshopDownloadManager
  onOpenSettings?: () => void
}

function extractWorkshopId(value: string) {
  const trimmed = value.trim()

  if (/^\d+$/.test(trimmed)) {
    return trimmed
  }

  return trimmed.match(/[?&]id=(\d+)/)?.[1] ?? ""
}

export function DownloadMods({ manager, onOpenSettings }: DownloadModsProps) {
  const { t } = useTranslation()
  const [workshopInput, setWorkshopInput] = useState("")
  const [downloadType, setDownloadType] = useState<DownloadType>("item")
  const [forceValidate, setForceValidate] = useState(false)
  const [isCheckingSettings, setIsCheckingSettings] = useState(true)
  const [isSteamcmdConfigured, setIsSteamcmdConfigured] = useState(false)
  const [resolvedSteamcmdPath, setResolvedSteamcmdPath] = useState<string | null>(null)
  const logEndRef = useRef<HTMLDivElement>(null)
  const workshopId = useMemo(() => extractWorkshopId(workshopInput), [workshopInput])
  const canDownload = Boolean(workshopId) && isSteamcmdConfigured && !manager.isDownloading && !isCheckingSettings

  async function loadSettings() {
    setIsCheckingSettings(true)

    try {
      const settings = await invokeTauri<AppSettings>("get_app_settings")
      setIsSteamcmdConfigured(settings.isSteamcmdConfigured)
      setResolvedSteamcmdPath(settings.resolvedSteamcmdPath)
    } finally {
      setIsCheckingSettings(false)
    }
  }

  async function handleDownload() {
    if (!workshopId || !isSteamcmdConfigured) {
      return
    }

    await manager.startDownload({ downloadType, workshopId, forceValidate })
  }

  async function openWorkshop() {
    const target = workshopId || workshopInput.trim()

    if (target) {
      await invokeTauri<void>("open_steam_workshop_external", { itemIdOrSearch: target })
    }
  }

  useEffect(() => {
    void loadSettings()
  }, [])

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ block: "end" })
  }, [manager.steamCmdLogLines])

  return (
    <div className="h-full overflow-y-auto bg-[#22272b] p-8 text-white custom-scrollbar">
      <div className="mx-auto max-w-3xl">
        <div className="mb-8">
          <h2 className="text-3xl font-black uppercase italic tracking-tight">{t("downloads.pageTitle")}</h2>
          <p className="mt-1 text-gray-400">{t("downloads.pageDescription")}</p>
        </div>

        <div className="mb-6 flex items-start gap-3 rounded-2xl border border-amber-400/20 bg-amber-500/10 p-4 text-amber-100">
          <Info size={20} className="mt-0.5 shrink-0 text-amber-300" />
          <div>
            <p className="text-sm font-bold">{t("downloads.experimentalNoticeTitle")}</p>
            <p className="mt-1 text-sm leading-relaxed text-amber-100/75">{t("downloads.experimentalNoticeBody")}</p>
          </div>
        </div>

        <section className="rounded-3xl border border-white/5 bg-[#2b3238] p-8 shadow-xl">
          <SteamCmdStatus
            isChecking={isCheckingSettings}
            isConfigured={isSteamcmdConfigured}
            path={resolvedSteamcmdPath}
            onOpenSettings={onOpenSettings}
          />

          <div className="mb-6 grid grid-cols-2 gap-3 rounded-2xl border border-white/5 bg-[#1e2327] p-2">
            <TypeButton active={downloadType === "item"} onClick={() => setDownloadType("item")} icon={<Download size={18} />}>
              {t("downloads.item")}
            </TypeButton>
            <TypeButton active={downloadType === "collection"} onClick={() => setDownloadType("collection")} icon={<Layers3 size={18} />}>
              {t("downloads.collection")}
            </TypeButton>
          </div>

          <label className="mb-2 block text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
            {downloadType === "collection" ? t("downloads.collectionId") : t("downloads.workshopId")}
          </label>
          <div className="relative">
            <Hash size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500" />
            <input
              value={workshopInput}
              onChange={(event) => setWorkshopInput(event.target.value)}
              placeholder={downloadType === "collection" ? t("downloads.collectionPlaceholder") : t("downloads.modPlaceholder")}
              className="w-full rounded-2xl border border-white/5 bg-[#1e2327] py-4 pl-12 pr-4 font-mono text-sm focus:border-orange-400/50 focus:outline-none"
            />
          </div>

          <label className="mt-4 flex cursor-pointer items-center gap-3 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
            <input type="checkbox" checked={forceValidate} onChange={(event) => setForceValidate(event.target.checked)} />
            <ShieldCheck size={18} className="text-orange-400" />
            <span>
              <span className="block text-sm font-bold">{t("downloads.validate")}</span>
              <span className="text-xs text-gray-500">{t("downloads.validateHint")}</span>
            </span>
          </label>

          {manager.status && <StatusBox status={manager.status} />}

          <div className="mt-5 grid gap-3 sm:grid-cols-[1fr_auto_auto]">
            <button
              onClick={() => void handleDownload()}
              disabled={!canDownload}
              className="flex items-center justify-center gap-3 rounded-2xl bg-orange-500 px-5 py-4 font-black uppercase italic tracking-widest transition-colors hover:bg-orange-600 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
            >
              {manager.isDownloading ? <RefreshCw size={20} className="animate-spin" /> : <Download size={20} />}
              {manager.isDownloading ? t("downloads.downloading") : downloadType === "collection" ? t("downloads.downloadCollection") : t("downloads.downloadMod")}
            </button>
            {manager.isDownloading && (
              <button onClick={() => void manager.cancelDownload()} className="flex items-center justify-center gap-2 rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm font-bold text-red-300">
                <Square size={16} /> {t("common.cancel")}
              </button>
            )}
            <button onClick={() => void openWorkshop()} disabled={!workshopInput.trim()} className="flex items-center justify-center gap-2 rounded-2xl border border-white/10 bg-[#1e2327] px-5 py-4 text-sm font-bold text-gray-300 disabled:text-gray-600">
              <ExternalLink size={18} /> Workshop
            </button>
          </div>
        </section>

        {manager.result && manager.result.failedItems.length === 0 && !manager.result.wasCancelled && (
          <section className="mt-6 flex items-center gap-4 rounded-3xl border border-green-500/20 bg-green-500/10 p-5 text-green-200">
            <CheckCircle2 size={24} />
            <div>
              <p className="font-bold">{t("downloads.completedTitle")}</p>
              <p className="text-sm text-green-300">{t("downloads.completedLibrary", { downloaded: manager.result.downloadedItems, skipped: manager.result.skippedItems })}</p>
              <p className="mt-1 flex items-center gap-1.5 text-xs font-bold text-green-300">
                <Clock3 size={13} />
                {t("downloads.finishedIn", { time: manager.elapsedLabel })}
              </p>
            </div>
          </section>
        )}

        {manager.downloadItems.length > 0 && (
          <section className="mt-6 rounded-3xl border border-white/5 bg-[#2b3238] p-6">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <h3 className="font-bold">{t("downloads.itemProgress")}</h3>
              <div className="text-right">
                <p className="text-xs text-gray-400">
                  {t("downloads.itemProgressSummary", {
                    completed: manager.progress.completedItems,
                    skipped: manager.progress.skippedItems,
                    failed: manager.progress.failedItems,
                    queued: manager.progress.queuedItems,
                  })}
                </p>
                <p className="mt-1 flex items-center justify-end gap-1.5 text-xs font-bold text-orange-200">
                  <Clock3 size={13} />
                  {t(manager.isDownloading ? "downloads.elapsed" : "downloads.finishedIn", { time: manager.elapsedLabel })}
                </p>
              </div>
            </div>
            <div className="max-h-72 space-y-2 overflow-y-auto pr-2 custom-scrollbar">
              {manager.downloadItems.map((item) => <DownloadItemRow key={item.workshopId} item={item} />)}
            </div>
          </section>
        )}

        {manager.steamCmdLogLines.length > 0 && (
          <section className="mt-6 overflow-hidden rounded-3xl border border-white/5 bg-[#111417]">
            <div className="flex items-center gap-2 border-b border-white/5 px-5 py-4 text-gray-300">
              <Terminal size={16} className="text-orange-400" />
              <h3 className="text-xs font-black uppercase tracking-widest">{t("downloads.steamcmdLog")}</h3>
            </div>
            <div className="max-h-72 overflow-y-auto whitespace-pre-wrap p-5 font-mono text-xs leading-relaxed text-gray-400 custom-scrollbar">
              {manager.steamCmdLogLines.map((entry, index) => (
                <div key={`${index}:${entry.instanceId}:${entry.line.slice(0, 24)}`} className={`mb-1 flex gap-2 rounded-r-md border-l-2 px-2 py-1 ${steamCmdInstanceLineClass(entry.colorKey)}`}>
                  <span className="shrink-0 font-black">
                    [{entry.label}]
                  </span>
                  <span>{entry.line}</span>
                </div>
              ))}
              <div ref={logEndRef} />
            </div>
          </section>
        )}
      </div>

      {manager.result && manager.isResultModalOpen && (
        <DownloadResultModal
          result={manager.result}
          elapsedLabel={manager.elapsedLabel}
          onClose={manager.closeResultModal}
          onRetry={() => void manager.retryFailedItems()}
        />
      )}
    </div>
  )
}

function SteamCmdStatus({ isChecking, isConfigured, path, onOpenSettings }: {
  isChecking: boolean
  isConfigured: boolean
  path: string | null
  onOpenSettings?: () => void
}) {
  const { t } = useTranslation()
  return (
    <div className="mb-6 flex items-start gap-3 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
      {isChecking ? <RefreshCw size={20} className="animate-spin text-orange-400" /> : isConfigured ? <CheckCircle2 size={20} className="text-green-400" /> : <XCircle size={20} className="text-red-400" />}
      <div className="min-w-0 flex-1">
        <p className="text-sm font-bold">{isChecking ? t("downloads.checking") : isConfigured ? t("downloads.configured") : t("downloads.notConfigured")}</p>
        <p className="mt-1 break-all text-xs text-gray-500">{path || t("downloads.configureHint")}</p>
      </div>
      {!isChecking && !isConfigured && onOpenSettings && (
        <button onClick={onOpenSettings} className="flex items-center gap-2 rounded-xl bg-orange-500 px-3 py-2 text-xs font-bold">
          <Settings size={15} /> {t("downloads.configure")}
        </button>
      )}
    </div>
  )
}

function TypeButton({ active, onClick, icon, children }: { active: boolean; onClick: () => void; icon: React.ReactNode; children: React.ReactNode }) {
  return <button onClick={onClick} className={`flex items-center justify-center gap-2 rounded-xl px-4 py-3 text-sm font-bold ${active ? "bg-orange-500 text-white" : "text-gray-400 hover:bg-white/5"}`}>{icon}{children}</button>
}

function StatusBox({ status }: { status: WorkshopDownloadStatus }) {
  return <div className={`mt-4 flex items-start gap-3 rounded-2xl border p-4 text-sm ${status.type === "success" ? "border-green-500/20 bg-green-500/10 text-green-300" : status.type === "error" ? "border-red-500/20 bg-red-500/10 text-red-300" : "border-orange-500/20 bg-orange-500/10 text-orange-300"}`}><Info size={18} />{status.message}</div>
}

function DownloadItemRow({ item }: { item: DownloadListItem }) {
  const { t } = useTranslation()
  const color = item.status === "completed" ? "text-green-300" : item.status === "skipped" ? "text-blue-300" : item.status === "failed" || item.status === "cancelled" ? "text-red-300" : "text-orange-300"
  return <div className="flex items-center gap-3 rounded-xl border border-white/5 bg-[#1e2327] px-4 py-3 text-sm"><Hash size={14} className="text-gray-600" /><span className="flex-1 font-mono">{item.workshopId}</span><span className={color}>{statusLabel(item.status, t)}</span></div>
}

function DownloadResultModal({ result, elapsedLabel, onClose, onRetry }: { result: WorkshopDownloadResult; elapsedLabel: string; onClose: () => void; onRetry: () => void }) {
  const { t } = useTranslation()

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm">
      <div className="w-full max-w-2xl rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl">
        <div className="flex items-center justify-between border-b border-white/5 p-6">
          <div><h3 className="text-xl font-bold">{t("downloads.summary")}</h3><p className="mt-1 text-sm text-gray-400">{t(result.wasCancelled ? "downloads.interrupted" : "downloads.failedSome")}</p></div>
          <button onClick={onClose} className="rounded-full p-2 text-gray-400 hover:bg-white/5"><X size={20} /></button>
        </div>
        <div className="p-6">
          <div className="mb-5 grid grid-cols-2 gap-3 text-center sm:grid-cols-6">
            <ResultCount label={t("downloads.total")} value={result.totalItems} color="text-white" />
            <ResultCount label={t("downloads.downloaded")} value={result.downloadedItems} color="text-green-300" />
            <ResultCount label={t("downloads.skipped")} value={result.skippedItems} color="text-blue-300" />
            <ResultCount label={t("downloads.failures")} value={result.failedItems.length} color="text-red-300" />
            <ResultCount label={t("downloads.cancelledCount")} value={result.cancelledItems} color="text-orange-300" />
            <ResultCount label={t("downloads.duration")} value={elapsedLabel} color="text-orange-200" />
          </div>
          {result.failedItems.length > 0 && (
            <div className="max-h-72 space-y-3 overflow-y-auto pr-2 custom-scrollbar">
              {result.failedItems.map((item) => <div key={item.workshopId} className="rounded-2xl border border-red-500/10 bg-red-500/5 p-4"><p className="font-bold text-white">{item.name}</p><p className="mt-1 font-mono text-xs text-red-300">{item.workshopId}</p><p className="mt-2 break-words text-xs text-gray-400">{item.error}</p></div>)}
            </div>
          )}
          <div className="mt-6 flex justify-end gap-3">
            <button onClick={onClose} className="rounded-xl border border-white/10 px-4 py-3 text-sm font-bold text-gray-300">{t("common.close")}</button>
            {result.failedItems.length > 0 && <button onClick={onRetry} className="rounded-xl bg-orange-500 px-4 py-3 text-sm font-bold">{t("downloads.retryFailed")}</button>}
          </div>
        </div>
      </div>
    </div>
  )
}

function ResultCount({ label, value, color }: { label: string; value: number | string; color: string }) {
  return <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4"><p className={`text-2xl font-black ${color}`}>{value}</p><p className="text-xs text-gray-500">{label}</p></div>
}

function steamCmdInstanceLineClass(colorKey: string) {
  return {
    orange: "border-l-orange-400 bg-orange-500/5 text-orange-200",
    blue: "border-l-sky-400 bg-sky-500/5 text-sky-200",
    green: "border-l-emerald-400 bg-emerald-500/5 text-emerald-200",
  }[colorKey] ?? "border-l-gray-500 bg-white/5 text-gray-300"
}

function statusLabel(status: DownloadItemStatus, t: (key: string) => string) {
  return t({ queued: "downloads.queued", downloading: "downloads.downloading", completed: "downloads.completed", retrying: "downloads.retryingStatus", failed: "downloads.failed", cancelled: "downloads.cancelledStatus", skipped: "downloads.skippedStatus" }[status])
}
