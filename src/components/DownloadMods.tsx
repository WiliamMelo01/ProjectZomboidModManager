import {
  CheckCircle2,
  Download,
  ExternalLink,
  Hash,
  Info,
  Layers3,
  RefreshCw,
  Settings,
  ShieldCheck,
  Square,
  X,
  XCircle,
} from "lucide-react"
import { useEffect, useMemo, useState } from "react"

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
  const [workshopInput, setWorkshopInput] = useState("")
  const [downloadType, setDownloadType] = useState<DownloadType>("item")
  const [forceValidate, setForceValidate] = useState(false)
  const [isCheckingSettings, setIsCheckingSettings] = useState(true)
  const [isSteamcmdConfigured, setIsSteamcmdConfigured] = useState(false)
  const [resolvedSteamcmdPath, setResolvedSteamcmdPath] = useState<string | null>(null)
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

  return (
    <div className="h-full overflow-y-auto bg-[#22272b] p-8 text-white custom-scrollbar">
      <div className="mx-auto max-w-3xl">
        <div className="mb-8">
          <h2 className="text-3xl font-black uppercase italic tracking-tight">Baixar da Oficina</h2>
          <p className="mt-1 text-gray-400">Baixe um mod ou uma coleção com uma única sessão SteamCMD.</p>
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
              Item individual
            </TypeButton>
            <TypeButton active={downloadType === "collection"} onClick={() => setDownloadType("collection")} icon={<Layers3 size={18} />}>
              Coleção completa
            </TypeButton>
          </div>

          <label className="mb-2 block text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
            {downloadType === "collection" ? "Collection ID ou URL" : "Workshop ID ou URL"}
          </label>
          <div className="relative">
            <Hash size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500" />
            <input
              value={workshopInput}
              onChange={(event) => setWorkshopInput(event.target.value)}
              placeholder={downloadType === "collection" ? "Cole o ID ou a URL da coleção" : "Cole o ID ou a URL do mod"}
              className="w-full rounded-2xl border border-white/5 bg-[#1e2327] py-4 pl-12 pr-4 font-mono text-sm focus:border-orange-400/50 focus:outline-none"
            />
          </div>

          <label className="mt-4 flex cursor-pointer items-center gap-3 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
            <input type="checkbox" checked={forceValidate} onChange={(event) => setForceValidate(event.target.checked)} />
            <ShieldCheck size={18} className="text-orange-400" />
            <span>
              <span className="block text-sm font-bold">Forçar validação completa</span>
              <span className="text-xs text-gray-500">Use somente para corrigir downloads corrompidos. O modo normal é mais rápido.</span>
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
              {manager.isDownloading ? "Baixando..." : downloadType === "collection" ? "Baixar coleção" : "Baixar mod"}
            </button>
            {manager.isDownloading && (
              <button onClick={() => void manager.cancelDownload()} className="flex items-center justify-center gap-2 rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm font-bold text-red-300">
                <Square size={16} /> Cancelar
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
              <p className="font-bold">Download concluído</p>
              <p className="text-sm text-green-300">{manager.result.downloadedItems} itens foram baixados e a biblioteca foi atualizada.</p>
            </div>
          </section>
        )}

        {manager.downloadItems.length > 0 && (
          <section className="mt-6 rounded-3xl border border-white/5 bg-[#2b3238] p-6">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <h3 className="font-bold">Progresso dos itens</h3>
              <p className="text-xs text-gray-400">{manager.progress.completedItems} concluídos · {manager.progress.failedItems} falhos · {manager.progress.queuedItems} aguardando</p>
            </div>
            <div className="max-h-72 space-y-2 overflow-y-auto pr-2 custom-scrollbar">
              {manager.downloadItems.map((item) => <DownloadItemRow key={item.workshopId} item={item} />)}
            </div>
          </section>
        )}
      </div>

      {manager.result && manager.isResultModalOpen && (
        <DownloadResultModal
          result={manager.result}
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
  return (
    <div className="mb-6 flex items-start gap-3 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
      {isChecking ? <RefreshCw size={20} className="animate-spin text-orange-400" /> : isConfigured ? <CheckCircle2 size={20} className="text-green-400" /> : <XCircle size={20} className="text-red-400" />}
      <div className="min-w-0 flex-1">
        <p className="text-sm font-bold">{isChecking ? "Verificando SteamCMD" : isConfigured ? "SteamCMD pronto" : "SteamCMD não configurado"}</p>
        <p className="mt-1 break-all text-xs text-gray-500">{path || "Configure o caminho do steamcmd.exe para liberar downloads."}</p>
      </div>
      {!isChecking && !isConfigured && onOpenSettings && (
        <button onClick={onOpenSettings} className="flex items-center gap-2 rounded-xl bg-orange-500 px-3 py-2 text-xs font-bold">
          <Settings size={15} /> Configurar
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
  const color = item.status === "completed" ? "text-green-300" : item.status === "failed" || item.status === "cancelled" ? "text-red-300" : "text-orange-300"
  return <div className="flex items-center gap-3 rounded-xl border border-white/5 bg-[#1e2327] px-4 py-3 text-sm"><Hash size={14} className="text-gray-600" /><span className="flex-1 font-mono">{item.workshopId}</span><span className={color}>{statusLabel(item.status)}</span></div>
}

function DownloadResultModal({ result, onClose, onRetry }: { result: WorkshopDownloadResult; onClose: () => void; onRetry: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm">
      <div className="w-full max-w-2xl rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl">
        <div className="flex items-center justify-between border-b border-white/5 p-6">
          <div><h3 className="text-xl font-bold">Resumo do download</h3><p className="mt-1 text-sm text-gray-400">{result.wasCancelled ? "Download interrompido." : "Alguns itens não puderam ser baixados."}</p></div>
          <button onClick={onClose} className="rounded-full p-2 text-gray-400 hover:bg-white/5"><X size={20} /></button>
        </div>
        <div className="p-6">
          <div className="mb-5 grid grid-cols-2 gap-3 text-center sm:grid-cols-4">
            <ResultCount label="Total" value={result.totalItems} color="text-white" />
            <ResultCount label="Baixados" value={result.downloadedItems} color="text-green-300" />
            <ResultCount label="Falhas" value={result.failedItems.length} color="text-red-300" />
            <ResultCount label="Cancelados" value={result.cancelledItems} color="text-orange-300" />
          </div>
          {result.failedItems.length > 0 && (
            <div className="max-h-72 space-y-3 overflow-y-auto pr-2 custom-scrollbar">
              {result.failedItems.map((item) => <div key={item.workshopId} className="rounded-2xl border border-red-500/10 bg-red-500/5 p-4"><p className="font-bold text-white">{item.name}</p><p className="mt-1 font-mono text-xs text-red-300">{item.workshopId}</p><p className="mt-2 break-words text-xs text-gray-400">{item.error}</p></div>)}
            </div>
          )}
          <div className="mt-6 flex justify-end gap-3">
            <button onClick={onClose} className="rounded-xl border border-white/10 px-4 py-3 text-sm font-bold text-gray-300">Fechar</button>
            {result.failedItems.length > 0 && <button onClick={onRetry} className="rounded-xl bg-orange-500 px-4 py-3 text-sm font-bold">Tentar falhos novamente</button>}
          </div>
        </div>
      </div>
    </div>
  )
}

function ResultCount({ label, value, color }: { label: string; value: number; color: string }) {
  return <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4"><p className={`text-2xl font-black ${color}`}>{value}</p><p className="text-xs text-gray-500">{label}</p></div>
}

function statusLabel(status: DownloadItemStatus) {
  return { queued: "Aguardando", downloading: "Baixando", completed: "Concluído", retrying: "Tentando novamente", failed: "Falhou", cancelled: "Cancelado" }[status]
}
