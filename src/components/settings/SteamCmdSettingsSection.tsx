import { CheckCircle2, Folder, Gauge, XCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

type SteamCmdSettingsSectionProps = {
  resolvedPath: string | null
  isConfigured: boolean
  maxConcurrentDownloads: number
}

export function SteamCmdSettingsSection({
  resolvedPath,
  isConfigured,
  maxConcurrentDownloads,
}: SteamCmdSettingsSectionProps) {
  const { t } = useTranslation()
  return (
    <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-6 shadow-xl relative group">
      <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />
      <div className="flex items-center gap-3 mb-4 relative z-10">
        <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
          <Folder size={20} />
        </div>
        <div>
          <h3 className="text-xl font-bold text-white">{t("settings.steamcmd.title")}</h3>
          <p className="text-xs text-gray-500">{t("settings.steamcmd.description")}</p>
        </div>
      </div>

      <div className="mb-4 rounded-2xl border border-white/5 bg-[#1e2327] p-4 relative z-10">
        <div className="flex items-start gap-3">
          {isConfigured ? (
            <CheckCircle2 size={20} className="text-green-400 shrink-0 mt-0.5" />
          ) : (
            <XCircle size={20} className="text-red-400 shrink-0 mt-0.5" />
          )}
          <div className="min-w-0">
            <p className="text-sm font-bold text-white">
              {isConfigured ? t("settings.steamcmd.configured") : t("settings.steamcmd.notConfigured")}
            </p>
            <p className="text-xs text-gray-500 break-all">
              {resolvedPath || t("settings.steamcmd.hint")}
            </p>
          </div>
        </div>
      </div>

      <div className="relative z-10">
        <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4 flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
              <Gauge size={16} />
            </div>
            <div>
              <p className="text-sm font-bold text-white">{t("settings.steamcmd.concurrentDownloads")}</p>
              <p className="text-xs text-gray-500">{t("settings.steamcmd.concurrentDownloadsHint")}</p>
            </div>
          </div>
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-orange-500/10 border border-orange-500/20">
            <span className="text-sm font-black text-orange-400">{maxConcurrentDownloads}</span>
            <span className="text-[10px] font-bold text-orange-400/60 uppercase tracking-tighter">Instance</span>
          </div>
        </div>
      </div>
    </section>
  )
}
