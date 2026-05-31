import { Activity, ChevronRight, Eye, EyeOff, FolderOpen, Plus, RefreshCw, Server, Users, Wifi } from "lucide-react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import type { ZomboidServer } from "@/types/server"

type DashboardProps = {
  servers: ZomboidServer[]
  isLoading: boolean
  error: string | null
  onRefresh: () => void
  onCreateServer: () => void
  searchQuery: string
  onServerClick: (server: ZomboidServer) => void
}

const HIDDEN_SERVERS_KEY = "pzmm_hidden_servers"

export function Dashboard({
  servers,
  isLoading,
  error,
  onRefresh,
  onCreateServer,
  searchQuery,
  onServerClick
}: DashboardProps) {
  const { t } = useTranslation()
  const [hiddenServerIds, setHiddenServerIds] = useState<Set<string>>(new Set())
  const [showHidden, setShowHidden] = useState(false)
  const [contextMenu, setContextMenu] = useState<{ server: ZomboidServer; x: number; y: number } | null>(null)

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

  const normalizedSearch = searchQuery.trim().toLowerCase()

  const allFiltered = servers.filter((server) => {
    if (!normalizedSearch) return true
    return (
      server.name.toLowerCase().includes(normalizedSearch) ||
      server.fileName.toLowerCase().includes(normalizedSearch) ||
      server.port.includes(searchQuery)
    )
  })

  const visibleServers = allFiltered.filter(s => !hiddenServerIds.has(s.id))
  const hiddenServers = allFiltered.filter(s => hiddenServerIds.has(s.id))

  return (
    <div className="p-8 h-full overflow-y-auto custom-scrollbar relative" onClick={() => setContextMenu(null)}>
      <div className="flex justify-between items-center mb-8">
        <div>
          <h2 className="text-3xl font-bold tracking-tight">{t("dashboard.title")}</h2>
          <p className="text-gray-400 mt-1">{t("dashboard.description")}</p>
        </div>

        <button
          className="flex items-center gap-2 bg-[#2b3238] border border-white/5 text-gray-300 hover:text-white hover:border-orange-400/30 px-4 py-2 rounded-xl transition-all"
          onClick={onRefresh}
        >
          <RefreshCw size={18} className={isLoading ? "animate-spin" : ""} />
          {t("common.refresh")}
        </button>
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
        {!isLoading && !error && visibleServers.length === 0 && <EmptyServerCard hasSearch={Boolean(normalizedSearch)} />}

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
        </div>
      )}
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


function EmptyServerCard({ hasSearch }: { hasSearch: boolean }) {
  const { t } = useTranslation()

  return (
    <div className="min-h-[220px] flex flex-col items-center justify-center gap-4 bg-[#2b3238] border border-white/5 rounded-2xl p-6 text-center">
      <div className="p-4 bg-[#22272b] rounded-full text-gray-400">
        <FolderOpen size={32} />
      </div>
      <div>
        <p className="text-lg font-semibold">{t("dashboard.noServers")}</p>
        <p className="text-sm text-gray-500">
          {t(hasSearch ? "dashboard.emptySearch" : "dashboard.emptyHint")}
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
