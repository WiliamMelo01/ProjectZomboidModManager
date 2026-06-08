import { Activity, ChevronRight, Eye, EyeOff, FolderOpen, Plus, RefreshCw, Server, Trash2, Users, Wifi } from "lucide-react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import type { GameBuild, ZomboidServer } from "@/types/server"

type DashboardProps = {
  servers: ZomboidServer[]
  isLoading: boolean
  error: string | null
  onRefresh: () => void
  onCreateServer: () => void
  searchQuery: string
  onServerClick: (server: ZomboidServer) => void
  onDeleteServer: (server: ZomboidServer) => Promise<void>
}

const HIDDEN_SERVERS_KEY = "pzmm_hidden_servers"

export function Dashboard({
  servers,
  isLoading,
  error,
  onRefresh,
  onCreateServer,
  searchQuery,
  onServerClick,
  onDeleteServer,
}: DashboardProps) {
  const { t } = useTranslation()
  const [hiddenServerIds, setHiddenServerIds] = useState<Set<string>>(new Set())
  const [showHidden, setShowHidden] = useState(false)
  const [filterBuild, setFilterBuild] = useState<"all" | GameBuild>("all")
  const [contextMenu, setContextMenu] = useState<{ server: ZomboidServer; x: number; y: number } | null>(null)
  const [pendingDeleteServer, setPendingDeleteServer] = useState<ZomboidServer | null>(null)
  const [isDeletingServer, setIsDeletingServer] = useState(false)

  useEffect(() => {
    const stored = window.localStorage.getItem(HIDDEN_SERVERS_KEY)
    if (stored) {
      try {
        setHiddenServerIds(new Set(JSON.parse(stored)))
      } catch (e) {
        console.error("Failed to parse hidden servers", e)
      }
    }

  }, [])

  const toggleHideServer = (serverId: string) => {
    const next = new Set(hiddenServerIds)
    if (next.has(serverId)) {
      next.delete(serverId)
    } else {
      next.add(serverId)
    }
    setHiddenServerIds(next)
    window.localStorage.setItem(HIDDEN_SERVERS_KEY, JSON.stringify(Array.from(next)))
    setContextMenu(null)
  }

  const handleContextMenu = (event: React.MouseEvent, server: ZomboidServer) => {
    event.preventDefault()
    setContextMenu({ server, x: event.clientX, y: event.clientY })
  }

  const requestDeleteServer = (server: ZomboidServer) => {
    setPendingDeleteServer(server)
    setContextMenu(null)
  }

  const confirmDeleteServer = async () => {
    if (!pendingDeleteServer) return

    setIsDeletingServer(true)
    try {
      await onDeleteServer(pendingDeleteServer)
      setPendingDeleteServer(null)
    } finally {
      setIsDeletingServer(false)
    }
  }

  const normalizedSearch = searchQuery.trim().toLowerCase()

  const allFiltered = servers.filter((server) => {
    const matchesBuild = filterBuild === "all" || server.gameBuild === filterBuild
    const matchesSearch =
      !normalizedSearch ||
      server.name.toLowerCase().includes(normalizedSearch) ||
      server.fileName.toLowerCase().includes(normalizedSearch) ||
      server.port.includes(searchQuery)

    return matchesBuild && matchesSearch
  })

  const visibleServers = allFiltered.filter(s => !hiddenServerIds.has(s.id))
  const hiddenServers = allFiltered.filter(s => hiddenServerIds.has(s.id))

  return (
    <div className="p-8 h-full overflow-y-auto custom-scrollbar relative" onClick={() => setContextMenu(null)}>
      <div className="flex flex-col justify-between gap-6 mb-8 lg:flex-row lg:items-center">
        <div>
          <h2 className="text-3xl font-bold tracking-tight">{t("dashboard.title")}</h2>
          <p className="text-gray-400 mt-1">{t("dashboard.description")}</p>
        </div>

        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <div className="flex bg-[#2b3238] p-1 rounded-xl border border-white/5 shadow-inner">
            {(["all", "b41", "b42"] as const).map((build) => (
              <button
                key={build}
                type="button"
                onClick={() => setFilterBuild(build)}
                className={`px-3 py-1.5 rounded-lg text-xs font-bold uppercase transition-all ${
                  filterBuild === build ? "bg-orange-500 text-white shadow-lg" : "text-gray-400 hover:text-white"
                }`}
              >
                {build === "all" ? t("dashboard.versions") : build}
              </button>
            ))}
          </div>

          <button
            className="flex items-center justify-center gap-2 bg-[#2b3238] border border-white/5 text-gray-300 hover:text-white hover:border-orange-400/30 px-4 py-2 rounded-xl transition-all"
            onClick={onRefresh}
          >
            <RefreshCw size={18} className={isLoading ? "animate-spin" : ""} />
            {t("common.refresh")}
          </button>
        </div>
      </div>

      {error && (
        <div className="mb-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
          {error}
        </div>
      )}

      {isLoading && (
        <div className="mb-6 rounded-2xl border border-white/5 bg-[#2b3238] px-5 py-4 text-sm text-gray-300">
          {t("dashboard.loading")}
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mb-12">
        {!isLoading && !error && visibleServers.length === 0 && (
          <EmptyServerCard hasSearch={Boolean(normalizedSearch)} hasBuildFilter={filterBuild !== "all"} />
        )}

        {visibleServers.map((server) => (
          <ServerCard
            key={server.id}
            server={server}
            onClick={() => onServerClick(server)}
            onContextMenu={(e) => handleContextMenu(e, server)}
          />
        ))}

        <AddServerCard onClick={onCreateServer} />
      </div>

      {/* Hidden Servers Section */}
      {hiddenServers.length > 0 && (
        <div className="mt-12 pt-8 border-t border-white/5">
          <button
            onClick={() => setShowHidden(!showHidden)}
            className="flex items-center gap-3 mb-6 px-2 py-2 hover:bg-white/5 rounded-xl transition-colors w-full text-left group"
          >
            <EyeOff size={18} className="text-gray-500" />
            <h3 className="text-lg font-bold text-gray-500 uppercase tracking-tighter">{t("dashboard.hidden")}</h3>
            <div className="h-px flex-1 bg-white/5" />
            <span className="text-xs font-mono text-gray-600 bg-[#2b3238] px-2 py-0.5 rounded-full">{hiddenServers.length}</span>
            <ChevronRight
              size={20}
              className={`text-gray-600 transition-transform duration-300 ${showHidden ? "rotate-90" : ""}`}
            />
          </button>

          <div className={`grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 transition-all duration-300 origin-top ${
            showHidden ? "opacity-100 scale-y-100 h-auto" : "opacity-0 scale-y-0 h-0 overflow-hidden"
          }`}>
            {hiddenServers.map((server) => (
              <ServerCard
                key={server.id}
                server={server}
                onClick={() => onServerClick(server)}
                onContextMenu={(e) => handleContextMenu(e, server)}
                isHidden
              />
            ))}
          </div>
        </div>
      )}

      {/* Context Menu */}
      {contextMenu && (
        <div
          className="fixed z-[100] w-48 overflow-hidden rounded-xl border border-white/10 bg-[#1e2327] py-1.5 shadow-2xl shadow-black/40"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            onClick={() => toggleHideServer(contextMenu.server.id)}
            className="flex w-full items-center gap-3 px-4 py-2 text-sm font-medium text-gray-300 transition-colors hover:bg-orange-500/10 hover:text-orange-300"
          >
            {hiddenServerIds.has(contextMenu.server.id) ? (
              <>
                <Eye size={16} />
                {t("dashboard.show")}
              </>
            ) : (
              <>
                <EyeOff size={16} />
                {t("dashboard.hide")}
              </>
            )}
          </button>
          <button
            onClick={() => requestDeleteServer(contextMenu.server)}
            className="flex w-full items-center gap-3 px-4 py-2 text-sm font-medium text-red-300 transition-colors hover:bg-red-500/10 hover:text-red-200"
          >
            <Trash2 size={16} />
            {t("dashboard.delete")}
          </button>
        </div>
      )}

      {pendingDeleteServer && (
        <DeleteServerModal
          server={pendingDeleteServer}
          isDeleting={isDeletingServer}
          onCancel={() => {
            if (!isDeletingServer) {
              setPendingDeleteServer(null)
            }
          }}
          onConfirm={() => void confirmDeleteServer()}
        />
      )}
    </div>
  )
}

function DeleteServerModal({
  server,
  isDeleting,
  onCancel,
  onConfirm,
}: {
  server: ZomboidServer
  isDeleting: boolean
  onCancel: () => void
  onConfirm: () => void
}) {
  const { t } = useTranslation()

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md"
      onClick={onCancel}
    >
      <div
        role="dialog"
        aria-modal="true"
        className="w-full max-w-md rounded-3xl border border-white/10 bg-[#22272b] p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start gap-4">
          <div className="rounded-2xl border border-red-500/20 bg-red-500/10 p-3 text-red-300">
            <Trash2 size={24} />
          </div>
          <div className="min-w-0">
            <h3 className="text-xl font-black text-white">{t("dashboard.deleteTitle")}</h3>
            <p className="mt-2 text-sm leading-relaxed text-gray-400">
              {t("dashboard.deleteBody", { name: server.name })}
            </p>
            <p className="mt-3 break-all rounded-xl border border-white/5 bg-[#1e2327] px-3 py-2 font-mono text-xs text-gray-500">
              {server.fileName}
            </p>
          </div>
        </div>

        <div className="mt-6 flex flex-col-reverse gap-3 sm:flex-row sm:justify-end">
          <button
            type="button"
            disabled={isDeleting}
            onClick={onCancel}
            className="rounded-xl border border-white/10 px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t("common.cancel")}
          </button>
          <button
            type="button"
            disabled={isDeleting}
            onClick={onConfirm}
            className="flex items-center justify-center gap-2 rounded-xl bg-red-500 px-4 py-2 text-sm font-black text-white transition-colors hover:bg-red-600 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {isDeleting ? <RefreshCw size={16} className="animate-spin" /> : <Trash2 size={16} />}
            {isDeleting ? t("dashboard.deleting") : t("dashboard.deleteConfirm")}
          </button>
        </div>
      </div>
    </div>
  )
}

function ServerCard({
  server,
  onClick,
  onContextMenu,
  isHidden
}: {
  server: ZomboidServer;
  onClick: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  isHidden?: boolean
}) {
  const { t } = useTranslation()
  const isOnline = server.status === "online"

  return (
    <div
      onClick={onClick}
      onContextMenu={onContextMenu}
      className={`group relative border rounded-2xl p-6 transition-all duration-300 overflow-hidden cursor-pointer ${
        isHidden
          ? "bg-[#2b3238]/40 border-white/5 opacity-60 hover:opacity-100"
          : "bg-[#2b3238] border-white/5 hover:border-orange-400/40 hover:bg-[#353c42] hover:shadow-[0_0_20px_rgba(251,146,60,0.1)]"
      }`}
    >
      <div className="absolute -right-8 -top-8 w-24 h-24 bg-orange-400/5 rounded-full blur-3xl group-hover:bg-orange-400/10 transition-colors duration-300" />

      <div className="flex justify-between items-start mb-4">
        <div
          className={`flex items-center gap-2 px-2.5 py-0.5 rounded-full text-xs font-medium ${
            isOnline ? "bg-green-500/10 text-green-400" : "bg-red-500/10 text-red-400"
          }`}
        >
          <div className={`w-1.5 h-1.5 rounded-full ${isOnline ? "bg-green-400 animate-pulse" : "bg-red-400"}`} />
          {server.status.toUpperCase()}
        </div>
        <span className="text-xs text-gray-500 font-mono">{server.fileName}</span>
      </div>

      <div className="flex items-center gap-3 mb-6">
        <div className="p-2.5 bg-[#22272b] rounded-xl group-hover:text-orange-400 transition-colors">
          <Server size={24} />
        </div>
        <h3 className="text-xl font-semibold truncate">{server.name}</h3>
        <span className="rounded-full border border-orange-400/20 bg-orange-400/10 px-2 py-0.5 text-[10px] font-black uppercase text-orange-300">
          {server.gameBuild}
        </span>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2 text-gray-400">
            <Users size={16} />
            <span>{t("serverDetail.maxPlayers")}</span>
          </div>
          <span className="font-medium">{server.maxPlayers || "-"}</span>
        </div>

        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2 text-gray-400">
            <Wifi size={16} />
            <span>{t("serverDetail.port")}</span>
          </div>
          <span className="font-mono text-xs text-gray-300">{server.port}</span>
        </div>

        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2 text-gray-400">
            <Activity size={16} />
            <span>Mods</span>
          </div>
          <span className="font-medium">{server.modsCount}</span>
        </div>
      </div>
    </div>
  )
}


function EmptyServerCard({ hasSearch, hasBuildFilter }: { hasSearch: boolean; hasBuildFilter: boolean }) {
  const { t } = useTranslation()
  const messageKey = hasSearch
    ? "dashboard.emptySearch"
    : hasBuildFilter
      ? "dashboard.emptyBuild"
      : "dashboard.emptyHint"

  return (
    <div className="min-h-[220px] flex flex-col items-center justify-center gap-4 bg-[#2b3238] border border-white/5 rounded-2xl p-6 text-center">
      <div className="p-4 bg-[#22272b] rounded-full text-gray-400">
        <FolderOpen size={32} />
      </div>
      <div>
        <p className="text-lg font-semibold">{t("dashboard.noServers")}</p>
        <p className="text-sm text-gray-500">
          {t(messageKey)}
        </p>
      </div>
    </div>
  )
}

function AddServerCard({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation()

  return (
    <button
      onClick={onClick}
      className="group h-full min-h-[220px] flex flex-col items-center justify-center gap-4 bg-transparent border-2 border-dashed border-white/10 rounded-2xl transition-all duration-300 hover:border-orange-400/50 hover:bg-orange-400/5 hover:shadow-[0_0_20px_rgba(251,146,60,0.05)]"
    >
      <div className="p-4 bg-[#2b3238] rounded-full group-hover:scale-110 group-hover:text-orange-400 transition-all duration-300">
        <Plus size={32} />
      </div>
      <div className="text-center">
        <p className="text-lg font-semibold">{t("dashboard.create")}</p>
        <p className="text-sm text-gray-500">{t("dashboard.addServer")}</p>
      </div>
    </button>
  )
}
