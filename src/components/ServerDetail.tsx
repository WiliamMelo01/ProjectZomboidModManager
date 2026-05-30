import { AlertCircle, AlertTriangle, ArrowLeft, Check, CheckCircle2, Info, Play, PlusCircle, RefreshCw, Search, Server, Trash2, X } from "lucide-react"
import { useState } from "react"

import { MissingDependencyModal } from "@/components/MissingDependencyModal"
import { ServerModList } from "@/components/server/ServerModList"
import { ServerPortConflictModal } from "@/components/server/ServerPortConflictModal"
import { buildActivationDependencyPlan, isLocalMod, normalizeModId } from "@/lib/modDependencies"
import { invokeTauri } from "@/lib/tauri"
import type { ZomboidMod } from "@/types/mod"
import type { ZomboidServer } from "@/types/server"
import type { ServerPortCheck } from "@/components/server/ServerPortConflictModal"

type ServerDetailProps = {
  server: ZomboidServer | null
  allMods?: ZomboidMod[]
  onBack: () => void
  onInstallMods: (mods: ZomboidMod[]) => Promise<void>
  onActivateMods: (mods: ZomboidMod[]) => Promise<void>
  onToggleMod: (mod: ZomboidMod, action: "activate" | "deactivate") => Promise<void>
  onMoveActiveMod: (mod: ZomboidMod, position: "start" | "end") => Promise<void>
  onRefreshMods?: () => Promise<void>
  onDependencyDownloaded?: (dependencyId: string) => Promise<void>
  onOpenSettings?: () => void
  runningServerTestId?: string | null
}

type PendingActivation = {
  mod: ZomboidMod
  dependenciesToInstall: ZomboidMod[]
  dependenciesToActivate: ZomboidMod[]
  modNeedsInstall: boolean
}

const MOVE_MOD_WARNING_KEY = "pzmm_move_mod_warning_modal_seen"

function matchesSearch(mod: ZomboidMod, search: string) {
  const normalizedSearch = search.trim().toLowerCase()

  if (!normalizedSearch) {
    return true
  }

  return (
    String(mod.name ?? "").toLowerCase().includes(normalizedSearch) ||
    String(mod.id ?? "").toLowerCase().includes(normalizedSearch)
  )
}

