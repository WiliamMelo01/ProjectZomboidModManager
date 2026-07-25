import { ArrowLeft, FilePenLine, FileText, Hash, PackageCheck, Play, RefreshCw, Search, Server, Settings, Square, Terminal, Wand2 } from "lucide-react"
import { useDeferredValue, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { MissingDependencyModal } from "@/components/MissingDependencyModal"
import {
  DeactivateModModal,
  ChangeServerBuildModal,
  DependencyWarningModal,
  IncompatibleModsModal,
  MapInstallConfirmationModal,
  MoveModWarningModal,
  PendingActivationModal,
  type MoveModRequest,
  type PendingActivation,
} from "@/components/server/ServerDetailModals"
import { FixWorkshopIdsModal } from "@/components/server/FixWorkshopIdsModal"
import { ServerLogsModal } from "@/components/server/ServerLogsModal"
import { ServerModContextMenu } from "@/components/server/ServerModContextMenu"
import { ServerModDetailsModal } from "@/components/server/ServerModDetailsModal"
import { ServerModList } from "@/components/server/ServerModList"
import { buildActivationDependencyPlan, isLocalMod, normalizeModId } from "@/lib/modDependencies"
import { resolveModForBuild } from "@/lib/modBuilds"
import { invokeTauri } from "@/lib/tauri"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import { i18n } from "@/i18n"
import type { ZomboidMod } from "@/types/mod"
import type { ZomboidServer } from "@/types/server"

type ServerFilePreview = {
  serverId?: string
  server_id?: string
  fileName?: string
  file_name?: string
  path?: string
  content?: string
}

type ServerDetailProps = {
  server: ZomboidServer | null
  allMods?: ZomboidMod[]
  onBack: () => void
  onInstallMods: (mods: ZomboidMod[]) => Promise<ZomboidMod[] | void>
  onActivateMods: (mods: ZomboidMod[]) => Promise<void>
  onToggleMod: (mod: ZomboidMod, action: "activate" | "deactivate") => Promise<void>
  onMoveActiveMod: (mod: ZomboidMod, position: "start" | "end") => Promise<void>
  onRefreshMods?: () => Promise<void>
  onDependencyDownloaded?: (dependencyId: string, originalModId?: string) => Promise<void>
  onOpenSettings?: () => void
  runningServerTestId?: string | null
  onChangeBuild: (gameBuild: "b41" | "b42") => Promise<void>
  onConfigureServer: (server: ZomboidServer) => void
  remoteConnection?: RemoteConnectionDraft | null
  isTestingServer: boolean
  isCheckingPorts: boolean
  isCheckingRemoteFirewall: boolean
  isConfiguringRemoteFirewall: boolean
  isStartingRemoteServer: boolean
  onTestServer: (server: ZomboidServer) => void
  onStartRemoteServer: (server: ZomboidServer) => void
  onOpenRemoteConsole?: (server: ZomboidServer) => void
  onStopRemoteServer?: (server: ZomboidServer) => void
  workshopMappings?: Record<string, string>
  onSaveWorkshopMapping?: (modId: string, workshopId: string) => Promise<void>
  onUpdateServerMods?: (server: ZomboidServer, activeModIds: string[]) => Promise<void>
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

function IniContentRenderer({ content = "" }: { content?: string }) {
  const lines = useMemo(() => (content ?? "").split("\n"), [content])

  if (!content || content.trim().length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center p-12 text-gray-400 italic text-xs font-mono bg-[#181c20]">
        O arquivo do servidor está vazio ou não possui conteúdo.
      </div>
    )
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto p-6 font-mono text-xs leading-relaxed custom-scrollbar bg-[#181c20]">
      {lines.map((line, index) => {
        const trimmed = (line ?? "").trim()

        // 1. Linha de Título / Comentário (# ou [)
        if (trimmed.startsWith("#") || trimmed.startsWith("[")) {
          return (
            <div key={index} className="flex items-start py-0.5 hover:bg-white/[0.02] rounded px-1 transition-colors">
              <span className="w-10 text-[10px] text-gray-600 select-none text-right pr-3 shrink-0 pt-0.5">
                {index + 1}
              </span>
              <span className="font-semibold text-orange-400/90 break-all">
                {line}
              </span>
            </div>
          )
        }

        // 2. Linha Vazia
        if (!trimmed) {
          return <div key={index} className="h-1.5" />
        }

        // 3. Linha de Configuração (Chave = Valor)
        const equalsIndex = line.indexOf("=")
        if (equalsIndex !== -1) {
          const key = line.slice(0, equalsIndex).trim()
          const value = line.slice(equalsIndex + 1)
          const elementId = key === "Mods" ? "ini-line-mods" : key === "WorkshopItems" ? "ini-line-workshop" : undefined

          return (
            <div
              key={index}
              id={elementId}
              className="flex items-start py-0.5 hover:bg-white/[0.02] rounded px-1 transition-colors"
            >
              <span className="w-10 text-[10px] text-gray-600 select-none text-right pr-3 shrink-0 pt-0.5">
                {index + 1}
              </span>
              <div className="flex-1 min-w-0 break-all">
                <span className={`font-semibold ${elementId ? "text-orange-300 font-bold" : "text-sky-300"}`}>
                  {key}
                </span>
                <span className="text-gray-500 font-bold mx-1">=</span>
                <span className="text-gray-200">{value}</span>
              </div>
            </div>
          )
        }

        // 4. Outras linhas
        return (
          <div key={index} className="flex items-start py-0.5 hover:bg-white/[0.02] rounded px-1 transition-colors">
            <span className="w-10 text-[10px] text-gray-600 select-none text-right pr-3 shrink-0 pt-0.5">
              {index + 1}
            </span>
            <span className="text-gray-400 flex-1 min-w-0 break-all">{line}</span>
          </div>
        )
      })}
    </div>
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
  onChangeBuild,
  onConfigureServer,
  remoteConnection = null,
  isTestingServer,
  isCheckingPorts,
  isCheckingRemoteFirewall,
  isConfiguringRemoteFirewall,
  isStartingRemoteServer,
  onTestServer,
  onStartRemoteServer,
  onOpenRemoteConsole,
  onStopRemoteServer,
  workshopMappings = {},
  onSaveWorkshopMapping,
  onUpdateServerMods,
}: ServerDetailProps) {
  const { t } = useTranslation()
  const [search, setSearch] = useState("")
  const [confirmDelete, setConfirmDelete] = useState<ZomboidMod | null>(null)
  const [dependencyWarning, setDependencyWarning] = useState<{ mod: ZomboidMod; dependents: ZomboidMod[] } | null>(null)
  const [missingDependency, setMissingDependency] = useState<{ mod: ZomboidMod; dependencyId: string } | null>(null)
  const [showFixWorkshopIdsModal, setShowFixWorkshopIdsModal] = useState(false)
  const [showLogsModal, setShowLogsModal] = useState(false)
  const [pendingActivation, setPendingActivation] = useState<PendingActivation | null>(null)
  const [contextMenu, setContextMenu] = useState<{ mod: ZomboidMod; x: number; y: number } | null>(null)
  const [showMoveWarning, setShowMoveWarning] = useState<MoveModRequest | null>(null)
  const [dontShowAgainMove, setDontShowAgainMove] = useState(false)
  const [mapInstallError, setMapInstallError] = useState<string | null>(null)
  const [activationError, setActivationError] = useState<string | null>(null)
  const [serverFileOpenError, setServerFileOpenError] = useState<string | null>(null)
  const [serverFilePreview, setServerFilePreview] = useState<ServerFilePreview | null>(null)
  const [isOpeningServerFile, setIsOpeningServerFile] = useState(false)
  const [pendingMapInstall, setPendingMapInstall] = useState<ZomboidMod | null>(null)
  const [isChangingBuild, setIsChangingBuild] = useState(false)
  const [pendingBuild, setPendingBuild] = useState<"b41" | "b42" | null>(null)
  const [showIncompatibleMods, setShowIncompatibleMods] = useState(false)
  const [selectedMod, setSelectedMod] = useState<ZomboidMod | null>(null)

  const [isActivatedExpanded, setIsActivatedExpanded] = useState(true)
  const [isAvailableExpanded, setIsAvailableExpanded] = useState(true)
  const deferredSearch = useDeferredValue(search)
  const safeMods = useMemo(() => Array.isArray(allMods) ? allMods : [], [allMods])
  const safeActiveIds = useMemo(() => Array.isArray(server?.activeModIds) ? server.activeModIds : [], [server?.activeModIds])
  const activatedModIds = useMemo(() => new Set(safeActiveIds.map((modId) => normalizeModId(modId))), [safeActiveIds])
  const libraryMods = useMemo(() => dedupeModsForServer(safeMods), [safeMods])
  const compatibleMods = useMemo(
    () => server
      ? libraryMods
        .map((mod) => resolveModForBuild(mod, server.gameBuild))
        .filter((mod): mod is ZomboidMod => Boolean(mod))
      : [],
    [libraryMods, server],
  )
  const compatibleModIds = useMemo(
    () => new Set(compatibleMods.map((mod) => normalizeModId(mod.id))),
    [compatibleMods],
  )
  const modsById = useMemo(
    () => new Map(
      libraryMods.flatMap((mod) => mod.variants.map((variant) => [
        normalizeModId(variant.id),
        { ...mod, id: variant.id, path: variant.path, dependencies: variant.dependencies, mapNames: variant.mapNames },
      ] as const)),
    ),
    [libraryMods],
  )
  const activatedMods = useMemo(
    () => safeActiveIds.map((modId) => modsById.get(normalizeModId(modId)) ?? createMissingActiveMod(modId)),
    [modsById, safeActiveIds],
  )
  const availableMods = useMemo(
    () => compatibleMods.filter((mod) => !activatedModIds.has(normalizeModId(mod.id))),
    [activatedModIds, compatibleMods],
  )
  const incompatibleActiveIds = useMemo(
    () => safeActiveIds.filter((modId) => !compatibleModIds.has(normalizeModId(modId))),
    [compatibleModIds, safeActiveIds],
  )
  const incompatibleActiveIdSet = useMemo(() => new Set(incompatibleActiveIds.map(normalizeModId)), [incompatibleActiveIds])
  const incompatibleActiveMods = useMemo(
    () => incompatibleActiveIds.map((modId) => ({
      id: modId,
      name: modsById.get(normalizeModId(modId))?.name ?? modId,
      compatibleBuilds: modsById.get(normalizeModId(modId))?.compatibleBuilds ?? [],
      isInLibrary: modsById.has(normalizeModId(modId)),
    })),
    [incompatibleActiveIds, modsById],
  )
  const filteredActivated = useMemo(
    () => activatedMods.filter((mod) => matchesSearch(mod, deferredSearch)),
    [activatedMods, deferredSearch],
  )
  const filteredAvailable = useMemo(
    () => availableMods.filter((mod) => matchesSearch(mod, deferredSearch)),
    [availableMods, deferredSearch],
  )

  if (!server) {
    return (
      <div className="h-full min-h-0 overflow-y-auto bg-[#22272b] p-8 text-white custom-scrollbar">
        <button
          onClick={onBack}
          className="flex items-center gap-2 text-gray-400 hover:text-orange-400 transition-colors w-fit group"
        >
          <ArrowLeft size={18} className="group-hover:-translate-x-1 transition-transform" />
          <span className="text-sm font-medium">{t("serverDetail.back")}</span>
        </button>

        <div className="mt-8 rounded-3xl border border-white/5 bg-[#2b3238] p-6 text-gray-400">
          {t("serverDetail.notFound")}
        </div>
      </div>
    )
  }

  const isCurrentServerTesting = isTestingServer || runningServerTestId === server.id
  const isServerOnline = server.status === "online"

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

    const uniqueMods = (items: ZomboidMod[]) => {
      const seen = new Set<string>()
      return items.filter((item) => {
        const id = normalizeModId(item.id)
        if (!id || seen.has(id)) return false
        seen.add(id)
        return true
      })
    }

    const activation = pendingActivation

    setPendingActivation(null)
    setActivationError(null)

    const modsToInstall = uniqueMods(
      activation.modNeedsInstall
        ? [...activation.dependenciesToInstall, activation.mod]
        : activation.dependenciesToInstall,
    )
    const modsToActivate = uniqueMods([
      ...activation.dependenciesToActivate,
      activation.mod,
    ])

    void (async () => {
      try {
        if (modsToInstall.length > 0) {
          await onInstallMods(modsToInstall)
        }

        await onActivateMods(modsToActivate)
      } catch (error) {
        setActivationError(getErrorMessage(error))
        if (onRefreshMods) {
          await onRefreshMods()
        }
      }
    })()
  }

  const installMap = async (mod: ZomboidMod) => {
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

      await invokeTauri(remoteConnection ? "install_remote_zomboid_server_map" : "install_zomboid_server_map", {
        ...(remoteConnection ? { connection: remoteConnection } : {}),
        serverId: server.id,
        modPath: mod.path,
      })
      await onActivateMods([...dependencyPlan.dependenciesToActivate, mod])
      setPendingMapInstall(null)
    } catch (error) {
      setMapInstallError(getErrorMessage(error))
    }
  }

  const openServerFile = async () => {
    if (!server) return
    setServerFileOpenError(null)
    setIsOpeningServerFile(true)

    try {
      if (remoteConnection) {
        const file = await invokeTauri<ServerFilePreview>("read_remote_zomboid_server_file", {
          connection: remoteConnection,
          serverId: server.id,
        })
        setServerFilePreview(file)
      } else {
        const file = await invokeTauri<ServerFilePreview>("read_zomboid_server_file", {
          serverId: server.id,
        })
        setServerFilePreview(file)
      }
    } catch (error) {
      setServerFileOpenError(getErrorMessage(error))
    } finally {
      setIsOpeningServerFile(false)
    }
  }

  const changeBuild = async () => {
    if (!pendingBuild || pendingBuild === server.gameBuild) return
    setIsChangingBuild(true)
    try {
      await onChangeBuild(pendingBuild)
      setPendingBuild(null)
    } finally {
      setIsChangingBuild(false)
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
          <span className="text-sm font-medium">{t("serverDetail.back")}</span>
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
                <span className={`flex items-center gap-1.5 ${server.status === "online" ? "text-green-300" : "text-gray-400"}`}>
                  <div className={`w-2 h-2 rounded-full ${server.status === "online" ? "bg-green-400 animate-pulse" : "bg-red-500"}`} />
                  {server.status.toUpperCase()}
                </span>
                <span className="text-white/10">|</span>
                <button
                  type="button"
                  onClick={() => void openServerFile()}
                  disabled={isOpeningServerFile}
                  title={t("serverDetail.openFile")}
                  className="flex items-center gap-1.5 transition-colors hover:text-orange-300 hover:underline disabled:cursor-wait disabled:opacity-60"
                >
                  {isOpeningServerFile ? <RefreshCw size={14} className="animate-spin" /> : <FilePenLine size={14} />}
                  <span>{server.fileName}</span>
                </button>
                <span className="text-white/10">|</span>
                <span>{t("serverDetail.port")}: {server.port}</span>
              </div>
              <div className="mt-3 flex items-center gap-2">
                {(["b41", "b42"] as const).map((build) => (
                  <button
                    key={build}
                    type="button"
                    disabled={isChangingBuild}
                    onClick={() => setPendingBuild(build)}
                    className={`rounded-full border px-2.5 py-1 text-[10px] font-black uppercase ${
                      server.gameBuild === build ? "border-orange-400/30 bg-orange-400/10 text-orange-300" : "border-white/10 text-gray-500 hover:text-white"
                    }`}
                  >
                    {build}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-4 relative z-10">
            {/* Group 1: Configuration & Testing */}
            <div className="flex items-center rounded-xl bg-[#22272b]/80 p-1 border border-white/5 shadow-inner">
              <button
                type="button"
                onClick={() => onConfigureServer(server)}
                className="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold text-gray-400 hover:text-white transition-all"
              >
                <Settings size={14} />
                <span>{t("serverDetail.configure")}</span>
              </button>
              <div className="h-4 w-[1px] bg-white/10" />
              <button
                onClick={() => void onTestServer(server)}
                disabled={isCurrentServerTesting || isCheckingPorts}
                className="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold text-gray-400 hover:text-white disabled:cursor-not-allowed disabled:opacity-50 transition-all"
              >
                {isCurrentServerTesting || isCheckingPorts ? <RefreshCw size={14} className="animate-spin" /> : <Play size={14} />}
                <span>
                  {isCheckingPorts
                    ? t("serverDetail.checkingPorts")
                    : isCurrentServerTesting
                    ? t("serverDetail.testing")
                    : t("serverDetail.test")}
                </span>
              </button>
            </div>

            {/* Group 2: Remote VM Operation Controls */}
            {remoteConnection && (
              <div className="flex items-center rounded-xl bg-[#22272b]/80 p-1 border border-white/5 shadow-inner">
                <button
                  type="button"
                  onClick={() => void onOpenRemoteConsole?.(server)}
                  disabled={!isServerOnline}
                  className="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold text-gray-400 hover:text-cyan-400 disabled:cursor-not-allowed disabled:opacity-50 transition-all"
                >
                  <Terminal size={14} />
                  <span>{t("serverDetail.console")}</span>
                </button>
                <div className="h-4 w-[1px] bg-white/10" />
                {isServerOnline ? (
                  <button
                    type="button"
                    onClick={() => void onStopRemoteServer?.(server)}
                    className="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold text-red-400 hover:text-red-300 transition-all"
                  >
                    <Square size={12} />
                    <span>{t("serverDetail.stop")}</span>
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => void onStartRemoteServer(server)}
                    disabled={
                      isCheckingRemoteFirewall ||
                      isConfiguringRemoteFirewall ||
                      isStartingRemoteServer
                    }
                    className="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold text-emerald-400 hover:text-emerald-300 disabled:cursor-not-allowed disabled:opacity-50 transition-all"
                  >
                    {isCheckingRemoteFirewall ||
                    isConfiguringRemoteFirewall ||
                    isStartingRemoteServer ? (
                      <RefreshCw size={14} className="animate-spin" />
                    ) : (
                      <Play size={14} />
                    )}
                    <span>{t("serverDetail.start")}</span>
                  </button>
                )}
              </div>
            )}

            {/* Group 3: Stats tags */}
            <div className="flex items-center gap-3">
              <div className="bg-[#22272b] px-4 py-2 rounded-xl border border-white/5 flex items-center gap-2 shadow-sm">
                <span className="text-[10px] text-gray-500 uppercase font-black tracking-wider">
                  {t("serverDetail.activeMods")}
                </span>
                <span className="text-sm font-black text-orange-400">{activatedMods.length}</span>
              </div>
              <div className="bg-[#22272b] px-4 py-2 rounded-xl border border-white/5 flex items-center gap-2 shadow-sm">
                <span className="text-[10px] text-gray-500 uppercase font-black tracking-wider">
                  {t("serverDetail.maxPlayers")}
                </span>
                <span className="text-sm font-black text-white">{server.maxPlayers || "-"}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Search & Filter bar */}
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="relative group max-w-md flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within:text-orange-400 transition-colors" size={18} />
          <input
            type="text"
            placeholder={t("serverDetail.filter")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full bg-[#2b3238] border border-white/5 rounded-xl py-2.5 pl-10 pr-4 text-sm focus:outline-none focus:border-orange-400/50 transition-all placeholder:text-gray-600"
          />
        </div>

        {server && (
          <div className="flex items-center gap-2 shrink-0">
            <button
              type="button"
              onClick={() => setShowLogsModal(true)}
              className="flex items-center gap-2 rounded-xl border border-cyan-500/30 bg-cyan-500/10 px-4 py-2.5 text-xs font-bold text-cyan-200 transition-all hover:bg-cyan-500/20 hover:border-cyan-500/50 shadow-md shadow-cyan-950/20"
            >
              <FileText size={16} className="text-cyan-400" />
              <span>{t("serverDetail.viewLogs")}</span>
            </button>

            <button
              type="button"
              onClick={() => setShowFixWorkshopIdsModal(true)}
              className="flex items-center gap-2 rounded-xl border border-orange-500/30 bg-orange-500/10 px-4 py-2.5 text-xs font-bold text-orange-200 transition-all hover:bg-orange-500/20 hover:border-orange-500/50 shadow-md shadow-orange-950/20"
            >
              <Wand2 size={16} className="text-orange-400" />
              <span>{t("serverDetail.organizeWorkshopIds")}</span>
            </button>
          </div>
        )}
      </div>

      {/* Lists */}
      <div className="flex flex-col gap-6 pb-10">
        {mapInstallError && (
          <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
            {mapInstallError}
          </div>
        )}

        {activationError && (
          <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
            {activationError}
          </div>
        )}

        {serverFileOpenError && (
          <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
            {serverFileOpenError}
          </div>
        )}

        {incompatibleActiveIds.length > 0 && (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-orange-500/20 bg-orange-500/10 px-5 py-4 text-sm text-orange-200">
            <span>{t("serverDetail.incompatibleWarning", { count: incompatibleActiveIds.length, build: server.gameBuild.toUpperCase() })}</span>
            <button
              type="button"
              onClick={() => setShowIncompatibleMods(true)}
              className="shrink-0 rounded-xl border border-orange-400/20 bg-orange-400/10 px-3 py-2 text-xs font-bold text-orange-200 transition-colors hover:bg-orange-400/20"
            >
              {t("serverDetail.viewMods")}
            </button>
          </div>
        )}

        <ServerModList
          title={t("serverDetail.activated")}
          mods={filteredActivated}
          emptyMessage={t("serverDetail.noActivated")}
          isExpanded={isActivatedExpanded}
          action="deactivate"
          onToggleExpanded={() => setIsActivatedExpanded(!isActivatedExpanded)}
          onAction={handleDeactivateClick}
          onSelect={setSelectedMod}
          onContextMenu={handleActiveModContextMenu}
          incompatibleModIds={incompatibleActiveIdSet}
          workshopMappings={workshopMappings}
        />

        <ServerModList
          title={t("serverDetail.available")}
          mods={filteredAvailable}
          emptyMessage={t("serverDetail.noAvailable")}
          isExpanded={isAvailableExpanded}
          action="activate"
          onToggleExpanded={() => setIsAvailableExpanded(!isAvailableExpanded)}
          onAction={handleActivateClick}
          onSelect={setSelectedMod}
          onInstallMap={setPendingMapInstall}
          paginate
          paginationResetKey={`${server.id}:${server.gameBuild}:${search}`}
          workshopMappings={workshopMappings}
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

      {selectedMod && (
        <ServerModDetailsModal
          mod={selectedMod}
          onClose={() => setSelectedMod(null)}
          workshopMappings={workshopMappings}
          onSaveWorkshopMapping={onSaveWorkshopMapping}
        />
      )}

      {showFixWorkshopIdsModal && server && (
        <FixWorkshopIdsModal
          server={server}
          allMods={allMods}
          workshopMappings={workshopMappings}
          onClose={() => setShowFixWorkshopIdsModal(false)}
          onSaveWorkshopMapping={onSaveWorkshopMapping || (async () => {})}
          onApplyServerMods={async (srv, activeModIds) => {
            if (onUpdateServerMods) {
              await onUpdateServerMods(srv, activeModIds)
            }
          }}
        />
      )}

      {showLogsModal && server && (
        <ServerLogsModal
          server={server}
          remoteConnection={remoteConnection}
          onClose={() => setShowLogsModal(false)}
        />
      )}

      {serverFilePreview && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-md animate-in fade-in duration-200"
          onClick={() => {
            setServerFilePreview(null)
          }}
        >
          <div
            className="flex h-[85vh] max-h-[90vh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-200"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between gap-4 border-b border-white/10 bg-[#2b3238] px-6 py-4">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <p className="text-[10px] font-black uppercase tracking-[0.2em] text-orange-400">server.ini</p>
                  <span className="text-white/20">•</span>
                  <span className="text-xs font-mono text-gray-300 truncate">
                    {serverFilePreview.fileName || serverFilePreview.file_name || server?.fileName || ""}
                  </span>
                </div>
                <p className="mt-0.5 truncate font-mono text-[10px] text-gray-500">{serverFilePreview.path || ""}</p>
              </div>

              {/* Botões de Navegação Rápida */}
              <div className="flex items-center gap-2 shrink-0">
                <button
                  type="button"
                  onClick={() => {
                    const el = document.getElementById("ini-line-mods")
                    if (el) {
                      el.scrollIntoView({ behavior: "smooth", block: "center" })
                      el.classList.remove("highlight-subtle")
                      void el.offsetWidth
                      el.classList.add("highlight-subtle")
                    }
                  }}
                  className="flex items-center gap-1.5 rounded-xl border border-sky-500/30 bg-sky-500/10 px-3 py-1.5 text-xs font-bold text-sky-200 transition-all hover:bg-sky-500/20 hover:border-sky-500/50 shadow-sm"
                >
                  <PackageCheck size={14} className="text-sky-400" />
                  <span>Mods</span>
                </button>

                <button
                  type="button"
                  onClick={() => {
                    const el = document.getElementById("ini-line-workshop")
                    if (el) {
                      el.scrollIntoView({ behavior: "smooth", block: "center" })
                      el.classList.remove("highlight-subtle")
                      void el.offsetWidth
                      el.classList.add("highlight-subtle")
                    }
                  }}
                  className="flex items-center gap-1.5 rounded-xl border border-orange-500/30 bg-orange-500/10 px-3 py-1.5 text-xs font-bold text-orange-200 transition-all hover:bg-orange-500/20 hover:border-orange-500/50 shadow-sm"
                >
                  <Hash size={14} className="text-orange-400" />
                  <span>Workshop IDs</span>
                </button>

                <div className="h-4 w-[1px] bg-white/10 mx-1" />

                <button
                  type="button"
                  onClick={() => setServerFilePreview(null)}
                  className="rounded-xl border border-white/10 bg-[#22272b] px-3.5 py-1.5 text-xs font-bold text-gray-300 transition-colors hover:border-orange-400/30 hover:text-orange-300"
                >
                  Fechar
                </button>
              </div>
            </div>
            <IniContentRenderer content={serverFilePreview.content || ""} />
          </div>
        </div>
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
          onCancel={() => {
            setPendingActivation(null)
          }}
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

      {pendingMapInstall && (
        <MapInstallConfirmationModal
          mod={pendingMapInstall}
          onCancel={() => setPendingMapInstall(null)}
          onConfirm={() => void installMap(pendingMapInstall)}
        />
      )}

      {pendingBuild && pendingBuild !== server.gameBuild && (
        <ChangeServerBuildModal
          currentBuild={server.gameBuild}
          nextBuild={pendingBuild}
          activeModsCount={safeActiveIds.length}
          isSaving={isChangingBuild}
          onCancel={() => setPendingBuild(null)}
          onConfirm={() => void changeBuild()}
        />
      )}

      {showIncompatibleMods && incompatibleActiveMods.length > 0 && (
        <IncompatibleModsModal
          gameBuild={server.gameBuild}
          mods={incompatibleActiveMods}
          onClose={() => setShowIncompatibleMods(false)}
        />
      )}
      {missingDependency && (
        <MissingDependencyModal
          mod={missingDependency.mod}
          dependencyId={missingDependency.dependencyId}
          onClose={() => setMissingDependency(null)}
          onDownloaded={onDependencyDownloaded ?? onRefreshMods}
          onOpenSettings={onOpenSettings}
          remoteConnection={remoteConnection}
        />
      )}
      </div>
    </div>
  )
}

function dedupeModsForServer(mods: ZomboidMod[]) {
  const seen = new Set<string>()
  const result: ZomboidMod[] = []

  for (const mod of mods) {
    if (!mod?.id) continue

    const keys = modIdentityKeys(mod)
    if (keys.some((key) => seen.has(key))) continue

    keys.forEach((key) => seen.add(key))
    result.push(mod)
  }

  return result
}

function modIdentityKeys(mod: ZomboidMod) {
  const keys = [mod.id, mod.workshopId, ...(mod.variants ?? []).map((variant) => variant.id)]
    .map((value) => String(value ?? "").trim().toLowerCase())
    .filter(Boolean)

  return keys.length > 0 ? Array.from(new Set(keys)) : [`path:${mod.packagePath || mod.path}`]
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

  return i18n.t("serverTest.fallbackError")
}

function createMissingActiveMod(modId: string): ZomboidMod {
  return {
    id: modId,
    name: modId,
    author: i18n.t("mods.unknownAuthor"),
    version: "-",
    workshopId: "",
    description: i18n.t("mods.activeMissingDescription"),
    size: "-",
    isInstalled: false,
    source: "missing",
    path: "",
    dependencies: [],
    mapNames: [],
    compatibleBuilds: [],
    variants: [],
    packagePath: "",
  }
}
