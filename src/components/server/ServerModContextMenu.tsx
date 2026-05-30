import type { ZomboidMod } from "@/types/mod"

type ServerModContextMenuProps = {
  mod: ZomboidMod
  x: number
  y: number
  dependents: ZomboidMod[]
  onClose: () => void
  onMove: (position: "start" | "end") => void
}

export function ServerModContextMenu({ mod, x, y, dependents, onClose, onMove }: ServerModContextMenuProps) {
  const cannotMoveToEnd = dependents.length > 0

  return (
    <div className="fixed inset-0 z-50" onClick={onClose} onContextMenu={(event) => event.preventDefault()}>
      <div
        className="absolute w-56 overflow-hidden rounded-xl border border-white/10 bg-[#1e2327] py-2 shadow-2xl shadow-black/40"
        style={{ left: x, top: y }}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="border-b border-white/5 px-4 pb-2 pt-1">
          <p className="truncate text-xs font-bold text-white">{mod.name}</p>
          <p className="truncate text-[10px] font-mono text-gray-500">{mod.id}</p>
        </div>
        <button
          onClick={() => onMove("start")}
          className="w-full px-4 py-2.5 text-left text-sm font-medium text-gray-300 transition-colors hover:bg-orange-500/10 hover:text-orange-300"
        >
          Colocar no inicio
        </button>
        <button
          onClick={() => onMove("end")}
          disabled={cannotMoveToEnd}
          title={cannotMoveToEnd ? `Este mod e dependencia de ${dependents.length} mod(s) ativo(s).` : undefined}
          className={`w-full px-4 py-2.5 text-left text-sm font-medium transition-colors ${
            cannotMoveToEnd
              ? "cursor-not-allowed text-gray-600"
              : "text-gray-300 hover:bg-orange-500/10 hover:text-orange-300"
          }`}
        >
          Colocar no final
        </button>
        {cannotMoveToEnd && (
          <p className="border-t border-white/5 px-4 py-2 text-[10px] leading-relaxed text-orange-300/80">
            Este mod precisa carregar antes de outros mods ativos.
          </p>
        )}
      </div>
    </div>
  )
}
