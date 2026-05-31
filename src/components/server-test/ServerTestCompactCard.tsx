import { AlertTriangle, CheckCircle2, Maximize2, RefreshCw, XCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { formatDuration, getServerTestStatusStyle } from "@/lib/serverTest"
import type { ServerTestResult } from "@/types/serverTest"

type ServerTestCompactCardProps = {
  serverId: string | null
  isTesting: boolean
  result: ServerTestResult | null
  elapsedSeconds: number
  logLineCount: number
  hasDownloadProgressCard: boolean
  onExpand: () => void
}

export function ServerTestCompactCard({
  serverId,
  isTesting,
  result,
  elapsedSeconds,
  logLineCount,
  hasDownloadProgressCard,
  onExpand,
}: ServerTestCompactCardProps) {
  const { t } = useTranslation()
  const statusStyle = getServerTestStatusStyle(result?.status, isTesting)

  return (
    <button
      onClick={onExpand}
      className={`fixed right-6 z-40 w-80 rounded-2xl border border-white/10 bg-[#22272b] p-4 text-left shadow-2xl shadow-black/50 transition-all hover:bg-[#293036] ${
        hasDownloadProgressCard ? "bottom-[144px]" : "bottom-6"
      }`}
    >
      <div className="flex items-center gap-3">
        <div className={`shrink-0 rounded-xl p-2 ${statusStyle.iconBg}`}>
          {isTesting ? (
            <RefreshCw size={18} className="animate-spin text-orange-400" />
          ) : result?.status === "passed" ? (
            <CheckCircle2 size={18} className="text-green-400" />
          ) : result?.status === "failed" ? (
            <XCircle size={18} className="text-red-400" />
          ) : (
            <AlertTriangle size={18} className="text-orange-400" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-black text-white">{t("serverTest.title")}</p>
          <p className="mt-1 truncate text-xs text-gray-400">
            {serverId ?? t("serverTest.profile")} · {formatDuration(elapsedSeconds)} · {logLineCount} {t("serverTest.lines")}
          </p>
        </div>
        <Maximize2 size={16} className="text-gray-500" />
      </div>
    </button>
  )
}
