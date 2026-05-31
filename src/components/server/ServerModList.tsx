import { ChevronRight, MapPinned, MinusCircle, PlusCircle } from "lucide-react"
import type { MouseEvent } from "react"

import { ModBadges } from "@/components/mods/ModBadges"
import type { ZomboidMod } from "@/types/mod"

type ServerModListProps = {
  title: string
  mods: ZomboidMod[]
  emptyMessage: string
  isExpanded: boolean
  action: "activate" | "deactivate"
  onToggleExpanded: () => void
  onAction: (mod: ZomboidMod) => void
  onInstallMap?: (mod: ZomboidMod) => void
  onContextMenu?: (event: MouseEvent<HTMLDivElement>, mod: ZomboidMod) => void
}

export function ServerModList({
  title,
  mods,
  emptyMessage,
  isExpanded,
  action,
  onToggleExpanded,
  onAction,
  onInstallMap,
  onContextMenu,
}: ServerModListProps) {
  const isActiveList = action === "deactivate"

  return (
    <section className="flex flex-col">
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
        {mods.map((mod) => (
          <div
            key={mod.id}
            onContextMenu={onContextMenu ? (event) => onContextMenu(event, mod) : undefined}
            className={`group rounded-2xl p-4 flex items-center justify-between transition-all ${
              isActiveList
                ? "bg-[#2b3238] border border-orange-400/20 hover:bg-[#353c42]"
                : "bg-[#2b3238]/50 border border-white/5 hover:bg-[#2b3238]"
            }`}
          >
            <div className={`flex items-center gap-4 min-w-0 ${isActiveList ? "" : "opacity-70 group-hover:opacity-100 transition-opacity"}`}>
              <div className="w-20 h-20 rounded-xl bg-[#1e2327] overflow-hidden shrink-0 border border-white/5 shadow-lg">
                {mod.imageUrl ? (
                  <img src={mod.imageUrl} alt={mod.name} className="w-full h-full object-cover transition-transform group-hover:scale-110" />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-white/5 font-black text-xs uppercase">
                    Sem Imagem
                  </div>
                )}
              </div>
              <div className="min-w-0">
                <p className="font-bold text-white truncate">{mod.name}</p>
                <p className="text-[10px] text-gray-500 font-mono truncate uppercase tracking-tighter">ID: {mod.id}</p>
                <div className="mt-2">
                  <ModBadges badges={mod.badges} />
                </div>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              {!isActiveList && onInstallMap && mod.mapNames && mod.mapNames.length > 0 && (
                <button
                  title="Instalar como mapa"
                  onClick={() => onInstallMap(mod)}
                  className="rounded-xl p-2 text-orange-400/60 transition-all hover:bg-orange-400/10 hover:text-orange-400"
                >
                  <MapPinned size={21} />
                </button>
              )}
              <button
                title={isActiveList ? "Desativar mod" : "Ativar mod"}
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
        ))}
        {mods.length === 0 && <p className="text-center text-gray-600 py-4 italic text-sm col-span-full">{emptyMessage}</p>}
      </div>
    </section>
  )
}
