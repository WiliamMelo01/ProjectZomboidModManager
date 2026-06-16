import { AlertTriangle, ChevronLeft, ChevronRight, MapPinned, MinusCircle, PlusCircle } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"
import type { MouseEvent } from "react"

import { getModImageSrc } from "@/lib/modImages"
import type { ZomboidMod } from "@/types/mod"

type ServerModListProps = {
  title: string
  mods: ZomboidMod[]
  emptyMessage: string
  isExpanded: boolean
  action: "activate" | "deactivate"
  onToggleExpanded: () => void
  onAction: (mod: ZomboidMod) => void
  onSelect: (mod: ZomboidMod) => void
  onInstallMap?: (mod: ZomboidMod) => void
  onContextMenu?: (event: MouseEvent<HTMLDivElement>, mod: ZomboidMod) => void
  incompatibleModIds?: Set<string>
  paginate?: boolean
  paginationResetKey?: string
}

const MODS_PER_PAGE = 30

export function ServerModList({
  title,
  mods,
  emptyMessage,
  isExpanded,
  action,
  onToggleExpanded,
  onAction,
  onSelect,
  onInstallMap,
  onContextMenu,
  incompatibleModIds = new Set(),
  paginate = false,
  paginationResetKey = "",
}: ServerModListProps) {
  const { t } = useTranslation()
  const isActiveList = action === "deactivate"
  const [currentPage, setCurrentPage] = useState(1)
  const sectionRef = useRef<HTMLElement>(null)
  const totalPages = Math.max(1, Math.ceil(mods.length / MODS_PER_PAGE))
  const visibleMods = paginate
    ? mods.slice((currentPage - 1) * MODS_PER_PAGE, currentPage * MODS_PER_PAGE)
    : mods

  useEffect(() => {
    setCurrentPage(1)
  }, [paginationResetKey])

  useEffect(() => {
    setCurrentPage((page) => Math.min(page, totalPages))
  }, [totalPages])

  const changePage = (page: number) => {
    setCurrentPage(page)
    sectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" })
  }

  return (
    <section ref={sectionRef} className="flex scroll-mt-6 flex-col">
      <button
        onClick={onToggleExpanded}
        className="flex items-center gap-3 mb-4 px-2 py-2 hover:bg-white/5 rounded-xl transition-colors w-full text-left group"
      >
        <h3 className="text-lg font-bold text-white uppercase tracking-tighter">{title}</h3>
        <div className="h-px flex-1 bg-white/5" />
        <span className="text-xs font-mono text-gray-500 bg-[#2b3238] px-2 py-0.5 rounded-full">{mods.length}</span>
        <ChevronRight
          size={20}
          className={`text-gray-500 transition-transform duration-300 ${isExpanded ? "rotate-90" : ""}`}
        />
      </button>

      <div className={`grid grid-cols-1 md:grid-cols-2 gap-4 transition-all duration-300 origin-top ${
        isExpanded ? "opacity-100 scale-y-100 h-auto" : "opacity-0 scale-y-0 h-0 overflow-hidden"
      }`}>
        {visibleMods.map((mod) => {
          const isIncompatible = incompatibleModIds.has(mod.id.toLowerCase())
          const imageSrc = getModImageSrc(mod.imageUrl)

          return (
          <div
            key={mod.id}
            onContextMenu={onContextMenu ? (event) => onContextMenu(event, mod) : undefined}
            className={`group rounded-2xl p-4 flex items-center justify-between transition-all ${
              isIncompatible
                ? "bg-red-500/5 border border-red-500/30 hover:bg-red-500/10"
                : isActiveList
                ? "bg-[#2b3238] border border-orange-400/20 hover:bg-[#353c42]"
                : "bg-[#2b3238]/50 border border-white/5 hover:bg-[#2b3238]"
            }`}
          >
            <button
              type="button"
              onClick={() => onSelect(mod)}
              className={`flex min-w-0 flex-1 items-center gap-4 text-left ${isActiveList ? "" : "opacity-70 transition-opacity group-hover:opacity-100"}`}
            >
              <div className="w-20 h-20 rounded-xl bg-[#1e2327] overflow-hidden shrink-0 border border-white/5 shadow-lg">
                {imageSrc ? (
                  <img src={imageSrc} alt={mod.name} className="w-full h-full object-cover transition-transform group-hover:scale-110" />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-white/5 font-black text-xs uppercase">
                    {t("mods.noImage")}
                  </div>
                )}
              </div>
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <p className="font-bold text-white truncate">{mod.name}</p>
                  {mod.mapNames && mod.mapNames.length > 0 && (
                    <span className="flex shrink-0 items-center gap-1 rounded-full border border-orange-400/20 bg-orange-400/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide text-orange-300">
                      <MapPinned size={11} />
                      {t("mods.map")}
                    </span>
                  )}
                </div>
                <p className="text-[10px] text-gray-500 font-mono truncate uppercase tracking-tighter">ID: {mod.id}</p>
                <div className="mt-2 flex flex-wrap gap-1">
                  {mod.compatibleBuilds.map((build) => (
                    <span key={build} className="rounded-full border border-white/10 bg-black/20 px-2 py-0.5 text-[9px] font-black uppercase text-gray-300">
                      {build}
                    </span>
                  ))}
                  {isIncompatible && (
                    <span className="flex items-center gap-1 rounded-full border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[9px] font-black uppercase text-red-300">
                      <AlertTriangle size={10} />
                      {t("mods.incompatible")}
                    </span>
                  )}
                  {mod.source === "missing" && (
                    <span className="rounded-full border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[9px] font-black uppercase text-red-300">
                      {t("mods.missing")}
                    </span>
                  )}
                </div>
              </div>
            </button>
            <div className="flex shrink-0 items-center gap-1">
              {!isActiveList && onInstallMap && mod.mapNames && mod.mapNames.length > 0 && (
                <button
                  title={t("mods.installMap")}
                  onClick={() => onInstallMap(mod)}
                  className="rounded-xl p-2 text-orange-400/60 transition-all hover:bg-orange-400/10 hover:text-orange-400"
                >
                  <MapPinned size={21} />
                </button>
              )}
              <button
                title={isActiveList ? t("mods.deactivate") : t("mods.activate")}
                onClick={() => onAction(mod)}
                className={`p-2 rounded-xl transition-all ${
                  isActiveList
                    ? "text-red-400/50 hover:text-red-400 hover:bg-red-400/10"
                    : "text-green-400/50 hover:text-green-400 hover:bg-green-400/10"
                }`}
              >
                {isActiveList ? <MinusCircle size={22} /> : <PlusCircle size={22} />}
              </button>
            </div>
          </div>
          )
        })}
        {mods.length === 0 && <p className="text-center text-gray-600 py-4 italic text-sm col-span-full">{emptyMessage}</p>}
      </div>
      {paginate && mods.length > MODS_PER_PAGE && (
        <div className="mt-4 flex items-center justify-center gap-4 border-t border-white/5 pt-4">
          <button
            type="button"
            disabled={currentPage === 1}
            onClick={() => changePage(Math.max(1, currentPage - 1))}
            className="flex items-center gap-2 rounded-xl border border-white/5 bg-[#2b3238] px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:border-orange-400/30 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            <ChevronLeft size={16} />
            {t("common.previous")}
          </button>
          <span className="text-sm text-gray-400">
            {t("library.page", { current: currentPage, total: totalPages })}
          </span>
          <button
            type="button"
            disabled={currentPage === totalPages}
            onClick={() => changePage(Math.min(totalPages, currentPage + 1))}
            className="flex items-center gap-2 rounded-xl border border-white/5 bg-[#2b3238] px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:border-orange-400/30 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("common.next")}
            <ChevronRight size={16} />
          </button>
        </div>
      )}
    </section>
  )
}
