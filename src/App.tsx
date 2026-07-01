import { useEffect, useMemo, useRef, useState } from "react"
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
import {
  RemoteServerStartModal,
  type RemoteServerActionResult,
  type RemoteServerFirewallCheck,
} from "@/components/server/RemoteServerStartModal"
import { ServerPortConflictModal } from "@/components/server/ServerPortConflictModal"
import type { ServerPortCheck } from "@/components/server/ServerPortConflictModal"
import { DeployLocalServerModal } from "@/components/server/DeployLocalServerModal"
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
  const [isDeployLocalModalOpen, setIsDeployLocalModalOpen] = useState(false)
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
  const [isTestingServer, setIsTestingServer] = useState(false)
  const [portConflictCheck, setPortConflictCheck] = useState<ServerPortCheck | null>(null)
  const [isCheckingPorts, setIsCheckingPorts] = useState(false)
  const [isKillingPorts, setIsKillingPorts] = useState(false)
  const [isRemoteStartOpen, setIsRemoteStartOpen] = useState(false)
  const [remoteFirewallCheck, setRemoteFirewallCheck] = useState<RemoteServerFirewallCheck | null>(null)
  const [remoteStartResult, setRemoteStartResult] = useState<RemoteServerActionResult | null>(null)
  const [remoteStartLogs, setRemoteStartLogs] = useState<string[]>([])
  const [remoteStartError, setRemoteStartError] = useState<string | null>(null)
  const [isCheckingRemoteFirewall, setIsCheckingRemoteFirewall] = useState(false)
  const [isConfiguringRemoteFirewall, setIsConfiguringRemoteFirewall] = useState(false)
  const [isStartingRemoteServer, setIsStartingRemoteServer] = useState(false)
  const [activeStartServer, setActiveStartServer] = useState<ZomboidServer | null>(null)
  const activeStartServerRef = useRef<ZomboidServer | null>(null)
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
    backgroundReloadAfterInstall: isRemoteWorkspace,
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
    window.dispatchEvent(new CustomEvent("pzmm-reveal-server", { detail: { serverId: createdServer.id } }))
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
      const result = await invokeTauri<DeleteServerResult>(
        isRemoteWorkspace && remoteConnection ? "delete_remote_zomboid_server" : "delete_zomboid_server",
        {
          ...(isRemoteWorkspace && remoteConnection ? { connection: remoteConnection } : {}),
          serverId: server.id,
        }
      )

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
        await invokeTauri<void>("clear_remote_zomboid_mods_and_images_cache", { connection: remoteConnection })
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

  async function testServer(server: ZomboidServer, skipPortCheck = false) {
    const isCurrentServerTesting = isTestingServer || runningServerTestId === server.id
    if (isCurrentServerTesting) {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id } }))
      return
    }

    setActiveStartServer(server)

    if (!remoteConnection && !skipPortCheck) {
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
      await invokeTauri(remoteConnection ? "start_remote_zomboid_server_test" : "start_zomboid_server_test", {
        ...(remoteConnection ? { connection: remoteConnection } : {}),
        serverId: server.id,
      })
    } catch (error) {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id, error: getErrorMessage(error) } }))
    } finally {
      setIsTestingServer(false)
    }
  }

  async function cancelServerTest(serverId: string) {
    try {
      await invokeTauri<void>(remoteConnection ? "cancel_remote_zomboid_server_test" : "cancel_zomboid_server_test", {
        ...(remoteConnection ? { connection: remoteConnection } : {}),
        serverId,
      })
    } catch (error) {
      addNotification({
        title: t("serverTest.failedTitle"),
        message: getErrorMessage(error),
        tone: "error",
      })
    } finally {
      setRunningServerTestId((currentServerId) => currentServerId === serverId ? null : currentServerId)
      setIsTestingServer(false)
    }
  }
  async function checkRemoteServerFirewall(server: ZomboidServer) {
    if (!remoteConnection || !server) {
      return
    }

    setIsRemoteStartOpen(true)
    setRemoteStartResult(null)
    setRemoteStartError(null)
    setRemoteStartLogs([`Checking remote systemd setup for ${server.name}...`])
    setIsCheckingRemoteFirewall(true)

    try {
      const check = await invokeTauri<RemoteServerFirewallCheck>("check_remote_zomboid_server_firewall", {
        connection: remoteConnection,
        serverId: server.id,
      })
      setRemoteFirewallCheck(check)
      setRemoteStartLogs([
        ...check.logs,
        check.isConfigured ? "systemd is configured. You can run the server." : "systemd needs configuration before running the server.",
      ])
    } catch (error) {
      setRemoteStartError(getErrorMessage(error))
      setRemoteStartLogs((currentLogs) => [...currentLogs, getErrorMessage(error)])
    } finally {
      setIsCheckingRemoteFirewall(false)
    }
  }

  function appendRemoteStartLogs(nextLogs: string[]) {
    if (nextLogs.length === 0) return

    setRemoteStartLogs((currentLogs) => {
      const seen = new Set(currentLogs)
      const merged = [...currentLogs]

      for (const line of nextLogs) {
        if (!seen.has(line)) {
          seen.add(line)
          merged.push(line)
        }
      }

      return merged
    })
  }

  function isRemoteStartupPending(result: RemoteServerActionResult | null) {
    if (!result?.success) return false

    return /not detected|keeps starting|still starting|not open yet|finalizing startup status/i.test(result.message)
  }
  async function configureRemoteServerFirewall(server: ZomboidServer) {
    if (!remoteConnection || !server) {
      return
    }

    setRemoteStartError(null)
    setIsConfiguringRemoteFirewall(true)
    setRemoteStartLogs((currentLogs) => [...currentLogs, "Configuring remote systemd service/socket units..."])

    try {
      const result = await invokeTauri<RemoteServerActionResult>("configure_remote_zomboid_server_firewall", {
        connection: remoteConnection,
        serverId: server.id,
      })
      appendRemoteStartLogs([...result.logs, result.message])

      const check = await invokeTauri<RemoteServerFirewallCheck>("check_remote_zomboid_server_firewall", {
        connection: remoteConnection,
        serverId: server.id,
      })
      setRemoteFirewallCheck(check)
      setRemoteStartLogs((currentLogs) => [
        ...currentLogs,
        ...check.logs,
        check.isConfigured ? "systemd is configured. You can run the server." : "systemd still needs attention.",
      ])
    } catch (error) {
      setRemoteStartError(getErrorMessage(error))
      setRemoteStartLogs((currentLogs) => [...currentLogs, getErrorMessage(error)])
    } finally {
      setIsConfiguringRemoteFirewall(false)
    }
  }

  async function startRemoteServer(server: ZomboidServer) {
    if (!remoteConnection || !server) {
      return
    }

    setRemoteStartError(null)
    setIsStartingRemoteServer(true)
    appendRemoteStartLogs(["Starting remote Project Zomboid server...", "Streaming startup output in real time."])

    try {
      const result = await invokeTauri<RemoteServerActionResult>("start_remote_zomboid_server", {
        connection: remoteConnection,
        serverId: server.id,
      })
      setRemoteStartResult(result)
      appendRemoteStartLogs([...result.logs, result.message])
    } catch (error) {
      setIsStartingRemoteServer(false)
      setRemoteStartError(getErrorMessage(error))
      setRemoteStartLogs((currentLogs) => [...currentLogs, getErrorMessage(error)])
    }
  }

  async function sendRemoteServerCommand(server: ZomboidServer, command: string) {
    if (!remoteConnection || !server) {
      return
    }

    setRemoteStartError(null)
    appendRemoteStartLogs([`> ${command}`])

    try {
      const result = await invokeTauri<RemoteServerActionResult>("send_remote_zomboid_server_command", {
        connection: remoteConnection,
        serverId: server.id,
        command,
      })
      appendRemoteStartLogs([...result.logs, result.message])
    } catch (error) {
      setRemoteStartError(getErrorMessage(error))
      setRemoteStartLogs((currentLogs) => [...currentLogs, getErrorMessage(error)])
      throw error
    }
  }

  async function stopRemoteServer(server: ZomboidServer) {
    await sendRemoteServerCommand(server, "quit")
    setRemoteStartResult({
      success: false,
      message: "Stop command queued. Refreshing server status shortly.",
      command: "quit",
      logs: [],
    })
    setRemoteStartLogs((currentLogs) => [...currentLogs, "Stop command queued. The server may take a few seconds to shut down."])
    window.setTimeout(() => {
      void loadServers()
    }, 5000)
  }

  async function openRemoteServerStart(server: ZomboidServer) {
    setActiveStartServer(server)
    setIsRemoteStartOpen(true)
    await checkRemoteServerFirewall(server)
  }

  function openRemoteServerConsole(server: ZomboidServer) {
    setActiveStartServer(server)
    setIsRemoteStartOpen(true)
    setRemoteFirewallCheck(null)
    setRemoteStartError(null)
    setRemoteStartResult({
      success: true,
      message: "Remote command channel opened. Commands will be queued for the server controller if it is running.",
      command: "send-server-command",
      logs: [],
    })
    setRemoteStartLogs([
      `Remote command console opened for ${server.name}.`,
      "Use this only after the remote server controller is running.",
    ])
  }

  async function killPortConflictsAndContinue(server: ZomboidServer) {
    if (!portConflictCheck) {
      return
    }

    setIsKillingPorts(true)

    try {
      await invokeTauri("kill_processes_by_pid", {
        pids: Array.from(new Set(portConflictCheck.usages.map((usage) => usage.pid))),
      })
      setPortConflictCheck(null)
      await testServer(server, true)
    } catch (error) {
      window.dispatchEvent(new CustomEvent("pzmm-open-server-test-panel", { detail: { serverId: server.id, error: getErrorMessage(error) } }))
    } finally {
      setIsKillingPorts(false)
    }
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
    activeStartServerRef.current = activeStartServer
  }, [activeStartServer])
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

    void listen<ServerTestEvent>("remote-server-start-event", (event) => {
      const payload = event.payload
      const currentStartServer = activeStartServerRef.current
      if (!currentStartServer || payload.serverId !== currentStartServer.id) {
        return
      }

      if (payload.event === "started") {
        setIsStartingRemoteServer(true)
        appendRemoteStartLogs(["Remote server process started. Streaming logs..."])
        return
      }

      if (payload.event === "line" && payload.line) {
        appendRemoteStartLogs([payload.line])
        return
      }

      if (payload.event === "ready") {
        setIsStartingRemoteServer(false)
        setRemoteStartError(null)
        setRemoteStartResult({
          success: true,
          message: "Remote server is running. Console command channel is ready.",
          command: "start-server-streaming",
          logs: [],
        })
        window.setTimeout(() => {
          void loadServers()
        }, 1500)
        return
      }

      if (payload.event === "finished") {
        setIsStartingRemoteServer(false)
        setRemoteStartResult({
          success: false,
          message: "Remote server process exited.",
          command: "start-server-streaming",
          logs: [],
        })
        appendRemoteStartLogs(["Remote server process exited."])
        window.setTimeout(() => {
          void loadServers()
        }, 1500)
        return
      }

      if (payload.event === "error") {
        setIsStartingRemoteServer(false)
        const message = payload.error ?? "Remote server start failed."
        setRemoteStartError(message)
        setRemoteStartResult({
          success: false,
          message,
          command: "start-server-streaming",
          logs: [],
        })
        appendRemoteStartLogs([message])
      }
    }).then((unsubscribe) => {
      unlisten = unsubscribe
    })

    return () => {
      unlisten?.()
    }
  }, [])
  useEffect(() => {
    if (!isRemoteStartOpen || !remoteConnection || !activeStartServer || !isRemoteStartupPending(remoteStartResult)) {
      return
    }

    let cancelled = false
    let timer: number | undefined

    async function pollRemoteServerStatus() {
      if (!remoteConnection || !activeStartServer) return

      try {
        const result = await invokeTauri<RemoteServerActionResult>("check_remote_zomboid_server_status", {
          connection: remoteConnection,
          serverId: activeStartServer.id,
        })

        if (cancelled) return

        setRemoteStartResult(result)
        appendRemoteStartLogs([...result.logs, result.message])

        if (result.success && /appears to be running|is running on port/i.test(result.message)) {
          window.setTimeout(() => {
            void loadServers()
          }, 1500)
          return
        }

        if (!result.success) {
          return
        }
      } catch (error) {
        if (cancelled) return
        appendRemoteStartLogs([getErrorMessage(error)])
      }

      if (!cancelled) {
        timer = window.setTimeout(pollRemoteServerStatus, 5000)
      }
    }

    timer = window.setTimeout(pollRemoteServerStatus, 3000)

    return () => {
      cancelled = true
      if (timer) {
        window.clearTimeout(timer)
      }
    }
  }, [isRemoteStartOpen, remoteConnection, activeStartServer, remoteStartResult?.message, remoteStartResult?.success])
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
                  isTestingServer={isTestingServer}
                  isCheckingPorts={isCheckingPorts}
                  isCheckingRemoteFirewall={isCheckingRemoteFirewall}
                  isConfiguringRemoteFirewall={isConfiguringRemoteFirewall}
                  isStartingRemoteServer={isStartingRemoteServer}
                  onTestServer={testServer}
                  onStartRemoteServer={openRemoteServerStart}
                  onOpenRemoteConsole={openRemoteServerConsole}
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
                onTestServer={testServer}
                onStartServer={openRemoteServerStart}
                onOpenRemoteConsole={openRemoteServerConsole}
                onDeployLocalServer={() => setIsDeployLocalModalOpen(true)}
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

      {activeStartServer && remoteConnection && (
        <RemoteServerStartModal
          isOpen={isRemoteStartOpen}
          server={activeStartServer}
          firewallCheck={remoteFirewallCheck}
          startResult={remoteStartResult}
          logs={remoteStartLogs}
          error={remoteStartError}
          isChecking={isCheckingRemoteFirewall}
          isConfiguring={isConfiguringRemoteFirewall}
          isStarting={isStartingRemoteServer}
          onClose={() => {
            setIsRemoteStartOpen(false)
            setActiveStartServer(null)
          }}
          onRecheck={() => void checkRemoteServerFirewall(activeStartServer)}
          onConfigureFirewall={() => void configureRemoteServerFirewall(activeStartServer)}
          onStartServer={() => void startRemoteServer(activeStartServer)}
          onSendCommand={(command) => sendRemoteServerCommand(activeStartServer, command)}
          onStopServer={() => stopRemoteServer(activeStartServer)}
        />
      )}

      {portConflictCheck && activeStartServer && (
        <ServerPortConflictModal
          check={portConflictCheck}
          isKilling={isKillingPorts}
          onCancel={() => {
            setPortConflictCheck(null)
            setActiveStartServer(null)
          }}
          onConfirm={() => void killPortConflictsAndContinue(activeStartServer)}
        />
      )}

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
          <DeployLocalServerModal
            isOpen={isDeployLocalModalOpen}
            connection={remoteConnection}
            onClose={() => setIsDeployLocalModalOpen(false)}
            onSuccess={(deployedServerName, deployedServerId) => {
              window.dispatchEvent(new CustomEvent("pzmm-reveal-server", { detail: { serverId: deployedServerId } }))
              addNotification({
                type: "success",
                title: t("deployLocalServer.successTitle", "Deploy concluído"),
                message: t("deployLocalServer.successMessage", {
                  name: deployedServerName,
                  defaultValue: `Servidor ${deployedServerName} implantado com sucesso na VM.`,
                }),
              })
              void loadServers()
              void loadMods()
            }}
          />
        </>
      )}

      <ServerTestPanel
        hasDownloadProgressCard={downloadManager.isDownloading && activeTab !== "download"}
        onNotification={addNotification}
        onCancelTest={cancelServerTest}
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