export function ServerDetail({
  server,
  allMods = [],
  onBack,
  onInstallMods,
  onActivateMods,
  onToggleMod,
  onMoveActiveMod,
  onRefreshMods,
  onDependencyDownloaded,
  onOpenSettings,
  runningServerTestId,
}: ServerDetailProps) {
  const [search, setSearch] = useState("")
  const [confirmDelete, setConfirmDelete] = useState<ZomboidMod | null>(null)
  const [dependencyWarning, setDependencyWarning] = useState<{ mod: ZomboidMod; dependents: ZomboidMod[] } | null>(null)
  const [missingDependency, setMissingDependency] = useState<{ mod: ZomboidMod; dependencyId: string } | null>(null)
  const [pendingActivation, setPendingActivation] = useState<PendingActivation | null>(null)
  const [contextMenu, setContextMenu] = useState<{ mod: ZomboidMod; x: number; y: number } | null>(null)
  const [showMoveWarning, setShowMoveWarning] = useState<{ mod: ZomboidMod; position: "start" | "end" } | null>(null)
  const [dontShowAgainMove, setDontShowAgainMove] = useState(false)
  const [isTestingServer, setIsTestingServer] = useState(false)
  const [portConflictCheck, setPortConflictCheck] = useState<ServerPortCheck | null>(null)
  const [isCheckingPorts, setIsCheckingPorts] = useState(false)
  const [isKillingPorts, setIsKillingPorts] = useState(false)

  const [isActivatedExpanded, setIsActivatedExpanded] = useState(true)
  const [isAvailableExpanded, setIsAvailableExpanded] = useState(true)

  if (!server) {
    return (
      <div className="h-full min-h-0 overflow-y-auto bg-[#22272b] p-8 text-white custom-scrollbar">
        <button
          onClick={onBack}
          className="flex items-center gap-2 text-gray-400 hover:text-orange-400 transition-colors w-fit group"
        >
          <ArrowLeft size={18} className="group-hover:-translate-x-1 transition-transform" />
          <span className="text-sm font-medium">Voltar para Servidores</span>
        </button>

        <div className="mt-8 rounded-3xl border border-white/5 bg-[#2b3238] p-6 text-gray-400">
          Servidor nao encontrado.
        </div>
      </div>
    )
  }

  const safeMods = Array.isArray(allMods) ? allMods : []
  const safeActiveIds = Array.isArray(server.activeModIds) ? server.activeModIds : []
  const activatedModIds = new Set(safeActiveIds.map((modId) => normalizeModId(modId)))
  const libraryMods = safeMods.filter((mod) => mod?.id)
  const modsById = new Map(libraryMods.map((mod) => [normalizeModId(mod.id), mod]))
  const activatedMods = safeActiveIds
    .map((modId) => modsById.get(normalizeModId(modId)))
    .filter((mod): mod is ZomboidMod => Boolean(mod))
  const availableMods = libraryMods.filter((mod) => !activatedModIds.has(String(mod.id).toLowerCase()))
  const isCurrentServerTesting = isTestingServer || runningServerTestId === server.id

  const filteredActivated = activatedMods.filter((mod) => matchesSearch(mod, search))
  const filteredAvailable = availableMods.filter((mod) => matchesSearch(mod, search))

  const handleActiveModContextMenu = (event: React.MouseEvent, mod: ZomboidMod) => {
    event.preventDefault()
    setContextMenu({ mod, x: event.clientX, y: event.clientY })
  }

  const getActiveDependents = (mod: ZomboidMod) =>
    activatedMods.filter((activeMod) =>
      activeMod.dependencies?.some((dependency) => normalizeModId(dependency) === normalizeModId(mod.id)),
    )

  const moveActiveMod = async (position: "start" | "end") => {
    if (!contextMenu) {
      return
    }

    const mod = contextMenu.mod

    if (position === "end" && getActiveDependents(mod).length > 0) {
      return
    }

    if (window.localStorage.getItem(MOVE_MOD_WARNING_KEY) === "true") {
      setContextMenu(null)
      await onMoveActiveMod(mod, position)
    } else {
      setContextMenu(null)
      setDontShowAgainMove(false)
      setShowMoveWarning({ mod, position })
    }
  }

  const confirmMoveMod = async () => {
    if (!showMoveWarning) return

    if (dontShowAgainMove) {
      window.localStorage.setItem(MOVE_MOD_WARNING_KEY, "true")
    }

    await onMoveActiveMod(showMoveWarning.mod, showMoveWarning.position)
    setShowMoveWarning(null)
    setDontShowAgainMove(false)
  }

  const handleDeactivateClick = (mod: ZomboidMod) => {
    setContextMenu(null)
    const dependents = getActiveDependents(mod)

    if (dependents.length > 0) {
      setDependencyWarning({ mod, dependents })
    } else {
      setConfirmDelete(mod)
    }
  }

  const handleActivateClick = async (mod: ZomboidMod) => {
    const dependencyPlan = buildActivationDependencyPlan(mod, safeMods, activatedModIds)
    const modNeedsInstall = !isLocalMod(mod)

    if (dependencyPlan.missingDependencyId) {
      setMissingDependency({ mod, dependencyId: dependencyPlan.missingDependencyId })
      return
    }

    if (modNeedsInstall || dependencyPlan.dependenciesToInstall.length > 0 || dependencyPlan.dependenciesToActivate.length > 0) {
      setPendingActivation({
        mod,
        dependenciesToInstall: dependencyPlan.dependenciesToInstall,
        dependenciesToActivate: dependencyPlan.dependenciesToActivate,
        modNeedsInstall,
      })
      return
    }

    await onToggleMod(mod, "activate")
  }

  const confirmActivationWithDependencies = async () => {
    if (!pendingActivation) {
      return
    }

    const modsToInstall = pendingActivation.modNeedsInstall
      ? [...pendingActivation.dependenciesToInstall, pendingActivation.mod]
      : pendingActivation.dependenciesToInstall

    if (modsToInstall.length > 0) {
      await onInstallMods(modsToInstall)
    }

    await onActivateMods([...pendingActivation.dependenciesToActivate, pendingActivation.mod])
    setPendingActivation(null)
  }

  const testServer = async (skipPortCheck = false) => {
    if (isCurrentServerTesting) {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id } }))
      return
    }

    if (!skipPortCheck) {
      setIsCheckingPorts(true)

      try {
        const check = await invokeTauri<ServerPortCheck>("check_zomboid_server_ports", {
          serverId: server.id,
        })

        if (check.usages.length > 0) {
          setPortConflictCheck(check)
          return
        }
      } catch (error) {
        window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id, error: getErrorMessage(error) } }))
        return
      } finally {
        setIsCheckingPorts(false)
      }
    }

    setIsTestingServer(true)
    window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id } }))

    try {
      await invokeTauri("start_zomboid_server_test", {
        serverId: server.id,
      })
    } catch (error) {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id, error: getErrorMessage(error) } }))
    } finally {
      setIsTestingServer(false)
    }
  }

  const killPortConflictsAndTest = async () => {
    if (!portConflictCheck) {
      return
    }

    setIsKillingPorts(true)

    try {
      await invokeTauri("kill_processes_by_pid", {
        pids: Array.from(new Set(portConflictCheck.usages.map((usage) => usage.pid))),
      })
      setPortConflictCheck(null)
      await testServer(true)
    } catch (error) {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id, error: getErrorMessage(error) } }))
    } finally {
      setIsKillingPorts(false)
    }
  }

  return (
    <div className="h-full min-h-0 overflow-y-auto bg-[#22272b] p-8 text-white custom-scrollbar">
      <div className="flex min-h-full flex-col gap-6 relative">
      {/* Header */}
      <div className="flex flex-col gap-6">
        <button
          onClick={onBack}
          className="flex items-center gap-2 text-gray-400 hover:text-orange-400 transition-colors w-fit group"
        >
          <ArrowLeft size={18} className="group-hover:-translate-x-1 transition-transform" />
          <span className="text-sm font-medium">Voltar para Servidores</span>
        </button>

        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-6 bg-[#2b3238] p-6 rounded-3xl border border-white/5 relative overflow-hidden">
          <div className="absolute -right-20 -top-20 w-64 h-64 bg-orange-400/5 rounded-full blur-3xl" />

          <div className="flex items-center gap-5 relative z-10">
            <div className="p-4 bg-[#22272b] rounded-2xl text-orange-400 border border-white/5 shadow-xl">
              <Server size={32} />
            </div>
            <div>
              <h2 className="text-3xl font-black text-white tracking-tight">{server.name}</h2>
              <div className="flex items-center gap-3 mt-1 text-sm text-gray-400 font-mono">
                <span className="flex items-center gap-1.5">
                  <div className="w-2 h-2 rounded-full bg-red-500" />
                  OFFLINE
                </span>
                <span className="text-white/10">|</span>
                <span>{server.fileName}</span>
                <span className="text-white/10">|</span>
                <span>Porta: {server.port}</span>
              </div>
            </div>
          </div>

          <div className="flex flex-wrap gap-3 relative z-10">
             <button
                onClick={() => void testServer()}
                disabled={isCurrentServerTesting || isCheckingPorts}
                className="flex items-center gap-2 rounded-xl border border-orange-500/20 bg-orange-500/10 px-4 py-2 text-sm font-black text-orange-400 transition-all hover:bg-orange-500 hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
             >
                {isCurrentServerTesting || isCheckingPorts ? <RefreshCw size={18} className="animate-spin" /> : <Play size={18} />}
                <span>{isCheckingPorts ? "Verificando portas" : isCurrentServerTesting ? "Testando" : "Testar servidor"}</span>
             </button>
             <div className="bg-[#22272b] px-4 py-2 rounded-xl border border-white/5 text-center">
                <p className="text-[10px] text-gray-500 uppercase font-bold tracking-widest">Mods Ativos</p>
                <p className="text-xl font-black text-orange-400">{activatedMods.length}</p>
             </div>
             <div className="bg-[#22272b] px-4 py-2 rounded-xl border border-white/5 text-center">
                <p className="text-[10px] text-gray-500 uppercase font-bold tracking-widest">Jogadores Max</p>
                <p className="text-xl font-black text-white">{server.maxPlayers || "-"}</p>
             </div>
          </div>
        </div>
      </div>

      {/* Search & Filter bar */}
      <div className="relative group max-w-md">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within:text-orange-400 transition-colors" size={18} />
        <input
          type="text"
          placeholder="Filtrar mods do servidor..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full bg-[#2b3238] border border-white/5 rounded-xl py-2.5 pl-10 pr-4 text-sm focus:outline-none focus:border-orange-400/50 transition-all placeholder:text-gray-600"
        />
      </div>

      {/* Lists */}
      <div className="flex flex-col gap-6 pb-10">

        <ServerModList
          title="Mods Ativados"
          mods={filteredActivated}
          emptyMessage="Nenhum mod ativado encontrado."
          isExpanded={isActivatedExpanded}
          action="deactivate"
          onToggleExpanded={() => setIsActivatedExpanded(!isActivatedExpanded)}
          onAction={handleDeactivateClick}
          onContextMenu={handleActiveModContextMenu}
        />

        <ServerModList
          title="Mods Disponíveis"
          mods={filteredAvailable}
          emptyMessage="Nenhum mod disponivel encontrado."
          isExpanded={isAvailableExpanded}
          action="activate"
          onToggleExpanded={() => setIsAvailableExpanded(!isAvailableExpanded)}
          onAction={handleActivateClick}
        />
      </div>

      {contextMenu && (
        <div className="fixed inset-0 z-50" onClick={() => setContextMenu(null)} onContextMenu={(event) => event.preventDefault()}>
          <div
            className="absolute w-56 overflow-hidden rounded-xl border border-white/10 bg-[#1e2327] py-2 shadow-2xl shadow-black/40"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            onClick={(event) => event.stopPropagation()}
          >
            {(() => {
              const dependents = getActiveDependents(contextMenu.mod)
              const cannotMoveToEnd = dependents.length > 0

              return (
                <>
            <div className="border-b border-white/5 px-4 pb-2 pt-1">
              <p className="truncate text-xs font-bold text-white">{contextMenu.mod.name}</p>
              <p className="truncate text-[10px] font-mono text-gray-500">{contextMenu.mod.id}</p>
            </div>
            <button
              onClick={() => void moveActiveMod("start")}
              className="w-full px-4 py-2.5 text-left text-sm font-medium text-gray-300 transition-colors hover:bg-orange-500/10 hover:text-orange-300"
            >
              Colocar no inicio
            </button>
            <button
              onClick={() => void moveActiveMod("end")}
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
                </>
              )
            })()}
          </div>
        </div>
      )}

      {portConflictCheck && (
        <ServerPortConflictModal
          check={portConflictCheck}
          isKilling={isKillingPorts}
          onCancel={() => setPortConflictCheck(null)}
          onConfirm={() => void killPortConflictsAndTest()}
        />
      )}

      {/* Confirmation Modal */}
      {confirmDelete && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
          <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-sm overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300 p-6 text-center">
            <div className="w-16 h-16 bg-red-500/10 text-red-500 rounded-full flex items-center justify-center mx-auto mb-4">
              <Trash2 size={32} />
            </div>
            <h3 className="text-xl font-bold text-white mb-2">Desativar Mod?</h3>
            <p className="text-gray-400 text-sm mb-6">
              Tem certeza que deseja desativar o mod <span className="text-white font-bold">{confirmDelete.name}</span> deste servidor?
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => {
                  void onToggleMod(confirmDelete, "deactivate")
                  setConfirmDelete(null)
                }}
                className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl transition-all"
              >
                Sim, Desativar
              </button>
              <button
                onClick={() => setConfirmDelete(null)}
                className="flex-1 py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all"
              >
                Cancelar
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Dependency Alert Modal (Active Dependents) */}
      {dependencyWarning && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
          <div className="bg-[#22272b] border border-orange-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
            <div className="p-6 bg-orange-500/10 border-b border-orange-500/10 flex items-center gap-3">
              <AlertTriangle className="text-orange-500" size={28} />
              <h3 className="text-xl font-bold text-white">Alerta de Dependência</h3>
            </div>
            <div className="p-6">
              <p className="text-gray-300 text-sm mb-4 leading-relaxed">
                O mod <span className="text-orange-400 font-bold">{dependencyWarning.mod.name}</span> não pode ser desativado sozinho pois é uma dependência direta de:
              </p>

              <div className="space-y-2 mb-6">
                {dependencyWarning.dependents.map(dep => (
                  <div key={dep.id} className="flex items-center gap-2 p-3 bg-[#1e2327] rounded-xl border border-white/5">
                    <div className="w-2 h-2 rounded-full bg-orange-500" />
                    <span className="text-sm font-medium text-white">{dep.name}</span>
                  </div>
                ))}
              </div>

              <div className="p-4 bg-orange-500/5 rounded-2xl border border-orange-500/10 flex gap-3 mb-6">
                <Info size={20} className="text-orange-400 shrink-0 mt-0.5" />
                <p className="text-[11px] text-gray-400 italic">
                  Para remover este mod, você deve primeiro desativar os mods listados acima.
                </p>
              </div>

              <button
                onClick={() => setDependencyWarning(null)}
                className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20"
              >
                Entendido
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Dependency Activation Modal */}
      {pendingActivation && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
          <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
            <div className="p-6 border-b border-white/5 flex justify-between items-center">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-orange-500/20 text-orange-400 rounded-xl">
                  <AlertCircle size={24} />
                </div>
                <h3 className="text-xl font-bold text-white">Dependencias pendentes</h3>
              </div>
              <button
                onClick={() => setPendingActivation(null)}
                className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors"
              >
                <X size={20} />
              </button>
            </div>

            <div className="p-6">
              <p className="text-gray-400 text-sm mb-4">
                O mod <span className="text-white font-bold">{pendingActivation.mod.name}</span> precisa ser preparado
                antes de ser ativado:
              </p>

              <div className="space-y-3 mb-6 max-h-56 overflow-y-auto custom-scrollbar pr-2">
                {pendingActivation.modNeedsInstall && (
                  <div className="flex items-center gap-3 p-3 bg-[#2b3238] border border-white/5 rounded-xl">
                    <div className="w-10 h-10 rounded-lg bg-[#1e2327] overflow-hidden shrink-0">
                      {pendingActivation.mod.imageUrl ? (
                        <img src={pendingActivation.mod.imageUrl} alt={pendingActivation.mod.name} className="w-full h-full object-cover" />
                      ) : (
                        <div className="w-full h-full flex items-center justify-center text-white/10">
                          <PlusCircle size={16} />
                        </div>
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-bold text-white truncate">{pendingActivation.mod.name}</p>
                      <p className="text-[10px] text-gray-500 font-mono truncate">{pendingActivation.mod.id}</p>
                    </div>
                    <span className="text-[10px] font-bold text-orange-300 bg-orange-500/10 border border-orange-500/10 rounded-full px-2 py-0.5 shrink-0">
                      Trazer
                    </span>
                  </div>
                )}
                {pendingActivation.dependenciesToActivate.map((dep) => {
                  const willInstall = pendingActivation.dependenciesToInstall.some(
                    (installDep) => normalizeModId(installDep.id) === normalizeModId(dep.id),
                  )

                  return (
                    <div key={dep.id} className="flex items-center gap-3 p-3 bg-[#2b3238] border border-white/5 rounded-xl">
                      <div className="w-10 h-10 rounded-lg bg-[#1e2327] overflow-hidden shrink-0">
                        {dep.imageUrl ? (
                          <img src={dep.imageUrl} alt={dep.name} className="w-full h-full object-cover" />
                        ) : (
                          <div className="w-full h-full flex items-center justify-center text-white/10">
                            <PlusCircle size={16} />
                          </div>
                        )}
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-bold text-white truncate">{dep.name}</p>
                        <p className="text-[10px] text-gray-500 font-mono truncate">{dep.id}</p>
                      </div>
                      <span className="text-[10px] font-bold text-orange-300 bg-orange-500/10 border border-orange-500/10 rounded-full px-2 py-0.5 shrink-0">
                        {willInstall ? "Trazer" : "Ativar"}
                      </span>
                    </div>
                  )
                })}
              </div>

              <div className="flex flex-col gap-3">
                <button
                  onClick={confirmActivationWithDependencies}
                  className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 flex items-center justify-center gap-2"
                >
                  <CheckCircle2 size={18} />
                  Trazer para local e ativar
                </button>
                <button
                  onClick={() => setPendingActivation(null)}
                  className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all"
                >
                  Cancelar
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Move Mod Warning Modal */}
      {showMoveWarning && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
          <div className="bg-[#22272b] border border-orange-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
            <div className="p-6 bg-orange-500/10 border-b border-orange-500/10 flex items-center gap-3">
              <AlertTriangle className="text-orange-500" size={28} />
              <h3 className="text-xl font-bold text-white">Aviso de Segurança</h3>
            </div>
            <div className="p-6">
              <p className="text-gray-300 text-sm mb-6 leading-relaxed">
                Alterar a ordem de carregamento pode quebrar o funcionamento de alguns mods.
                Mova <span className="text-orange-400 font-bold">{showMoveWarning.mod.name}</span> apenas se tiver certeza de que ele deve carregar
                {showMoveWarning.position === "start" ? " no início " : " no final "} da lista.
              </p>

              <button
                onClick={() => void confirmMoveMod()}
                className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 mb-4 flex items-center justify-center gap-2"
              >
                <Check size={18} />
                Confirmar Movimentação
              </button>

              <button
                onClick={() => setDontShowAgainMove(!dontShowAgainMove)}
                className="mb-4 flex items-center gap-2 text-left group"
              >
                <span
                  className={`flex h-5 w-5 items-center justify-center rounded border transition-all ${
                    dontShowAgainMove
                      ? "border-orange-500 bg-orange-500"
                      : "border-white/20 bg-transparent group-hover:border-white/40"
                  }`}
                >
                  {dontShowAgainMove && <Check size={12} className="text-white" />}
                </span>
                <span className="text-xs text-gray-400 transition-colors group-hover:text-gray-300">
                  NÃ£o mostrar este alerta novamente
                </span>
              </button>

              <button
                onClick={() => {
                  setShowMoveWarning(null)
                  setDontShowAgainMove(false)
                }}
                className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all"
              >
                Cancelar
              </button>
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
          onDownloaded={onDependencyDownloaded ?? onRefreshMods}
          onOpenSettings={onOpenSettings}
        />
      )}
      </div>
    </div>
  )
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  if (error) {
    return JSON.stringify(error)
  }

  return "Nao foi possivel testar o servidor."
}
