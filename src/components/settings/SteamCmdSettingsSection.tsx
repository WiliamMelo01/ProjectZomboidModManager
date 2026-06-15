import { CheckCircle2, Folder, Gauge, Search, XCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

type SteamCmdSettingsSectionProps = {
  path: string
  resolvedPath: string | null
  isConfigured: boolean
  maxConcurrentDownloads: number
  onPathChange: (path: string) => void
  onMaxConcurrentDownloadsChange: (value: number) => void
  onBrowse: () => void
  onDetect: () => void
}

export function SteamCmdSettingsSection({
  path,
  resolvedPath,
  isConfigured,
  maxConcurrentDownloads,
  onPathChange,
  onMaxConcurrentDownloadsChange,
  onBrowse,
  onDetect,
}: SteamCmdSettingsSectionProps) {
  const { t } = useTranslation()
  return (
    <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-8 shadow-xl relative group">
      <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />
      <div className="flex items-center gap-3 mb-6 relative z-10">
        <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
          <Folder size={20} />
        </div>
        <div>
          <h3 className="text-xl font-bold text-white">{t("settings.steamcmd.title")}</h3>
          <p className="text-xs text-gray-500">{t("settings.steamcmd.description")}</p>
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
              {isConfigured ? t("settings.steamcmd.configured") : t("settings.steamcmd.notConfigured")}
            </p>
            <p className="text-xs text-gray-500 break-all">
              {resolvedPath || t("settings.steamcmd.hint")}
            </p>
          </div>
        </div>
      </div>

      <div className="space-y-3 relative z-10">
        <label htmlFor="steamcmd-path" className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
          {t("settings.steamcmd.executablePath")}
        </label>
        <div className="flex flex-col gap-3 md:flex-row">
          <div className="relative flex-1 group/input">
            <Folder size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within/input:text-orange-400 transition-colors" />
            <input
              id="steamcmd-path"
              type="text"
              value={path}
              onChange={(event) => onPathChange(event.target.value)}
              placeholder="C:\steamcmd\steamcmd.exe"
              className="w-full bg-[#1e2327] border border-white/5 rounded-2xl py-3.5 pl-12 pr-4 text-sm focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-700"
            />
          </div>
          <button onClick={onBrowse} className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95">
            <Folder size={18} />
            {t("settings.steamcmd.browse")}
          </button>
          <button onClick={onDetect} className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95">
            <Search size={18} />
            {t("settings.steamcmd.detect")}
          </button>
        </div>
        <p className="text-xs text-gray-500">
          {t("settings.steamcmd.emptyHint")}
        </p>

        <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
          <div className="mb-3 flex items-start gap-3">
            <Gauge size={18} className="mt-0.5 shrink-0 text-orange-400" />
            <div>
              <p className="text-sm font-bold text-white">{t("settings.steamcmd.concurrentDownloads")}</p>
              <p className="text-xs text-gray-500">{t("settings.steamcmd.concurrentDownloadsHint")}</p>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-2">
            {[1, 2, 3].map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => onMaxConcurrentDownloadsChange(value)}
                className={`rounded-xl border px-4 py-3 text-sm font-black transition-all ${
                  maxConcurrentDownloads === value
                    ? "border-orange-500/40 bg-orange-500 text-white"
                    : "border-white/10 bg-[#2b3238] text-gray-400 hover:border-orange-500/30 hover:text-white"
                }`}
              >
                {value}
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}
