import { AlertCircle, Download, Hash, PackageCheck, User, Trash2 } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import { getModImageSrc } from "@/lib/modImages"
import { isLocalMod } from "@/lib/modDependencies"
import { isModInCloud } from "@/lib/serverMods"
import type { ZomboidMod } from "@/types/mod"

type ModCardProps = {
  mod: ZomboidMod
  onInstall?: () => void
  onDelete?: () => void
  isReadOnly?: boolean
  onSelect?: (mod: ZomboidMod) => void
  workshopMappings?: Record<string, string>
}

export function ModCard({
  mod,
  onInstall,
  onDelete,
  isReadOnly = false,
  onSelect,
  workshopMappings = {},
}: ModCardProps) {
  const { t } = useTranslation()
  const isLocal = isLocalMod(mod)
  const sourceBadge = getSourceBadge(mod)
  const resolvedWorkshopId = mod.workshopId?.trim() || workshopMappings[mod.id] || workshopMappings[mod.id.toLowerCase()] || "-"
  const isFoundInCloud = isModInCloud(mod, workshopMappings)
  const hasDependencies = mod.dependencies && mod.dependencies.length > 0
  const imageSrc = getModImageSrc(mod.imageUrl)

  return (
    <div
      onClick={() => onSelect?.(mod)}
      className="group bg-[#2b3238] border border-white/5 rounded-2xl flex flex-col transition-all duration-300 hover:border-orange-400/30 hover:bg-[#353c42] hover:shadow-[0_10px_30px_rgba(0,0,0,0.2)] overflow-hidden cursor-pointer"
    >
      <div className="relative h-40 w-full bg-[#1e2327] overflow-hidden shrink-0">
        {imageSrc ? (
          <img
            src={imageSrc}
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
          <span className={`rounded-md px-2 py-0.5 text-[10px] font-bold text-white shadow-lg ${sourceBadge.className}`}>
            {sourceBadge.label}
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
          <div className="bg-[#22272b] p-2 rounded-lg border border-white/5 flex flex-col justify-between">
            <div>
              <p className="text-[10px] text-gray-500 uppercase font-bold tracking-tighter">Workshop ID</p>
              <div className="flex items-center gap-1.5 mt-0.5">
                <Hash size={12} className="text-orange-400 shrink-0" />
                <span className="text-xs font-mono text-gray-300 truncate">{resolvedWorkshopId}</span>
              </div>
            </div>
            <div className="mt-1.5">
              {isFoundInCloud ? (
                <span className="inline-flex items-center gap-1 text-[9px] font-bold text-sky-300 bg-sky-500/10 border border-sky-500/20 px-1.5 py-0.5 rounded">
                  ☁ Found in Cloud Database
                </span>
              ) : (
                <span className="inline-flex items-center gap-1 text-[9px] font-bold text-amber-300 bg-amber-500/10 border border-amber-500/20 px-1.5 py-0.5 rounded">
                  ⚠ Not found in cloud database
                </span>
              )}
            </div>
          </div>
          <div className="bg-[#22272b] p-2 rounded-lg border border-white/5">
            <p className="text-[10px] text-gray-500 uppercase font-bold tracking-tighter">Mod ID</p>
            <div className="flex items-center gap-1.5 mt-0.5">
              <PackageCheck size={12} className="text-orange-400 shrink-0" />
              <span className="text-xs font-mono text-gray-300 truncate">{mod.id}</span>
            </div>
          </div>
        </div>

        <div className="bg-[#22272b] p-2 rounded-lg border border-white/5 mb-6">
          <p className="text-[10px] text-gray-500 uppercase font-bold tracking-tighter text-center">{t("mods.size")}</p>
          <p className="text-xs font-mono text-gray-300 mt-0.5 text-center">{mod.size}</p>
        </div>

        <div className="flex gap-2 mt-auto">
          <button
            disabled={isLocal || isReadOnly}
            onClick={(e) => {
              e.stopPropagation()
              onInstall?.()
            }}
            className={`flex-1 py-3 rounded-xl font-bold text-sm transition-all duration-300 flex items-center justify-center gap-2 ${
              isLocal || isReadOnly
                ? "bg-white/5 text-gray-500 cursor-not-allowed border border-white/5"
                : "bg-orange-500 text-white hover:bg-orange-600 hover:shadow-[0_4px_15_rgba(249,115,22,0.3)] active:scale-[0.98]"
            }`}
          >
            {isLocal ? t("mods.installed") : isReadOnly ? "Remote" : t("mods.install")}
          </button>
          {onDelete && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                onDelete()
              }}
              className="px-4 py-3 rounded-xl bg-red-500/10 text-red-400 hover:bg-red-500 hover:text-white transition-all border border-red-500/20 active:scale-[0.98]"
              title={t("common.delete", "Excluir")}
            >
              <Trash2 size={16} />
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

function getSourceBadge(mod: ZomboidMod) {
  if (isLocalMod(mod)) {
    return { label: "LOCAL", className: "bg-green-500" }
  }

  if (mod.source === "steamcmd") {
    return { label: "STEAMCMD", className: "bg-sky-500" }
  }

  return { label: "STEAM", className: "bg-orange-500" }
}
