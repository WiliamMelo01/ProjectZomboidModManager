import { ArrowLeft, Play, RefreshCw, Search, Server } from "lucide-react"
import { useState } from "react"

import { MissingDependencyModal } from "@/components/MissingDependencyModal"
import {
  DeactivateModModal,
  DependencyWarningModal,
  MoveModWarningModal,
  PendingActivationModal,
  type MoveModRequest,
  type PendingActivation,
} from "@/components/server/ServerDetailModals"
import { ServerModContextMenu } from "@/components/server/ServerModContextMenu"
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
  const [showMoveWarning, setShowMoveWarning] = useState<MoveModRequest | null>(null)
  const [dontShowAgainMove, setDontShowAgainMove] = useState(false)
  const [isTestingServer, setIsTestingServer] = useState(false)
  const [portConflictCheck, setPortConflictCheck] = useState<ServerPortCheck | null>(null)
  const [isCheckingPorts, setIsCheckingPorts] = useState(false)
  const [isKillingPorts, setIsKillingPorts] = useState(false)
  const [mapInstallError, setMapInstallError] = useState<string | null>(null)

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

  const handleInstallMapClick = async (mod: ZomboidMod) => {
    const dependencyPlan = buildActivationDependencyPlan(mod, safeMods, activatedModIds)

    if (dependencyPlan.missingDependencyId) {
      setMissingDependency({ mod, dependencyId: dependencyPlan.missingDependencyId })
      return
    }

    setMapInstallError(null)

    try {
      const modsToInstall = !isLocalMod(mod)
        ? [...dependencyPlan.dependenciesToInstall, mod]
        : dependencyPlan.dependenciesToInstall

      if (modsToInstall.length > 0) {
        await onInstallMods(modsToInstall)
      }

      await invokeTauri("install_zomboid_server_map", {
        serverId: server.id,
        modPath: mod.path,
      })
      await onActivateMods([...dependencyPlan.dependenciesToActivate, mod])
    } catch (error) {
      setMapInstallError(getErrorMessage(error))
    }
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
        {mapInstallError && (
          <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
            {mapInstallError}
          </div>
        )}

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
          onInstallMap={(mod) => void handleInstallMapClick(mod)}
        />
      </div>

      {contextMenu && (
        <ServerModContextMenu
          mod={contextMenu.mod}
          x={contextMenu.x}
          y={contextMenu.y}
          dependents={getActiveDependents(contextMenu.mod)}
          onClose={() => setContextMenu(null)}
          onMove={(position) => void moveActiveMod(position)}
        />
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
        <DeactivateModModal
          mod={confirmDelete}
          onCancel={() => setConfirmDelete(null)}
          onConfirm={() => {
            void onToggleMod(confirmDelete, "deactivate")
            setConfirmDelete(null)
          }}
        />
      )}

      {/* Dependency Alert Modal (Active Dependents) */}
      {dependencyWarning && (
        <DependencyWarningModal
          mod={dependencyWarning.mod}
          dependents={dependencyWarning.dependents}
          onClose={() => setDependencyWarning(null)}
        />
      )}

      {/* Dependency Activation Modal */}
      {pendingActivation && (
        <PendingActivationModal
          activation={pendingActivation}
          onCancel={() => setPendingActivation(null)}
          onConfirm={() => void confirmActivationWithDependencies()}
        />
      )}

      {/* Move Mod Warning Modal */}
      {showMoveWarning && (
        <MoveModWarningModal
          request={showMoveWarning}
          dontShowAgain={dontShowAgainMove}
          onToggleDontShowAgain={() => setDontShowAgainMove(!dontShowAgainMove)}
          onCancel={() => {
            setShowMoveWarning(null)
            setDontShowAgainMove(false)
          }}
          onConfirm={() => void confirmMoveMod()}
        />
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
