import { useEffect, useMemo, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { Box, Download, Server, Settings } from "lucide-react"

import { AppHeader, type AppNotification } from "@/components/AppHeader"
import { AppSidebar } from "@/components/AppSidebar"
import { CreateServerModal } from "@/components/CreateServerModal"
import { Dashboard } from "@/components/Dashboard"
import { DownloadMods } from "@/components/DownloadMods"
import { DownloadProgressCard } from "@/components/DownloadProgressCard"
import { LoadingModsPanel } from "@/components/LoadingModsPanel"
import { ModsList } from "@/components/ModsList"
import { ServerDetail } from "@/components/ServerDetail"
import { ServerTestPanel } from "@/components/ServerTestPanel"
import { Settings as SettingsView } from "@/components/Settings"
import { WorkshopWindow } from "@/components/WorkshopWindow"
import { useModsLibrary } from "@/hooks/useModsLibrary"
import { useWorkshopDownloadManager } from "@/hooks/useWorkshopDownloadManager"
import { getErrorMessage } from "@/lib/errors"
import { getActiveDependencyChain, getWorkshopIdsForModIds } from "@/lib/serverMods"
import { invokeTauri } from "@/lib/tauri"
import type { ZomboidMod } from "@/types/mod"
import type { ZomboidServer } from "@/types/server"

type ServerTestEvent = {
  serverId: string
  event: "started" | "line" | "finished" | "error"
}

function App() {
  if (window.location.hash.startsWith("#/workshop")) {
    return <WorkshopWindow />
  }

  const [isCreateServerModalOpen, setIsCreateServerModalOpen] = useState(false)
  const [activeTab, setActiveTab] = useState("dashboard")
  const [selectedServer, setSelectedServer] = useState<ZomboidServer | null>(null)
  const [servers, setServers] = useState<ZomboidServer[]>([])
  const [serversError, setServersError] = useState<string | null>(null)
  const [isLoadingServers, setIsLoadingServers] = useState(true)
  const [searchQuery, setSearchQuery] = useState("")
  const [notifications, setNotifications] = useState<AppNotification[]>([])
  const [runningServerTestId, setRunningServerTestId] = useState<string | null>(null)
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
  } = useModsLibrary()
  const navItems = useMemo(
    () => [
      { id: "dashboard", label: "Servidores", icon: Server },
      { id: "mods", label: "Mods", icon: Box, badge: String(modsCount) },
      { id: "download", label: "Baixar", icon: Download },
      { id: "settings", label: "Settings", icon: Settings },
    ],
    [modsCount],
  )
  const downloadManager = useWorkshopDownloadManager({
    isDownloadScreenActive: activeTab === "download",
    onDownloadFinished: loadMods,
    onNotification: addNotification,
  })

  async function loadServers() {
    setIsLoadingServers(true)
    setServersError(null)

    try {
      const foundServers = await invokeTauri<ZomboidServer[]>("list_zomboid_servers")
      setServers(foundServers)
      setSelectedServer((current) =>
        current ? foundServers.find((server) => server.id === current.id) ?? null : null,
      )
    } catch (error) {
      const message = getErrorMessage(error)
      setServersError(message)
      setServers([])
    } finally {
      setIsLoadingServers(false)
    }
  }

  async function updateServerMods(server: ZomboidServer, activeModIds: string[]) {
    setServersError(null)
    const workshopIds = getWorkshopIdsForModIds(activeModIds, mods)

    await invokeTauri<void>("update_zomboid_server_mods", {
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
    setServers((currentServers) =>
      currentServers.map((currentServer) => (currentServer.id === server.id ? updatedServer : currentServer)),
    )
  }

  async function toggleServerMod(server: ZomboidServer, mod: ZomboidMod, action: "activate" | "deactivate") {
    const activeModIds = server.activeModIds ?? []
    const normalizedModId = mod.id.toLowerCase()
    const nextActiveModIds =
      action === "activate"
        ? activeModIds.some((modId) => modId.toLowerCase() === normalizedModId)
          ? activeModIds
          : [...activeModIds, mod.id]
        : activeModIds.filter((modId) => modId.toLowerCase() !== normalizedModId)

    try {
      await updateServerMods(server, nextActiveModIds)
    } catch (error) {
      setServersError(getErrorMessage(error))
    }
  }

  async function moveServerMod(server: ZomboidServer, mod: ZomboidMod, position: "start" | "end") {
    const normalizedModId = mod.id.toLowerCase()
    const activeModIds = server.activeModIds ?? []
    const activeModIdKeys = new Set(activeModIds.map((modId) => modId.toLowerCase()))
    const modsById = new Map(mods.filter((item) => item.id).map((item) => [item.id.toLowerCase(), item]))
    const moveModIds =
      position === "start"
        ? getActiveDependencyChain(mod, modsById, activeModIdKeys)
        : [mod.id]
    const moveModIdKeys = new Set(moveModIds.map((modId) => modId.toLowerCase()))
    const remainingModIds = activeModIds.filter((modId) => !moveModIdKeys.has(modId.toLowerCase()))

    if (!activeModIdKeys.has(normalizedModId)) {
      return
    }

    const nextActiveModIds = position === "start" ? [...moveModIds, ...remainingModIds] : [...remainingModIds, mod.id]

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
      const normalizedModId = mod.id.toLowerCase()

      if (!activeModIdsSet.has(normalizedModId)) {
        nextActiveModIds.push(mod.id)
        activeModIdsSet.add(normalizedModId)
      }
    }

    try {
      await updateServerMods(server, nextActiveModIds)
    } catch (error) {
      setServersError(getErrorMessage(error))
    }
  }

  async function createServer(data: { name: string; modIds: string[] }) {
    const workshopIds = getWorkshopIdsForModIds(data.modIds, mods)
    const createdServer = await invokeTauri<ZomboidServer>("create_zomboid_server", {
      name: data.name,
      modIds: data.modIds,
      workshopIds,
    })

    setServers((currentServers) =>
      [...currentServers.filter((server) => server.id !== createdServer.id), createdServer].sort((left, right) =>
        left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
      ),
    )
    setSelectedServer(createdServer)
    setActiveTab("dashboard")
  }

  async function installDownloadedDependencyForServer(server: ZomboidServer, dependencyId: string) {
    const refreshedMods = await loadMods()
    const normalizedDependencyId = dependencyId.trim().toLowerCase()
    const dependency = refreshedMods.find((mod) => mod.id.toLowerCase() === normalizedDependencyId)

    if (!dependency) {
      throw new Error(`Dependencia '${dependencyId}' baixada, mas ainda nao apareceu na biblioteca. Atualize os mods e tente novamente.`)
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

  async function scanData() {
    await loadServers()

    void loadModsInBackground()
  }

  async function loadInitialData() {
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
          void installAllUninstalledMods()
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
        onTabChange={(tabId) => {
          setActiveTab(tabId)
          setSelectedServer(null)
          if (tabId === "mods") {
            void ensureModsLoaded()
          }
        }}
      />

      <div className="flex-1 flex flex-col min-w-0">
        <AppHeader
          onScanMods={scanData}
          onInstallAllMods={installAllUninstalledMods}
          isInstallingAllMods={isInstallingAllMods}
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
                  onRefreshMods={loadMods}
                  onDependencyDownloaded={(dependencyId) => installDownloadedDependencyForServer(selectedServer, dependencyId)}
                  onOpenSettings={() => setActiveTab("settings")}
                  runningServerTestId={runningServerTestId}
                />
              )
            ) : (
              <Dashboard
                servers={servers}
                isLoading={isLoadingServers}
                error={serversError}
                onRefresh={loadServers}
                onCreateServer={() => setIsCreateServerModalOpen(true)}
                searchQuery={searchQuery}
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
            />
          )}
          {activeTab === "download" && (
            <DownloadMods
              manager={downloadManager}
              onOpenSettings={() => setActiveTab("settings")}
            />
          )}
          {activeTab === "settings" && (
            <SettingsView />
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
