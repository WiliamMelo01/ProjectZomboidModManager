import { AlertCircle, Download, Hash, PackageCheck, User } from "lucide-react"
import { useTranslation } from "react-i18next"

import { isLocalMod } from "@/lib/modDependencies"
import type { ZomboidMod } from "@/types/mod"

type ModCardProps = {
  mod: ZomboidMod
  onInstall: () => void
}

export function ModCard({ mod, onInstall }: ModCardProps) {
  const { t } = useTranslation()
  const isLocal = isLocalMod(mod)
  const sourceLabel = isLocal ? "LOCAL" : "STEAM"
  const displayWorkshopId = mod.workshopId || "-"
  const hasDependencies = mod.dependencies && mod.dependencies.length > 0

  return (
    <div className="group bg-[#2b3238] border border-white/5 rounded-2xl flex flex-col transition-all duration-300 hover:border-orange-400/30 hover:bg-[#353c42] hover:shadow-[0_10px_30px_rgba(0,0,0,0.2)] overflow-hidden">
      <div className="relative h-40 w-full bg-[#1e2327] overflow-hidden shrink-0">
        {mod.imageUrl ? (
          <img
            src={mod.imageUrl}
            alt={mod.name}
            className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-[#2b3238] to-[#1e2327]">
            <Download size={48} className="text-white/5" />
          </div>
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-[#2b3238] to-transparent opacity-60" />

        <div className="absolute top-3 left-3">
          <span className="text-[10px] text-white font-bold bg-orange-500 px-2 py-0.5 rounded-md shadow-lg">
            {sourceLabel}
          </span>
        </div>
        <div className="absolute bottom-3 left-3 flex gap-1">
          {mod.compatibleBuilds.map((build) => (
            <span key={build} className="rounded-md border border-white/10 bg-black/50 px-2 py-0.5 text-[10px] font-black uppercase text-orange-200">
              {build}
            </span>
          ))}
        </div>

        <div className="absolute top-3 right-3">
          <span className="text-[10px] text-gray-300 font-mono bg-black/40 backdrop-blur-md px-2 py-0.5 rounded-md border border-white/10">
            v{mod.version}
          </span>
        </div>
      </div>

      <div className="p-5 flex flex-col flex-1">
        <div className="flex justify-between items-start mb-4">
          <div className="flex-1 min-w-0">
            <h3 className="text-lg font-bold text-white group-hover:text-orange-400 transition-colors truncate">
              {mod.name}
            </h3>
            <div className="flex items-center gap-2 mt-1">
              <div className="flex items-center gap-1 text-xs text-gray-500">
                <User size={12} />
                <span>{t("mods.by")} {mod.author}</span>
              </div>
              {hasDependencies && (
                <div className="flex items-center gap-1 text-[10px] text-orange-400/80 bg-orange-400/5 px-2 py-0.5 rounded-full border border-orange-400/10">
                  <AlertCircle size={10} />
                  <span>{t("mods.requiresDependencies")}</span>
                </div>
              )}
            </div>
          </div>
          {isLocal && (
            <span className="flex items-center gap-1 bg-green-500/10 text-green-400 text-[10px] font-bold px-2 py-0.5 rounded-full border border-green-500/20 shrink-0 ml-2">
              <PackageCheck size={12} />
              LOCAL
            </span>
          )}
        </div>

        <p className="text-xs text-gray-400 line-clamp-2 mb-6 h-8">{mod.description}</p>
        <div className="grid grid-cols-2 gap-3 mb-3">
          <div className="bg-[#22272b] p-2 rounded-lg border border-white/5">
            <p className="text-[10px] text-gray-500 uppercase font-bold tracking-tighter">Workshop ID</p>
            <div className="flex items-center gap-1.5 mt-0.5">
              <Hash size={12} className="text-orange-400" />
              <span className="text-xs font-mono text-gray-300 truncate">{displayWorkshopId}</span>
            </div>
          </div>
          <div className="bg-[#22272b] p-2 rounded-lg border border-white/5">
            <p className="text-[10px] text-gray-500 uppercase font-bold tracking-tighter">Mod ID</p>
            <div className="flex items-center gap-1.5 mt-0.5">
              <PackageCheck size={12} className="text-orange-400" />
              <span className="text-xs font-mono text-gray-300 truncate">{mod.id}</span>
            </div>
          </div>
        </div>

        <div className="bg-[#22272b] p-2 rounded-lg border border-white/5 mb-6">
          <p className="text-[10px] text-gray-500 uppercase font-bold tracking-tighter text-center">{t("mods.size")}</p>
          <p className="text-xs font-mono text-gray-300 mt-0.5 text-center">{mod.size}</p>
        </div>

        <button
          disabled={isLocal}
          onClick={onInstall}
          className={`w-full py-3 rounded-xl font-bold text-sm transition-all duration-300 flex items-center justify-center gap-2 mt-auto ${
            isLocal
              ? "bg-white/5 text-gray-500 cursor-not-allowed border border-white/5"
              : "bg-orange-500 text-white hover:bg-orange-600 hover:shadow-[0_4px_15_rgba(249,115,22,0.3)] active:scale-[0.98]"
          }`}
        >
          {isLocal ? t("mods.installed") : t("mods.install")}
        </button>
      </div>
    </div>
  )
}
