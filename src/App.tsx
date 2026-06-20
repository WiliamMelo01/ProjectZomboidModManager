import { useEffect, useMemo, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { Box, Download, Server, Settings } from "lucide-react"
import { useTranslation } from "react-i18next"

import { AppHeader, type AppNotification } from "@/components/AppHeader"
import { AppSidebar } from "@/components/AppSidebar"
import { CreateServerModal } from "@/components/CreateServerModal"
import { Dashboard } from "@/components/Dashboard"
import { DownloadMods } from "@/components/DownloadMods"
import { DownloadProgressCard } from "@/components/DownloadProgressCard"
import { LoadingModsPanel } from "@/components/LoadingModsPanel"
import { ModsList } from "@/components/ModsList"
import { RemoteSteamCmdModal } from "@/components/RemoteSteamCmdModal"
import { RemoteTerminalModal } from "@/components/RemoteTerminalModal"
import { ServerConfigurationModal } from "@/components/ServerConfigurationModal"
import { ServerDetail } from "@/components/ServerDetail"
import { ServerTestPanel } from "@/components/ServerTestPanel"
import { Settings as SettingsView } from "@/components/Settings"
import { WorkspaceSelector } from "@/components/WorkspaceSelector"
import { WorkshopWindow } from "@/components/WorkshopWindow"
import { useModsLibrary } from "@/hooks/useModsLibrary"
import { useWorkshopDownloadManager } from "@/hooks/useWorkshopDownloadManager"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import { getErrorMessage } from "@/lib/errors"
import { findModForServerId, resolveModForBuild } from "@/lib/modBuilds"
import { clearModsLibraryCache } from "@/lib/modsCache"
import { getActiveDependencyChain, getWorkshopIdsForModIds } from "@/lib/serverMods"
import { readServersCache, writeServersCache } from "@/lib/serversCache"
import { invokeTauri } from "@/lib/tauri"
import type { ZomboidMod } from "@/types/mod"
import type { ServerIniSettings, ZomboidServer } from "@/types/server"

type ServerTestEvent = {
  serverId: string
  event: "started" | "line" | "finished" | "error"
}

type DeleteServerResult = {
  backupPath: string
}

function App() {
  if (window.location.hash.startsWith("#/workshop")) {
    return <WorkshopWindow />
  }

  const [workspaceMode, setWorkspaceMode] = useState<"local" | "remote" | null>(null)
  const [remoteConnection, setRemoteConnection] = useState<RemoteConnectionDraft | null>(null)

  if (workspaceMode === null) {
    return (
      <WorkspaceSelector
        onSelectLocal={() => {
          setRemoteConnection(null)
          setWorkspaceMode("local")
        }}
        onSelectRemote={(connection) => {
          setRemoteConnection(connection)
          setWorkspaceMode("remote")
        }}
      />
    )
  }

  return (
    <LocalWorkspaceApp
      onChangeWorkspace={() => setWorkspaceMode(null)}
      remoteConnection={workspaceMode === "remote" ? remoteConnection : null}
    />
  )
}

function LocalWorkspaceApp({
  onChangeWorkspace,
  remoteConnection,
}: {
  onChangeWorkspace: () => void
  remoteConnection: RemoteConnectionDraft | null
}) {
  const [isCreateServerModalOpen, setIsCreateServerModalOpen] = useState(false)
  const [isRemoteSteamCmdModalOpen, setIsRemoteSteamCmdModalOpen] = useState(false)
  const [isRemoteTerminalModalOpen, setIsRemoteTerminalModalOpen] = useState(false)
  const isRemoteWorkspace = remoteConnection !== null
  const workspaceCacheId = remoteConnection
    ? `remote:${[
      remoteConnection.name,
      remoteConnection.username,
      remoteConnection.host,
      remoteConnection.port,
    ].map((part) => encodeURIComponent(part.trim())).join(":")}`
    : "local"
  const modsCacheKey = workspaceCacheId === "local" ? "pzmm:mods-library" : `pzmm:mods-library:${workspaceCacheId}`
  const serversCacheKey = workspaceCacheId === "local" ? "pzmm:servers" : `pzmm:servers:${workspaceCacheId}`
  const cachedServers = useMemo(() => readServersCache(serversCacheKey), [serversCacheKey])
  const [serverConfigTarget, setServerConfigTarget] = useState<ZomboidServer | null>(null)
  const [activeTab, setActiveTab] = useState("dashboard")
  const [selectedServer, setSelectedServer] = useState<ZomboidServer | null>(null)
  const [servers, setServers] = useState<ZomboidServer[]>(cachedServers?.servers ?? [])
  const [serversError, setServersError] = useState<string | null>(null)
  const [isLoadingServers, setIsLoadingServers] = useState(!cachedServers)
  const [searchQuery, setSearchQuery] = useState("")
  const [notifications, setNotifications] = useState<AppNotification[]>([])
  const [runningServerTestId, setRunningServerTestId] = useState<string | null>(null)
  const { t } = useTranslation()
  const {
    mods,
    modsCount,
    modsError,
    isLoadingMods,
    isInstallingAllMods,
    hasLoadedMods,
    loadMods,
    ensureModsLoaded,
    installMods,
    installAllUninstalledMods,
    loadModsInBackground,
  } = useModsLibrary({
    listCommand: isRemoteWorkspace ? "list_remote_zomboid_mods" : "list_zomboid_mods",
    listArgs: isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : undefined,
    installCommand: isRemoteWorkspace ? "install_remote_zomboid_mod" : "install_zomboid_mod",
    installArgs: isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : undefined,
    clearCacheCommand: isRemoteWorkspace ? "clear_remote_zomboid_mods_cache" : undefined,
    clearCacheArgs: isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : undefined,
    reloadAfterInstall: isRemoteWorkspace,
    useCache: true,
    cacheKey: modsCacheKey,
  })
  const navItems = useMemo(
    () => [
      { id: "dashboard", label: t("nav.servers"), icon: Server },
      { id: "mods", label: "Mods", icon: Box, badge: String(modsCount) },
      { id: "download", label: t("nav.download"), icon: Download },
      { id: "settings", label: t("nav.settings"), icon: Settings },
    ],
    [modsCount, t],
  )
  const downloadManager = useWorkshopDownloadManager({
    isDownloadScreenActive: activeTab === "download",
    remoteConnection,
    onDownloadFinished: loadMods,
    onNotification: addNotification,
  })

  function normalizeServers(nextServers: ZomboidServer[]) {
    return [...nextServers].sort((left, right) =>
      left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
    )
  }

  function applyServers(nextServers: ZomboidServer[]) {
    const sortedServers = normalizeServers(nextServers)

    setServers(sortedServers)
    writeServersCache(sortedServers, serversCacheKey)
    return sortedServers
  }

  function updateServers(updater: (currentServers: ZomboidServer[]) => ZomboidServer[]) {
    setServers((currentServers) => {
      const nextServers = normalizeServers(updater(currentServers))
      writeServersCache(nextServers, serversCacheKey)
      return nextServers
    })
  }

  async function loadServers() {
    if (isRemoteWorkspace) {
      if (!remoteConnection) return
      setIsLoadingServers(true)
      setServersError(null)

      try {
        const foundServers = await invokeTauri<ZomboidServer[]>("list_remote_zomboid_servers", {
          connection: remoteConnection,
        })
        applyServers(foundServers)
        setSelectedServer((current) =>
          current ? foundServers.find((server) => server.id === current.id) ?? null : null,
        )
      } catch (error) {
        const message = getErrorMessage(error)
        setServersError(message)
      } finally {
        setIsLoadingServers(false)
      }
      return
    }

    setIsLoadingServers(true)
    setServersError(null)

    try {
      const foundServers = await invokeTauri<ZomboidServer[]>("list_zomboid_servers")
      applyServers(foundServers)
      setSelectedServer((current) =>
        current ? foundServers.find((server) => server.id === current.id) ?? null : null,
      )
    } catch (error) {
      const message = getErrorMessage(error)
      setServersError(message)
    } finally {
      setIsLoadingServers(false)
    }
  }

  async function updateServerMods(server: ZomboidServer, activeModIds: string[]) {
    setServersError(null)
    const workshopIds = getWorkshopIdsForModIds(activeModIds, mods, server.gameBuild)

    await invokeTauri<void>(isRemoteWorkspace && remoteConnection ? "update_remote_zomboid_server_mods" : "update_zomboid_server_mods", {
      ...(isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : {}),
      serverId: server.id,
      modIds: activeModIds,
      workshopIds,
    })

    const updatedServer = {
      ...server,
      activeModIds: activeModIds ?? [],
      modsCount: activeModIds.length,
    }

    setSelectedServer(updatedServer)
    updateServers((currentServers) =>
      currentServers.map((currentServer) => (currentServer.id === server.id ? updatedServer : currentServer)),
    )
  }

  async function toggleServerMod(server: ZomboidServer, mod: ZomboidMod, action: "activate" | "deactivate") {
    const activeModIds = server.activeModIds ?? []
    const resolvedMod = action === "deactivate" ? mod : resolveModForBuild(mod, server.gameBuild)
    if (!resolvedMod) return
    const normalizedModId = resolvedMod.id.toLowerCase()
    const nextActiveModIds =
      action === "activate"
        ? activeModIds.some((modId) => modId.toLowerCase() === normalizedModId)
          ? activeModIds
          : [...activeModIds, resolvedMod.id]
        : activeModIds.filter((modId) => modId.toLowerCase() !== normalizedModId)

    try {
      await updateServerMods(server, nextActiveModIds)
    } catch (error) {
      setServersError(getErrorMessage(error))
    }
  }

  async function moveServerMod(server: ZomboidServer, mod: ZomboidMod, position: "start" | "end") {
    const resolvedMod = resolveModForBuild(mod, server.gameBuild)
    if (!resolvedMod) return
    const normalizedModId = resolvedMod.id.toLowerCase()
    const activeModIds = server.activeModIds ?? []
    const activeModIdKeys = new Set(activeModIds.map((modId) => modId.toLowerCase()))
    const modsById = new Map(
      mods.flatMap((item) => item.variants.map((variant) => [
        variant.id.toLowerCase(),
        { ...item, id: variant.id, path: variant.path, dependencies: variant.dependencies, mapNames: variant.mapNames },
      ] as const)),
    )
    const moveModIds =
      position === "start"
        ? getActiveDependencyChain(mod, modsById, activeModIdKeys)
        : [resolvedMod.id]
    const moveModIdKeys = new Set(moveModIds.map((modId) => modId.toLowerCase()))
    const remainingModIds = activeModIds.filter((modId) => !moveModIdKeys.has(modId.toLowerCase()))

    if (!activeModIdKeys.has(normalizedModId)) {
      return
    }

    const nextActiveModIds = position === "start" ? [...moveModIds, ...remainingModIds] : [...remainingModIds, resolvedMod.id]

    try {
      await updateServerMods(server, nextActiveModIds)
    } catch (error) {
      setServersError(getErrorMessage(error))
    }
  }

  async function activateServerMods(server: ZomboidServer, modsToActivate: ZomboidMod[]) {
    const nextActiveModIds = [...(server.activeModIds ?? [])]
    const activeModIdsSet = new Set(nextActiveModIds.map((modId) => modId.toLowerCase()))

    for (const mod of modsToActivate) {
      const resolvedMod = resolveModForBuild(mod, server.gameBuild)
      if (!resolvedMod) continue
      const normalizedModId = resolvedMod.id.toLowerCase()

      if (!activeModIdsSet.has(normalizedModId)) {
        nextActiveModIds.push(resolvedMod.id)
        activeModIdsSet.add(normalizedModId)
      }
    }

    try {
      await updateServerMods(server, nextActiveModIds)
    } catch (error) {
      setServersError(getErrorMessage(error))
    }
  }

  async function createServer(data: { name: string; modIds: string[]; gameBuild: "b41" | "b42"; maxPlayers: number }) {
    const resolvedModIds = data.modIds.flatMap((modId) => {
      const mod = mods.find((item) => item.id === modId)
      const resolved = mod ? resolveModForBuild(mod, data.gameBuild) : findModForServerId(mods, modId, data.gameBuild)
      return resolved ? [resolved.id] : []
    })
    const workshopIds = getWorkshopIdsForModIds(resolvedModIds, mods, data.gameBuild)
    const createdServer = await invokeTauri<ZomboidServer>(
      isRemoteWorkspace && remoteConnection ? "create_remote_zomboid_server" : "create_zomboid_server",
      {
        ...(isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : {}),
        name: data.name,
        modIds: resolvedModIds,
        workshopIds,
        gameBuild: data.gameBuild,
        maxPlayers: data.maxPlayers,
      },
    )

    updateServers((currentServers) =>
      [...currentServers.filter((server) => server.id !== createdServer.id), createdServer].sort((left, right) =>
        left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
      ),
    )
    setSelectedServer(createdServer)
    setServerConfigTarget(createdServer)
    setActiveTab("dashboard")
  }

  async function updateServerSettings(settings: ServerIniSettings) {
    if (!serverConfigTarget) return

    const updatedServer = await invokeTauri<ZomboidServer>(isRemoteWorkspace && remoteConnection ? "update_remote_zomboid_server_settings" : "update_zomboid_server_settings", {
      ...(isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : {}),
      serverId: serverConfigTarget.id,
      settings,
    })

    updateServers((currentServers) =>
      currentServers.map((server) => server.id === updatedServer.id ? updatedServer : server).sort((left, right) =>
        left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
      ),
    )
    setSelectedServer((currentServer) => currentServer?.id === updatedServer.id ? updatedServer : currentServer)
    setServerConfigTarget(updatedServer)
  }

  async function installDownloadedDependencyForServer(server: ZomboidServer, dependencyId: string) {
    const refreshedMods = await loadMods()
    const normalizedDependencyId = dependencyId.trim().toLowerCase()
    const dependency = findModForServerId(refreshedMods, normalizedDependencyId, server.gameBuild)

    if (!dependency) {
      throw new Error(t("dependency.downloadedMissing", { id: dependencyId }))
    }

    if (!dependency.isInstalled) {
      await installMods([dependency])
    }

    await activateServerMods(server, [
      {
        ...dependency,
        isInstalled: true,
        source: dependency.source === "steam" ? "local" : dependency.source,
      },
    ])
  }

  async function changeServerBuild(server: ZomboidServer, gameBuild: "b41" | "b42") {
    await invokeTauri<void>(isRemoteWorkspace && remoteConnection ? "update_remote_zomboid_server_build" : "update_zomboid_server_build", {
      ...(isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : {}),
      serverId: server.id,
      gameBuild,
    })
    const updatedServer = { ...server, gameBuild }
    setSelectedServer(updatedServer)
    updateServers((currentServers) => currentServers.map((item) => item.id === server.id ? updatedServer : item))
  }

  async function deleteServer(server: ZomboidServer) {
    try {
      const result = await invokeTauri<DeleteServerResult>("delete_zomboid_server", {
        serverId: server.id,
      })

      updateServers((currentServers) => currentServers.filter((item) => item.id !== server.id))
      setSelectedServer((current) => current?.id === server.id ? null : current)
      await loadServers()
      addNotification({
        title: t("dashboard.deleteSuccessTitle"),
        message: t("dashboard.deleteSuccessBody", { name: server.name, backupPath: result.backupPath }),
        tone: "success",
      })
    } catch (error) {
      const message = getErrorMessage(error)
      setServersError(message)
      addNotification({
        title: t("dashboard.deleteErrorTitle"),
        message: t("dashboard.deleteErrorBody", { name: server.name, error: message }),
        tone: "error",
      })
    }
  }

  async function scanData() {
    if (isRemoteWorkspace) {
      await loadServers()
      void loadModsInBackground()
      return
    }

    await loadServers()

    void loadModsInBackground()
  }

  async function rescanModsFromScratch() {
    if (isRemoteWorkspace) {
      clearModsLibraryCache(modsCacheKey)
      if (remoteConnection) {
        await invokeTauri<void>("clear_remote_zomboid_mods_cache", { connection: remoteConnection })
      }
      await loadMods()
      return
    }

    clearModsLibraryCache(modsCacheKey)
    await invokeTauri<void>("clear_zomboid_mods_cache")
    await loadMods()
  }

  async function loadInitialData() {
    if (isRemoteWorkspace) {
      await loadServers()
      void loadModsInBackground()
      return
    }

    await loadServers()
    void loadModsInBackground()
  }

  function addNotification(notification: Omit<AppNotification, "id" | "createdAt" | "isRead">) {
    setNotifications((currentNotifications) => [
      {
        ...notification,
        id: `${Date.now()}:${Math.random().toString(16).slice(2)}`,
        createdAt: new Date().toISOString(),
        isRead: false,
      },
      ...currentNotifications,
    ].slice(0, 30))
  }

  function handleNotificationClick(notification: AppNotification) {
    setNotifications((currentNotifications) =>
      currentNotifications.map((currentNotification) =>
        currentNotification.id === notification.id ? { ...currentNotification, isRead: true } : currentNotification,
      ),
    )

    if (notification.action?.type === "server-test") {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: notification.action.serverId } }))
    }

    if (notification.action?.type === "download-result") {
      setSelectedServer(null)
      setActiveTab("download")
      downloadManager.openResultDetails(notification.action.result)
    }
  }

  function markAllNotificationsRead() {
    setNotifications((currentNotifications) =>
      currentNotifications.map((notification) => ({ ...notification, isRead: true })),
    )
  }

  useEffect(() => {
    void loadInitialData()
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | null = null

    void listen<ServerTestEvent>("server-test-event", (event) => {
      const payload = event.payload

      if (payload.event === "started") {
        setRunningServerTestId(payload.serverId)
        return
      }

      if (payload.event === "finished" || payload.event === "error") {
        setRunningServerTestId((currentServerId) =>
          currentServerId === payload.serverId ? null : currentServerId,
        )
      }
    }).then((unsubscribe) => {
      unlisten = unsubscribe
    })

    return () => {
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | null = null

    void listen<string>("native-menu", (event) => {
      switch (event.payload) {
        case "new_server":
          setIsCreateServerModalOpen(true)
          break
        case "show_dashboard":
          setSelectedServer(null)
          setActiveTab("dashboard")
          break
        case "show_mods":
          setSelectedServer(null)
          setActiveTab("mods")
          void ensureModsLoaded()
          break
        case "show_downloads":
          if (isRemoteWorkspace) {
            setActiveTab("dashboard")
            break
          }
          setSelectedServer(null)
          setActiveTab("download")
          break
        case "show_settings":
          setSelectedServer(null)
          setActiveTab("settings")
          break
        case "scan_mods":
          void scanData()
          break
        case "bring_steam_mods":
          if (!isRemoteWorkspace) {
            void installAllUninstalledMods()
          }
          break
        case "reload":
          window.location.reload()
          break
      }
    }).then((unsubscribe) => {
      unlisten = unsubscribe
    })

    return () => {
      unlisten?.()
    }
  })
 
  return (
    <main className="flex h-screen w-screen overflow-hidden bg-[#22272b] text-white">
      <AppSidebar
        activeTab={activeTab}
        items={navItems}
        onChangeWorkspace={onChangeWorkspace}
        onTabChange={(tabId) => {
          setActiveTab(tabId)
          setSelectedServer(null)
          if (!isRemoteWorkspace && tabId === "mods") {
            void ensureModsLoaded()
          }
        }}
      />

      <div className="flex-1 flex flex-col min-w-0">
        <AppHeader
          onScanMods={scanData}
          onInstallAllMods={isRemoteWorkspace ? undefined : installAllUninstalledMods}
          isInstallingAllMods={isInstallingAllMods}
          isRemoteWorkspace={isRemoteWorkspace}
          onConfigureRemoteSteamCmd={() => setIsRemoteSteamCmdModalOpen(true)}
          onOpenRemoteTerminal={() => setIsRemoteTerminalModalOpen(true)}
          showSearch={!(activeTab === "dashboard" && selectedServer)}
          onOpenSettings={() => setActiveTab("settings")}
          notifications={notifications}
          onNotificationClick={handleNotificationClick}
          onMarkAllNotificationsRead={markAllNotificationsRead}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
        />

        <div className="flex-1 overflow-hidden relative">
          {activeTab === "dashboard" && (
            selectedServer ? (
              !hasLoadedMods ? (
                <LoadingModsPanel error={modsError} isLoading={isLoadingMods} onRetry={ensureModsLoaded} />
              ) : (
                <ServerDetail
                  server={selectedServer}
                  allMods={mods ?? []}
                  onBack={() => setSelectedServer(null)}
                  onInstallMods={installMods}
                  onActivateMods={(modsToActivate) => activateServerMods(selectedServer, modsToActivate)}
                  onToggleMod={(mod, action) => toggleServerMod(selectedServer, mod, action)}
                  onMoveActiveMod={(mod, position) => moveServerMod(selectedServer, mod, position)}
                  onRefreshMods={async () => {
                    await loadMods()
                  }}
                  onDependencyDownloaded={(dependencyId) => installDownloadedDependencyForServer(selectedServer, dependencyId)}
                  onOpenSettings={() => setActiveTab("settings")}
                  runningServerTestId={runningServerTestId}
                  onChangeBuild={(gameBuild) => changeServerBuild(selectedServer, gameBuild)}
                  onConfigureServer={setServerConfigTarget}
                  remoteConnection={remoteConnection}
                />
              )
            ) : (
              <Dashboard
                servers={servers}
                isLoading={isLoadingServers}
                error={serversError}
                onRefresh={loadServers}
                onCreateServer={() => {
                  setIsCreateServerModalOpen(true)
                  void ensureModsLoaded()
                }}
                searchQuery={searchQuery}
                onDeleteServer={deleteServer}
                onConfigureServer={setServerConfigTarget}
                isReadOnly={isRemoteWorkspace}
                canCreateServer
                onServerClick={(server) => {
                  setSelectedServer(server)
                  void ensureModsLoaded()
                }}
              />
            )
          )}
          {activeTab === "mods" && (
            <ModsList
              mods={mods}
              isLoading={isLoadingMods}
              error={modsError}
              onRefresh={loadMods}
              onInstall={installMods}
              onInstallAll={installAllUninstalledMods}
              isInstallingAll={isInstallingAllMods}
              onOpenSettings={() => setActiveTab("settings")}
              searchQuery={searchQuery}
              onSearchChange={setSearchQuery}
              isReadOnly={isRemoteWorkspace}
            />
          )}
          {activeTab === "download" && (
            <DownloadMods
              manager={downloadManager}
              remoteConnection={remoteConnection}
              onOpenSettings={() => setActiveTab("settings")}
            />
          )}
          {activeTab === "settings" && (
            <SettingsView onRescanMods={rescanModsFromScratch} remoteConnection={remoteConnection} />
          )}
        </div>
      </div>

      <CreateServerModal
        isOpen={isCreateServerModalOpen}
        onClose={() => setIsCreateServerModalOpen(false)}
        existingServers={servers}
        availableMods={mods}
        onCreate={createServer}
      />

      <ServerConfigurationModal
        isOpen={serverConfigTarget !== null}
        server={serverConfigTarget}
        remoteConnection={remoteConnection}
        onClose={() => setServerConfigTarget(null)}
        onSave={updateServerSettings}
      />

      {remoteConnection && (
        <>
          <RemoteSteamCmdModal
            connection={remoteConnection}
            isOpen={isRemoteSteamCmdModalOpen}
            onClose={() => setIsRemoteSteamCmdModalOpen(false)}
          />
          <RemoteTerminalModal
            connection={remoteConnection}
            isOpen={isRemoteTerminalModalOpen}
            onClose={() => setIsRemoteTerminalModalOpen(false)}
          />
        </>
      )}

      <ServerTestPanel
        hasDownloadProgressCard={downloadManager.isDownloading && activeTab !== "download"}
        onNotification={addNotification}
      />

      {downloadManager.isDownloading && activeTab !== "download" && (
        <DownloadProgressCard
          manager={downloadManager}
          onOpen={() => {
            setSelectedServer(null)
            setActiveTab("download")
          }}
        />
      )}
    </main>
  );
}

export default App
