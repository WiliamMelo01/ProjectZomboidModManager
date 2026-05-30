import { useEffect, useMemo, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { Box, Download, RefreshCw, Server, Settings } from "lucide-react"

import { AppHeader, type AppNotification } from "@/components/AppHeader"
import { AppSidebar } from "@/components/AppSidebar"
import { CreateServerModal } from "@/components/CreateServerModal"
import { Dashboard } from "@/components/Dashboard"
import { DownloadMods } from "@/components/DownloadMods"
import { DownloadProgressCard } from "@/components/DownloadProgressCard"
import { ModsList } from "@/components/ModsList"
import { ServerDetail } from "@/components/ServerDetail"
import { ServerTestPanel } from "@/components/ServerTestPanel"
import { Settings as SettingsView } from "@/components/Settings"
import { WorkshopWindow } from "@/components/WorkshopWindow"
import { useWorkshopDownloadManager } from "@/hooks/useWorkshopDownloadManager"
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
  const [mods, setMods] = useState<ZomboidMod[]>([])
  const [modsCount, setModsCount] = useState(0)
  const [modsError, setModsError] = useState<string | null>(null)
  const [isLoadingMods, setIsLoadingMods] = useState(false)
  const [isInstallingAllMods, setIsInstallingAllMods] = useState(false)
  const [hasLoadedMods, setHasLoadedMods] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [notifications, setNotifications] = useState<AppNotification[]>([])
  const [runningServerTestId, setRunningServerTestId] = useState<string | null>(null)
  const modsLoadPromiseRef = useRef<Promise<ZomboidMod[]> | null>(null)
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

  async function loadMods() {
    if (modsLoadPromiseRef.current) {
      return modsLoadPromiseRef.current
    }

    const loadPromise = (async () => {
      setIsLoadingMods(true)
      setModsError(null)

      try {
        const foundMods = await invokeTauri<ZomboidMod[]>("list_zomboid_mods")
        setMods(foundMods)
        setModsCount(foundMods.length)
        setHasLoadedMods(true)
        return foundMods
      } catch (error) {
        const message = getErrorMessage(error)
        setModsError(message)
        setMods([])
        return []
      } finally {
        setIsLoadingMods(false)
        modsLoadPromiseRef.current = null
      }
    })()

    modsLoadPromiseRef.current = loadPromise
    return loadPromise
  }

  async function loadModsCount() {
    setModsError(null)

    try {
      const foundModsCount = await invokeTauri<number>("count_zomboid_mods")
      setModsCount(foundModsCount)
    } catch (error) {
      setModsError(getErrorMessage(error))
      setModsCount(0)
    }
  }

  async function ensureModsLoaded() {
    if (hasLoadedMods || isLoadingMods) {
      return
    }

    await loadMods()
  }

  async function installMods(modsToInstall: ZomboidMod[]) {
    setModsError(null)

    try {
      const modsToMove = modsToInstall.filter((mod) => !mod.isInstalled && mod.source !== "local")

      for (const mod of modsToMove) {
        await invokeTauri<void>("install_zomboid_mod", {
          modPath: mod.path,
          modId: mod.id,
          workshopId: mod.workshopId,
        })
      }
      const installedModIds = new Set(modsToMove.map((mod) => mod.id.toLowerCase()))

      setMods((currentMods) =>
        currentMods.map((mod) =>
          installedModIds.has(mod.id.toLowerCase())
            ? {
                ...mod,
                isInstalled: true,
                source: mod.source === "steam" ? "local" : mod.source,
              }
            : mod,
        ),
      )
    } catch (error) {
      setModsError(getErrorMessage(error))
      throw error
    }
  }

  async function installAllUninstalledMods() {
    if (isInstallingAllMods) {
      return
    }

    setIsInstallingAllMods(true)

    try {
      const availableMods = hasLoadedMods ? mods : await loadMods()
      const modsToInstall = availableMods.filter((mod) => !mod.isInstalled && mod.source !== "local")

      if (modsToInstall.length === 0) {
        return
      }

      await installMods(modsToInstall)
    } finally {
      setIsInstallingAllMods(false)
    }
  }

  async function updateServerMods(server: ZomboidServer, activeModIds: string[]) {
    setServersError(null)
    const workshopIds = getWorkshopIdsForModIds(activeModIds)

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

  function getActiveDependencyChain(
    mod: ZomboidMod,
    modsById: Map<string, ZomboidMod>,
    activeModIds: Set<string>,
  ) {
    const orderedModIds: string[] = []
    const visitingModIds = new Set<string>()
    const visitedModIds = new Set<string>()

    function visit(currentMod: ZomboidMod) {
      const currentModId = currentMod.id.toLowerCase()

      if (visitedModIds.has(currentModId) || visitingModIds.has(currentModId)) {
        return
      }

      visitingModIds.add(currentModId)

      for (const dependencyId of currentMod.dependencies ?? []) {
        const normalizedDependencyId = dependencyId.toLowerCase()

        if (!activeModIds.has(normalizedDependencyId)) {
          continue
        }

        const dependency = modsById.get(normalizedDependencyId)

        if (dependency) {
          visit(dependency)
        }
      }

      visitingModIds.delete(currentModId)
      visitedModIds.add(currentModId)

      if (activeModIds.has(currentModId)) {
        orderedModIds.push(currentMod.id)
      }
    }

    visit(mod)
    return orderedModIds
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
    const selectedModIds = new Set(data.modIds.map((modId) => modId.toLowerCase()))
    const workshopIds = getWorkshopIdsForModIds(data.modIds)
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

  function getWorkshopIdsForModIds(modIds: string[]) {
    const selectedModIds = new Set(modIds.map((modId) => modId.toLowerCase()))
    const seenWorkshopIds = new Set<string>()

    return modIds.flatMap((modId) => {
      const mod = mods.find((item) => item.id.toLowerCase() === modId.toLowerCase())
      const workshopId = mod?.workshopId?.trim()

      if (!workshopId) {
        return []
      }

      const normalizedWorkshopId = workshopId.toLowerCase()

      if (seenWorkshopIds.has(normalizedWorkshopId)) {
        return []
      }

      if (!selectedModIds.has(modId.toLowerCase())) {
        return []
      }

      seenWorkshopIds.add(normalizedWorkshopId)
      return [workshopId]
    })
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

  async function loadModsInBackground() {
    setIsLoadingMods(true)
    await loadModsCount()
    await loadMods()
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

  return "Nao foi possivel buscar os servidores."
}

function LoadingModsPanel({
  error,
  isLoading,
  onRetry,
}: {
  error: string | null
  isLoading: boolean
  onRetry: () => Promise<void>
}) {
  return (
    <div className="h-full bg-[#22272b] p-8 text-white">
      <div className="rounded-3xl border border-white/5 bg-[#2b3238] p-6 text-gray-300">
        <div className="flex items-center gap-3">
          <RefreshCw size={20} className={isLoading ? "animate-spin text-orange-400" : "text-gray-500"} />
          <div>
            <p className="font-bold text-white">Carregando mods</p>
            <p className="text-sm text-gray-400">A lista completa so e carregada quando ela for necessaria.</p>
          </div>
        </div>

        {error && (
          <div className="mt-5 rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
            {error}
          </div>
        )}

        {error && (
          <button
            onClick={() => void onRetry()}
            className="mt-5 rounded-xl bg-orange-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-orange-600"
          >
            Tentar novamente
          </button>
        )}
      </div>
    </div>
  )
}

export default App
