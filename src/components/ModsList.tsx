import { AlertCircle, CheckCircle2, Download, Info, RefreshCw, Search, X } from "lucide-react"
import { useState } from "react"

import { MissingDependencyModal } from "@/components/MissingDependencyModal"
import { ModCard } from "@/components/mods/ModCard"
import { buildInstallDependencyPlan, isLocalMod } from "@/lib/modDependencies"
import type { ZomboidMod } from "@/types/mod"

type ModsListProps = {
  mods: ZomboidMod[]
  isLoading: boolean
  error: string | null
  onRefresh: () => void
  onInstall: (mods: ZomboidMod[]) => Promise<void>
  onInstallAll: () => Promise<void>
  isInstallingAll: boolean
  onOpenSettings?: () => void
  searchQuery: string
  onSearchChange: (value: string) => void
}

export function ModsList({
  mods,
  isLoading,
  error,
  onRefresh,
  onInstall,
  onInstallAll,
  isInstallingAll,
  onOpenSettings,
  searchQuery,
  onSearchChange,
}: ModsListProps) {
  const [filterStatus, setFilterStatus] = useState<"all" | "local" | "steam">("all")
  const [pendingInstall, setPendingInstall] = useState<{ mod: ZomboidMod; dependencies: ZomboidMod[] } | null>(null)
  const [missingDependency, setMissingDependency] = useState<{ mod: ZomboidMod; dependencyId: string } | null>(null)
  const steamCount = mods.filter((mod) => !isLocalMod(mod)).length

  const normalizedSearch = searchQuery.trim().toLowerCase()
  const filteredMods = mods.filter((mod) => {
    const matchesSearch =
      !normalizedSearch ||
      mod.name.toLowerCase().includes(normalizedSearch) ||
      mod.id.toLowerCase().includes(normalizedSearch) ||
      mod.author.toLowerCase().includes(normalizedSearch) ||
      mod.description.toLowerCase().includes(normalizedSearch) ||
      mod.workshopId.includes(searchQuery) ||
      mod.dependencies?.some((dependency) => dependency.toLowerCase().includes(normalizedSearch))

    const matchesFilter =
      filterStatus === "all" ||
      (filterStatus === "local" && isLocalMod(mod)) ||
      (filterStatus === "steam" && !isLocalMod(mod))

    return matchesSearch && matchesFilter
  })

  const handleInstallClick = async (mod: ZomboidMod) => {
    const dependencyPlan = buildInstallDependencyPlan(mod, mods)

    if (dependencyPlan.missingDependencyId) {
      setMissingDependency({ mod, dependencyId: dependencyPlan.missingDependencyId })
      return
    }

    if (dependencyPlan.dependenciesToInstall.length > 0) {
      setPendingInstall({ mod, dependencies: dependencyPlan.dependenciesToInstall })
    } else {
      await onInstall([mod])
    }
  }

  const confirmBulkInstall = async () => {
    if (pendingInstall) {
      await onInstall([...pendingInstall.dependencies, pendingInstall.mod])
      setPendingInstall(null)
    }
  }

  return (
    <div className="p-8 h-full flex flex-col gap-6 relative">
      <div className="flex flex-col xl:flex-row justify-between items-start xl:items-center gap-6">
        <div>
          <h2 className="text-3xl font-bold tracking-tight text-white">Workshop de Mods</h2>
          <p className="text-gray-400 mt-1">Mods encontrados na pasta local do Zomboid e na Steam.</p>
        </div>

        <div className="flex w-full flex-col gap-4 md:flex-row md:items-center xl:w-auto">
          {/* Filtro de Status */}
          <div className="flex bg-[#2b3238] p-1 rounded-xl border border-white/5 shadow-inner">
            <button
              onClick={() => setFilterStatus("all")}
              className={`px-4 py-1.5 rounded-lg text-xs font-bold transition-all ${
                filterStatus === "all" ? "bg-orange-500 text-white shadow-lg" : "text-gray-400 hover:text-white"
              }`}
            >
              Todos
            </button>
            <button
              onClick={() => setFilterStatus("local")}
              className={`px-4 py-1.5 rounded-lg text-xs font-bold transition-all ${
                filterStatus === "local" ? "bg-orange-500 text-white shadow-lg" : "text-gray-400 hover:text-white"
              }`}
            >
              Local
            </button>
            <button
              onClick={() => setFilterStatus("steam")}
              className={`px-4 py-1.5 rounded-lg text-xs font-bold transition-all ${
                filterStatus === "steam" ? "bg-orange-500 text-white shadow-lg" : "text-gray-400 hover:text-white"
              }`}
            >
              Steam
            </button>
          </div>

          <div className="relative w-full md:w-64 group">
            <Search
              className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within:text-orange-400 transition-colors"
              size={18}
            />
            <input
              type="text"
              placeholder="Buscar mods..."
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              className="w-full bg-[#2b3238] border border-white/5 rounded-xl py-2.5 pl-10 pr-4 text-sm focus:outline-none focus:border-orange-400/50 transition-all placeholder:text-gray-600"
            />
          </div>

          <button
            className="flex items-center justify-center gap-2 bg-[#2b3238] border border-white/5 text-gray-300 hover:text-white hover:border-orange-400/30 px-4 py-2.5 rounded-xl transition-all"
            onClick={onRefresh}
          >
            <RefreshCw size={18} className={isLoading ? "animate-spin" : ""} />
            <span className="hidden md:inline">Atualizar</span>
          </button>

          <button
            disabled={isLoading || isInstallingAll || steamCount === 0}
            className={`flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl transition-all font-bold text-sm ${
              isLoading || isInstallingAll || steamCount === 0
                ? "bg-white/5 text-gray-500 border border-white/5 cursor-not-allowed"
                : "bg-orange-500 text-white hover:bg-orange-600 shadow-lg shadow-orange-500/20"
            }`}
            onClick={() => void onInstallAll()}
          >
            {isInstallingAll ? <RefreshCw size={18} className="animate-spin" /> : <Download size={18} />}
            <span>Trazer Steam para local</span>
            {steamCount > 0 && <span className="text-xs opacity-80">({steamCount})</span>}
          </button>
        </div>
      </div>

      {error && (
        <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
          {error}
        </div>
      )}

      {isLoading && (
        <div className="rounded-2xl border border-white/5 bg-[#2b3238] px-5 py-4 text-sm text-gray-300">
          Buscando mods em Zomboid/mods e nas bibliotecas Steam...
        </div>
      )}

      <div className="flex-1 overflow-y-auto custom-scrollbar pr-2">
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6 pb-8">
          {filteredMods.map((mod) => (
            <ModCard
              key={`${mod.source}:${mod.workshopId}:${mod.id}:${mod.path}`}
              mod={mod}
              onInstall={() => handleInstallClick(mod)}
            />
          ))}

          {!isLoading && filteredMods.length === 0 && (
            <div className="col-span-full flex flex-col items-center justify-center py-20 text-gray-500 bg-[#2b3238]/30 rounded-3xl border-2 border-dashed border-white/5">
              <Info size={48} className="mb-4 opacity-20" />
              <p className="text-lg font-medium">Nenhum mod encontrado</p>
              <p className="text-sm">Tente buscar por outro nome, autor, Mod ID, dependencia ou Workshop ID.</p>
            </div>
          )}
        </div>
      </div>

      {/* Dependency Modal (Found in Library) */}
      {pendingInstall && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
          <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
            <div className="p-6 border-b border-white/5 flex justify-between items-center">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-orange-500/20 text-orange-400 rounded-xl">
                  <AlertCircle size={24} />
                </div>
                <h3 className="text-xl font-bold text-white">Dependências</h3>
              </div>
              <button
                onClick={() => setPendingInstall(null)}
                className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors"
              >
                <X size={20} />
              </button>
            </div>

            <div className="p-6">
              <p className="text-gray-400 text-sm mb-4">
                O mod <span className="text-white font-bold">{pendingInstall.mod.name}</span> requer os seguintes mods
                adicionais para funcionar corretamente:
              </p>

              <div className="space-y-3 mb-6 max-h-48 overflow-y-auto custom-scrollbar pr-2">
                {pendingInstall.dependencies.map((dep) => (
                  <div key={dep.id} className="flex items-center gap-3 p-3 bg-[#2b3238] border border-white/5 rounded-xl">
                    <div className="w-10 h-10 rounded-lg bg-[#1e2327] overflow-hidden shrink-0">
                      {dep.imageUrl ? (
                        <img src={dep.imageUrl} alt={dep.name} className="w-full h-full object-cover" />
                      ) : (
                        <div className="w-full h-full flex items-center justify-center">
                          <Download size={16} className="text-white/10" />
                        </div>
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-bold text-white truncate">{dep.name}</p>
                      <p className="text-[10px] text-gray-500 font-mono truncate">{dep.id}</p>
                    </div>
                  </div>
                ))}
              </div>

              <div className="flex flex-col gap-3">
                <button
                  onClick={confirmBulkInstall}
                  className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 flex items-center justify-center gap-2"
                >
                  <CheckCircle2 size={18} />
                  Trazer tudo para local
                </button>
                <button
                  onClick={() => setPendingInstall(null)}
                  className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all"
                >
                  Cancelar
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Missing Dependency Modal (Not in Library) */}
      {missingDependency && (
        <MissingDependencyModal
          mod={missingDependency.mod}
          dependencyId={missingDependency.dependencyId}
          onClose={() => setMissingDependency(null)}
          onDownloaded={onRefresh}
          onOpenSettings={onOpenSettings}
        />
      )}
    </div>
  )
}
