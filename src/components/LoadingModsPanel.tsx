import { RefreshCw } from "lucide-react"
import { useTranslation } from "react-i18next"
import { Logo } from "./Logo"

type LoadingModsPanelProps = {
  error: string | null
  isLoading: boolean
  onRetry: () => Promise<void>
}

export function LoadingModsPanel({ error, isLoading, onRetry }: LoadingModsPanelProps) {
  const { t } = useTranslation()
  return (
    <div className="h-full bg-[#22272b] p-8 flex flex-col items-center justify-center text-white">
      <div className="flex flex-col items-center gap-6 max-w-md w-full text-center">
        <Logo size={64} iconOnly className="animate-pulse" />

        <div className="space-y-2">
          <h2 className="text-2xl font-bold">{t("mods.title")}</h2>
          <p className="text-gray-400 text-sm">
            {t("loadingMods.description")}
          </p>
        </div>

        <div className="w-full rounded-2xl border border-white/5 bg-[#2b3238] p-6 text-gray-300">
          <div className="flex items-center justify-center gap-3">
            <RefreshCw size={20} className={isLoading ? "animate-spin text-orange-400" : "text-gray-500"} />
            <span className="font-medium text-sm">
              {t(isLoading ? "loadingMods.processing" : "loadingMods.waiting")}
            </span>
          </div>

          {error && (
            <div className="mt-4 space-y-4">
              <div className="rounded-xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-xs text-red-300">
                {error}
              </div>
              <button
                onClick={() => void onRetry()}
                className="w-full rounded-xl bg-orange-500 py-2 text-sm font-bold text-white transition-all hover:bg-orange-600 active:scale-95 shadow-lg shadow-orange-500/20"
              >
                {t("common.retry")}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
