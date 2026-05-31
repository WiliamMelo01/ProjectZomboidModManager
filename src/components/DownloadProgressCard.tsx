import { Download, RefreshCw } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { WorkshopDownloadManager } from "@/hooks/useWorkshopDownloadManager"

type DownloadProgressCardProps = {
  manager: WorkshopDownloadManager
  onOpen: () => void
}

export function DownloadProgressCard({ manager, onOpen }: DownloadProgressCardProps) {
  const { t } = useTranslation()
  const { progress } = manager

  return (
    <button
      onClick={onOpen}
      className="fixed bottom-6 right-6 z-40 w-80 rounded-2xl border border-orange-500/20 bg-[#22272b] p-4 text-left shadow-2xl shadow-black/50 transition-colors hover:bg-[#293036]"
    >
      <div className="flex items-start gap-3">
        <div className="rounded-xl bg-orange-500/10 p-2 text-orange-400">
          {progress.isPreparing ? <RefreshCw size={18} className="animate-spin" /> : <Download size={18} />}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <p className="truncate text-sm font-black text-white">{t("downloads.progressTitle")}</p>
            {!progress.isPreparing && <span className="text-xs font-black text-orange-300">{progress.percentage}%</span>}
          </div>
          <p className="mt-1 text-xs text-gray-400">
            {progress.isPreparing
              ? t("downloads.preparing")
              : t("downloads.progressSummary", { completed: progress.completedItems, total: progress.totalItems, failed: progress.failedItems })}
          </p>
          <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-white/5">
            <div
              className={`h-full rounded-full bg-orange-500 transition-all ${progress.isPreparing ? "w-1/3 animate-pulse" : ""}`}
              style={progress.isPreparing ? undefined : { width: `${progress.percentage}%` }}
            />
          </div>
        </div>
      </div>
    </button>
  )
}
