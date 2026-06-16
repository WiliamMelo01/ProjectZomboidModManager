import { Folder, FolderPlus, RefreshCw } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { ModLocation } from "@/types/settings"

type ModLocationsSectionProps = {
  locations: ModLocation[]
  isAddingFolder: boolean
  isRescanning: boolean
  onAddFolder: () => void
  onRefresh: () => void
  onRescan: () => void
  onOpenLocation: (path: string) => void
}

export function ModLocationsSection({
  locations,
  isAddingFolder,
  isRescanning,
  onAddFolder,
  onRefresh,
  onRescan,
  onOpenLocation,
}: ModLocationsSectionProps) {
  const { t } = useTranslation()
  return (
    <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-6 shadow-xl relative group">
      <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />
      <div className="flex items-center justify-between mb-4 relative z-10">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
            <FolderPlus size={20} />
          </div>
          <div>
            <h3 className="text-xl font-bold text-white">{t("settings.modLocations.title")}</h3>
            <p className="text-xs text-gray-500">{t("settings.modLocations.description")}</p>
          </div>
        </div>
        <div className="flex flex-wrap justify-end gap-2">
          <button
            disabled={isAddingFolder}
            onClick={onAddFolder}
            className="flex items-center gap-2 bg-orange-500/10 text-orange-400 hover:bg-orange-500 hover:text-white disabled:opacity-60 px-4 py-2 rounded-xl transition-all font-bold text-sm border border-orange-500/20 active:scale-95"
          >
            {isAddingFolder ? <RefreshCw size={18} className="animate-spin" /> : <FolderPlus size={18} />}
            <span>{t("settings.modLocations.add")}</span>
          </button>
          <button onClick={onRefresh} className="flex items-center gap-2 bg-orange-500/10 text-orange-400 hover:bg-orange-500 hover:text-white px-4 py-2 rounded-xl transition-all font-bold text-sm border border-orange-500/20 active:scale-95">
            <RefreshCw size={18} />
            <span>{t("settings.modLocations.reload")}</span>
          </button>
          <button
            disabled={isRescanning}
            onClick={onRescan}
            className="flex items-center gap-2 bg-[#1e2327] text-gray-300 hover:bg-orange-500 hover:text-white disabled:opacity-60 px-4 py-2 rounded-xl transition-all font-bold text-sm border border-white/10 active:scale-95"
          >
            <RefreshCw size={18} className={isRescanning ? "animate-spin" : ""} />
            <span>{t("settings.modLocations.rescanAll")}</span>
          </button>
        </div>
      </div>

      <div className="space-y-3 relative z-10">
        <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">{t("settings.modLocations.saved")}</label>
        <div className="max-h-72 overflow-y-auto pr-2 custom-scrollbar">
          <div className="grid gap-2">
            {locations.length === 0 ? (
              <div className="bg-[#1e2327] border border-dashed border-white/5 rounded-2xl p-6 text-center">
                <p className="text-sm text-gray-600">{t("settings.modLocations.empty")}</p>
              </div>
            ) : (
              locations.map((location) => (
                <div key={`${location.kind}:${location.path}`} className="group/path flex items-center gap-3 bg-[#1e2327] border border-white/5 rounded-2xl p-3 pl-4 transition-all hover:border-orange-500/20">
                  <button
                    type="button"
                    disabled={!location.exists}
                    onClick={() => onOpenLocation(location.path)}
                    title={t("settings.modLocations.open")}
                    className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl text-gray-500 transition-colors hover:bg-orange-500/10 hover:text-orange-400 disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    <Folder size={18} />
                  </button>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-sm font-bold text-white">{location.label}</span>
                      <span className={`rounded-full border px-2 py-0.5 text-[10px] font-bold uppercase ${
                        location.exists
                          ? "border-green-500/20 bg-green-500/10 text-green-300"
                          : "border-red-500/20 bg-red-500/10 text-red-300"
                      }`}>
                        {location.exists ? t("settings.modLocations.found") : t("settings.modLocations.missing")}
                      </span>
                    </div>
                    <p className="mt-1 truncate font-mono text-xs text-gray-400">{location.path}</p>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </section>
  )
}
