import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Box, Download, Server, Settings, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AppHeader, type AppNotification } from "@/components/AppHeader";
import { AppSidebar } from "@/components/AppSidebar";
import { CreateServerModal } from "@/components/CreateServerModal";
import { Dashboard } from "@/components/Dashboard";
import { DownloadMods } from "@/components/DownloadMods";
import { DownloadProgressCard } from "@/components/DownloadProgressCard";
import { LoadingModsPanel } from "@/components/LoadingModsPanel";
import { ModsList } from "@/components/ModsList";
import { RemoteSteamCmdModal } from "@/components/RemoteSteamCmdModal";
import { RemoteTerminalModal } from "@/components/RemoteTerminalModal";
import { ServerConfigurationModal } from "@/components/ServerConfigurationModal";
import { ServerDetail } from "@/components/ServerDetail";
import { ServerTestPanel } from "@/components/ServerTestPanel";
import {
  RemoteServerStartModal,
  type RemoteServerActionResult,
  type RemoteServerFirewallCheck,
} from "@/components/server/RemoteServerStartModal";
import { ServerPortConflictModal } from "@/components/server/ServerPortConflictModal";
import type { ServerPortCheck } from "@/components/server/ServerPortConflictModal";
import { DeployLocalServerModal } from "@/components/server/DeployLocalServerModal";
import { Settings as SettingsView } from "@/components/Settings";
import {
  LAST_WORKSPACE_KEY,
  WorkspaceSelector,
  readSavedRemoteConnections,
  remoteConfigToDraft,
} from "@/components/WorkspaceSelector";
import { WorkshopWindow } from "@/components/WorkshopWindow";
import { useModsLibrary } from "@/hooks/useModsLibrary";
import { useWorkshopDownloadManager } from "@/hooks/useWorkshopDownloadManager";
import type {
  RemoteConnectionDraft,
  RemoteWorkspaceConfig,
} from "@/lib/commandRunner";
import { getErrorMessage } from "@/lib/errors";
import { findModForServerId, resolveModForBuild } from "@/lib/modBuilds";
import { clearModsLibraryCache } from "@/lib/modsCache";
import { readServerDetailModsCache, writeServerDetailModsCache } from "@/lib/serverDetailModsCache";
import {
  getActiveDependencyChain,
  getWorkshopIdsForModIds,
  isModInCloud,
} from "@/lib/serverMods";
import { readServersCache, writeServersCache } from "@/lib/serversCache";
import { invokeTauri } from "@/lib/tauri";
import type { ZomboidMod } from "@/types/mod";
import type { ServerIniSettings, ZomboidServer } from "@/types/server";

type ServerTestEvent = {
  serverId: string;
  event: "started" | "line" | "ready" | "finished" | "error";
  line?: string;
  error?: string;
};

type DeleteServerResult = {
  backupPath: string;
};

const WORKSHOP_MAPPINGS_API_URL =
  "http://ec2-52-67-72-177.sa-east-1.compute.amazonaws.com:8080";

function isRemoteSetupComplete(config: RemoteWorkspaceConfig | null) {
  const completedStep = Math.min(
    Math.max(config?.remoteSetupCompletedStep ?? 0, 0),
    4,
  );
  return completedStep >= 4;
}

function getInitialWorkspace() {
  const lastWorkspace = window.localStorage.getItem(LAST_WORKSPACE_KEY);

  if (lastWorkspace === "local") {
    return { mode: "local" as const, connection: null };
  }

  if (lastWorkspace?.startsWith("remote:")) {
    const connectionId = lastWorkspace.slice("remote:".length);
    const savedConnection = readSavedRemoteConnections().find(
      (connection) => connection.id === connectionId,
    );

    if (savedConnection) {
      return {
        mode: "remote" as const,
        connection: remoteConfigToDraft(savedConnection),
      };
    }

    window.localStorage.removeItem(LAST_WORKSPACE_KEY);
  }

  return { mode: null, connection: null };
}

function normalizeWorkshopMappings(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {};
  }

  const mappings: Record<string, string> = {};

  for (const [modId, workshopId] of Object.entries(value)) {
    const cleanModId = modId.trim();
    const cleanWorkshopId = String(workshopId ?? "").trim();

    if (cleanModId && /^\d+$/.test(cleanWorkshopId)) {
      mappings[cleanModId] = cleanWorkshopId;
    }
  }

  return mappings;
}

function App() {
  if (window.location.hash.startsWith("#/workshop")) {
    return <WorkshopWindow />;
  }

  const initialWorkspace = useMemo(getInitialWorkspace, []);
  const [workspaceMode, setWorkspaceMode] = useState<"local" | "remote" | "connecting" | null>(
    initialWorkspace.mode === "remote" ? "connecting" : initialWorkspace.mode
  );
  const [remoteConnection, setRemoteConnection] =
    useState<RemoteConnectionDraft | null>(initialWorkspace.connection);
  const [autoConnectError, setAutoConnectError] = useState<string | null>(null);

  const { t } = useTranslation();

  useEffect(() => {
    if (initialWorkspace.mode === "remote" && initialWorkspace.connection) {
      const conn = initialWorkspace.connection;
      invokeTauri<RemoteServerConnectionResult>("test_remote_server_connection", {
        connection: conn,
      })
        .then((result) => {
          setRemoteConnection({
            ...conn,
            host: result.host,
            port: String(result.port),
            serverPath: result.serverPath,
          });
          setWorkspaceMode("remote");
        })
        .catch((err) => {
          console.error("Auto remote connection failed:", err);
          window.localStorage.removeItem(LAST_WORKSPACE_KEY);
          setRemoteConnection(null);
          setWorkspaceMode(null);
          setAutoConnectError(
            t("remoteSetup.autoConnectFailed", "Não foi possível conectar automaticamente ao último servidor.")
          );
        });
    }
  }, [initialWorkspace.mode, initialWorkspace.connection, t]);

  if (workspaceMode === "connecting") {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center bg-[#22272b] text-white">
        <Loader2 size={48} className="animate-spin text-orange-500 mb-4" />
        <h2 className="text-lg font-bold">{t("remoteSetup.connecting", "Conectando ao servidor remoto...")}</h2>
        <p className="text-xs text-gray-500 mt-2">
          {initialWorkspace.connection?.username}@{initialWorkspace.connection?.host}
        </p>
      </main>
    );
  }

  if (workspaceMode === null) {
    return (
      <WorkspaceSelector
        onSelectLocal={() => {
          window.localStorage.setItem(LAST_WORKSPACE_KEY, "local");
          setRemoteConnection(null);
          setWorkspaceMode("local");
        }}
        onSelectRemote={(connection) => {
          setRemoteConnection(connection);
          setWorkspaceMode("remote");
        }}
        initialError={autoConnectError}
      />
    );
  }

  return (
    <LocalWorkspaceApp
      onChangeWorkspace={() => {
        window.localStorage.removeItem(LAST_WORKSPACE_KEY);
        setWorkspaceMode(null);
      }}
      remoteConnection={workspaceMode === "remote" ? remoteConnection : null}
    />
  );
}

function LocalWorkspaceApp({
  onChangeWorkspace,
  remoteConnection,
}: {
  onChangeWorkspace: () => void;
  remoteConnection: RemoteConnectionDraft | null;
}) {
  const [isCreateServerModalOpen, setIsCreateServerModalOpen] = useState(false);
  const [isRemoteSteamCmdModalOpen, setIsRemoteSteamCmdModalOpen] =
    useState(false);
  const [isRemoteTerminalModalOpen, setIsRemoteTerminalModalOpen] =
    useState(false);
  const [isDeployLocalModalOpen, setIsDeployLocalModalOpen] = useState(false);
  const isRemoteWorkspace = remoteConnection !== null;
  const remoteSetupPromptedConnectionRef = useRef<string | null>(null);
  const workspaceCacheId = remoteConnection
    ? `remote:${[
        remoteConnection.name,
        remoteConnection.username,
        remoteConnection.host,
        remoteConnection.port,
      ]
        .map((part) => encodeURIComponent(part.trim()))
        .join(":")}`
    : "local";
  const modsCacheKey =
    workspaceCacheId === "local"
      ? "pzmm:mods-library"
      : `pzmm:mods-library:${workspaceCacheId}`;
  const serversCacheKey =
    workspaceCacheId === "local"
      ? "pzmm:servers"
      : `pzmm:servers:${workspaceCacheId}`;
  const cachedServers = useMemo(
    () => readServersCache(serversCacheKey),
    [serversCacheKey],
  );
  const [serverConfigTarget, setServerConfigTarget] =
    useState<ZomboidServer | null>(null);
  const [activeTab, setActiveTab] = useState("dashboard");
  const [selectedServer, setSelectedServer] = useState<ZomboidServer | null>(
    null,
  );
  const [servers, setServers] = useState<ZomboidServer[]>(
    cachedServers?.servers ?? [],
  );
  const [serversError, setServersError] = useState<string | null>(null);
  const [isLoadingServers, setIsLoadingServers] = useState(!cachedServers);
  const [searchQuery, setSearchQuery] = useState("");
  const [notifications, setNotifications] = useState<AppNotification[]>([]);
  const [runningServerTestId, setRunningServerTestId] = useState<string | null>(
    null,
  );
  const [isTestingServer, setIsTestingServer] = useState(false);
  const [portConflictCheck, setPortConflictCheck] =
    useState<ServerPortCheck | null>(null);
  const [isCheckingPorts, setIsCheckingPorts] = useState(false);
  const [isKillingPorts, setIsKillingPorts] = useState(false);
  const [isRemoteStartOpen, setIsRemoteStartOpen] = useState(false);
  const [remoteFirewallCheck, setRemoteFirewallCheck] =
    useState<RemoteServerFirewallCheck | null>(null);
  const [remoteStartResult, setRemoteStartResult] =
    useState<RemoteServerActionResult | null>(null);
  const [remoteStartLogs, setRemoteStartLogs] = useState<string[]>([]);
  const [remoteStartError, setRemoteStartError] = useState<string | null>(null);
  const [isCheckingRemoteFirewall, setIsCheckingRemoteFirewall] =
    useState(false);
  const [isConfiguringRemoteFirewall, setIsConfiguringRemoteFirewall] =
    useState(false);
  const [isStartingRemoteServer, setIsStartingRemoteServer] = useState(false);
  const [activeStartServer, setActiveStartServer] =
    useState<ZomboidServer | null>(null);
  const activeStartServerRef = useRef<ZomboidServer | null>(null);
  const confirmedOnlineServerIdsRef = useRef<Set<string>>(new Set());
  const [syncError, setSyncError] = useState<string | null>(null);
  const [workshopMappings, setWorkshopMappings] = useState<Record<string, string>>({});
  const autoUploadedModIdsRef = useRef<Set<string>>(new Set());

  async function loadWorkshopMappings() {
    try {
      const mappings = await invokeTauri<Record<string, string>>("get_workshop_mappings");
      setWorkshopMappings(mappings || {});
    } catch (err) {
      console.error("Falha ao carregar mapeamentos locais:", err);
    }
  }

  // Carrega mapeamentos locais em cache imediatamente na montagem do app
  useEffect(() => {
    void loadWorkshopMappings();
  }, []);

  useEffect(() => {
    async function syncWorkshopMappings() {
      setSyncError(null);
      try {
        const response = await fetch(`${WORKSHOP_MAPPINGS_API_URL}/mappings`);
        if (!response.ok) {
          throw new Error(`HTTP error status: ${response.status}`);
        }
        const remoteMappings = normalizeWorkshopMappings(await response.json());
        const localMappings = await invokeTauri<Record<string, string>>("get_workshop_mappings");
        const mergedMappings = { ...(localMappings || {}), ...remoteMappings };

        await invokeTauri("save_workshop_mappings", { mappings: mergedMappings });
        setWorkshopMappings(mergedMappings);
      } catch (err) {
        console.warn("Sincronizacao remota de mapeamentos indisponivel. Usando cache local:", err);
        setSyncError("Falha ao sincronizar o banco de dados de mapeamentos da workshop.");
        await loadWorkshopMappings();
      }
    }
    void syncWorkshopMappings();
  }, [workspaceCacheId]);

  const { t } = useTranslation();
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
    listCommand: isRemoteWorkspace
      ? "list_remote_zomboid_mods"
      : "list_zomboid_mods",
    listArgs:
      isRemoteWorkspace && remoteConnection
        ? { connection: remoteConnection }
        : undefined,
    installCommand: isRemoteWorkspace
      ? "install_remote_zomboid_mod"
      : "install_zomboid_mod",
    installArgs:
      isRemoteWorkspace && remoteConnection
        ? { connection: remoteConnection }
        : undefined,
    backgroundReloadAfterInstall: isRemoteWorkspace,
    useCache: true,
    cacheKey: modsCacheKey,
  });

  // Background Auto-Upload: Se houver mods carregados com Workshop ID que ainda NÃO estão na nuvem, envia em segundo plano sem criar loop
  useEffect(() => {
    if (!hasLoadedMods || mods.length === 0) return;

    const unmappedPairs: { modId: string; workshopId: string }[] = [];

    for (const mod of mods) {
      if (autoUploadedModIdsRef.current.has(mod.id)) continue;

      const cleanWorkshopId = mod.workshopId?.trim();
      if (cleanWorkshopId && /^\d+$/.test(cleanWorkshopId)) {
        const isMappedInCloud = isModInCloud(mod, workshopMappings);
        if (!isMappedInCloud) {
          unmappedPairs.push({
            modId: mod.id,
            workshopId: cleanWorkshopId,
          });
        }
      }
    }

    if (unmappedPairs.length > 0) {
      for (const item of unmappedPairs) {
        autoUploadedModIdsRef.current.add(item.modId);
      }

      console.log(
        `[Background Auto-Upload] Encontrados ${unmappedPairs.length} mods com Workshop ID não cadastrados na nuvem. Enviando...`,
      );

      fetch(`${WORKSHOP_MAPPINGS_API_URL}/mappings/bulk`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          mappings: unmappedPairs,
        }),
      })
        .then(async (res) => {
          if (res.ok) {
            console.log(
              `[Background Auto-Upload] ${unmappedPairs.length} mods sincronizados com a nuvem com sucesso!`,
            );
            const newMappingsObj: Record<string, string> = {};
            for (const item of unmappedPairs) {
              newMappingsObj[item.modId] = item.workshopId;
            }
            const updatedMappings = { ...workshopMappings, ...newMappingsObj };
            await invokeTauri("save_workshop_mappings", { mappings: updatedMappings });
            setWorkshopMappings(updatedMappings);
          }
        })
        .catch((err) => {
          console.error("Falha na sincronização em segundo plano dos mods:", err);
        });
    }
  }, [hasLoadedMods, mods, workshopMappings]);
  const navItems = useMemo(
    () => [
      { id: "dashboard", label: t("nav.servers"), icon: Server },
      { id: "mods", label: "Mods", icon: Box, badge: String(modsCount) },
      { id: "download", label: t("nav.download"), icon: Download },
      { id: "settings", label: t("nav.settings"), icon: Settings },
    ],
    [modsCount, t],
  );
  const downloadManager = useWorkshopDownloadManager({
    isDownloadScreenActive: activeTab === "download",
    remoteConnection,
    onDownloadFinished: refreshModsAfterWorkshopDownload,
    onNotification: addNotification,
  });

  const serverDetailCacheKey = selectedServer
    ? `${workspaceCacheId}:${selectedServer.id}:${selectedServer.gameBuild}`
    : "";
  const cachedServerDetailMods = useMemo(
    () => serverDetailCacheKey ? readServerDetailModsCache(serverDetailCacheKey) : [],
    [serverDetailCacheKey],
  );
  const serverDetailMods = hasLoadedMods || mods.length > 0
    ? mods
    : cachedServerDetailMods;
  const canRenderSelectedServerDetails = hasLoadedMods || serverDetailMods.length > 0;

  useEffect(() => {
    if (!selectedServer || !hasLoadedMods || mods.length === 0 || !serverDetailCacheKey) return;

    const compatibleMods = mods.filter((mod) => resolveModForBuild(mod, selectedServer.gameBuild));
    writeServerDetailModsCache(serverDetailCacheKey, compatibleMods);
  }, [hasLoadedMods, mods, selectedServer, serverDetailCacheKey]);

  async function refreshModsAfterWorkshopDownload() {
    clearModsLibraryCache(modsCacheKey);

    if (isRemoteWorkspace && remoteConnection) {
      await invokeTauri<void>("clear_remote_zomboid_mods_and_images_cache", {
        connection: remoteConnection,
      });
    } else {
      await invokeTauri<void>("clear_zomboid_mods_cache");
    }

    await loadMods();
  }

  function normalizeServers(nextServers: ZomboidServer[]) {
    return [...nextServers].sort((left, right) =>
      left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
    );
  }

  function applyRuntimeStatusOverrides(nextServers: ZomboidServer[]) {
    const confirmedOnlineIds = confirmedOnlineServerIdsRef.current;

    if (confirmedOnlineIds.size === 0) {
      return nextServers;
    }

    return nextServers.map((server) =>
      confirmedOnlineIds.has(server.id) ? { ...server, status: "online" as const } : server,
    );
  }

  function applyServers(nextServers: ZomboidServer[]) {
    const sortedServers = normalizeServers(applyRuntimeStatusOverrides(nextServers));

    setServers(sortedServers);
    writeServersCache(sortedServers, serversCacheKey);
    return sortedServers;
  }

  function updateServers(
    updater: (currentServers: ZomboidServer[]) => ZomboidServer[],
  ) {
    setServers((currentServers) => {
      const nextServers = normalizeServers(updater(currentServers));
      writeServersCache(nextServers, serversCacheKey);
      return nextServers;
    });
  }

  function updateServerStatus(serverId: string, status: ZomboidServer["status"]) {
    if (status === "online") {
      confirmedOnlineServerIdsRef.current.add(serverId);
    } else {
      confirmedOnlineServerIdsRef.current.delete(serverId);
    }

    updateServers((currentServers) =>
      currentServers.map((server) =>
        server.id === serverId ? { ...server, status } : server,
      ),
    );
    setSelectedServer((currentServer) =>
      currentServer?.id === serverId ? { ...currentServer, status } : currentServer,
    );
    setActiveStartServer((currentServer) =>
      currentServer?.id === serverId ? { ...currentServer, status } : currentServer,
    );
  }

  async function loadServers() {
    if (isRemoteWorkspace) {
      if (!remoteConnection) return;
      setIsLoadingServers(true);
      setServersError(null);

      try {
        const foundServers = await invokeTauri<ZomboidServer[]>(
          "list_remote_zomboid_servers",
          {
            connection: remoteConnection,
          },
        );
        const nextServers = applyServers(foundServers);
        setSelectedServer((current) =>
          current
            ? (nextServers.find((server) => server.id === current.id) ?? null)
            : null,
        );
      } catch (error) {
        const message = getErrorMessage(error);
        setServersError(message);
      } finally {
        setIsLoadingServers(false);
      }
      return;
    }

    setIsLoadingServers(true);
    setServersError(null);

    try {
      const foundServers = await invokeTauri<ZomboidServer[]>(
        "list_zomboid_servers",
      );
      const nextServers = applyServers(foundServers);
      setSelectedServer((current) =>
        current
          ? (nextServers.find((server) => server.id === current.id) ?? null)
          : null,
      );
    } catch (error) {
      const message = getErrorMessage(error);
      setServersError(message);
    } finally {
      setIsLoadingServers(false);
    }
  }

  async function saveWorkshopMappingAndSync(modId: string, workshopId: string) {
    try {
      await invokeTauri("save_workshop_mapping", { modId, workshopId });
      await loadWorkshopMappings();
      
      fetch(`${WORKSHOP_MAPPINGS_API_URL}/mappings/bulk`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          mappings: [{ modId, workshopId }],
        }),
      }).catch((err) => {
        console.error("Falha ao sincronizar mapeamento para a nuvem:", err);
      });

      if (selectedServer) {
        const currentServer = servers.find((s) => s.id === selectedServer.id);
        if (currentServer) {
          await updateServerMods(currentServer, currentServer.modIds);
        }
      }
    } catch (err) {
      console.error("Falha ao associar ID da workshop ao mod:", err);
      throw err;
    }
  }

  async function updateServerMods(
    server: ZomboidServer,
    activeModIds: string[],
  ) {
    setServersError(null);
    const workshopIds = getWorkshopIdsForModIds(
      activeModIds,
      mods,
      server.gameBuild,
      workshopMappings,
    );

    await invokeTauri<void>(
      isRemoteWorkspace && remoteConnection
        ? "update_remote_zomboid_server_mods"
        : "update_zomboid_server_mods",
      {
        ...(isRemoteWorkspace && remoteConnection
          ? { connection: remoteConnection }
          : {}),
        serverId: server.id,
        modIds: activeModIds,
        workshopIds,
      },
    );

    const updatedServer = {
      ...server,
      activeModIds: activeModIds ?? [],
      modsCount: activeModIds.length,
    };

    setSelectedServer(updatedServer);
    updateServers((currentServers) =>
      currentServers.map((currentServer) =>
        currentServer.id === server.id ? updatedServer : currentServer,
      ),
    );
  }

  async function toggleServerMod(
    server: ZomboidServer,
    mod: ZomboidMod,
    action: "activate" | "deactivate",
  ) {
    const activeModIds = server.activeModIds ?? [];
    const resolvedMod =
      action === "deactivate" ? mod : resolveModForBuild(mod, server.gameBuild);
    if (!resolvedMod) return;
    const normalizedModId = resolvedMod.id.toLowerCase();
    const nextActiveModIds =
      action === "activate"
        ? activeModIds.some((modId) => modId.toLowerCase() === normalizedModId)
          ? activeModIds
          : [...activeModIds, resolvedMod.id]
        : activeModIds.filter(
            (modId) => modId.toLowerCase() !== normalizedModId,
          );

    try {
      await updateServerMods(server, nextActiveModIds);
    } catch (error) {
      setServersError(getErrorMessage(error));
    }
  }

  async function moveServerMod(
    server: ZomboidServer,
    mod: ZomboidMod,
    position: "start" | "end",
  ) {
    const resolvedMod = resolveModForBuild(mod, server.gameBuild);
    if (!resolvedMod) return;
    const normalizedModId = resolvedMod.id.toLowerCase();
    const activeModIds = server.activeModIds ?? [];
    const activeModIdKeys = new Set(
      activeModIds.map((modId) => modId.toLowerCase()),
    );
    const modsById = new Map(
      mods.flatMap((item) =>
        item.variants.map(
          (variant) =>
            [
              variant.id.toLowerCase(),
              {
                ...item,
                id: variant.id,
                path: variant.path,
                dependencies: variant.dependencies,
                mapNames: variant.mapNames,
              },
            ] as const,
        ),
      ),
    );
    const moveModIds =
      position === "start"
        ? getActiveDependencyChain(mod, modsById, activeModIdKeys)
        : [resolvedMod.id];
    const moveModIdKeys = new Set(
      moveModIds.map((modId) => modId.toLowerCase()),
    );
    const remainingModIds = activeModIds.filter(
      (modId) => !moveModIdKeys.has(modId.toLowerCase()),
    );

    if (!activeModIdKeys.has(normalizedModId)) {
      return;
    }

    const nextActiveModIds =
      position === "start"
        ? [...moveModIds, ...remainingModIds]
        : [...remainingModIds, resolvedMod.id];

    try {
      await updateServerMods(server, nextActiveModIds);
    } catch (error) {
      setServersError(getErrorMessage(error));
    }
  }

  async function activateServerMods(
    server: ZomboidServer,
    modsToActivate: ZomboidMod[],
  ) {
    const nextActiveModIds = [...(server.activeModIds ?? [])];
    const activeModIdsSet = new Set(
      nextActiveModIds.map((modId) => modId.toLowerCase()),
    );

    for (const mod of modsToActivate) {
      const resolvedMod = resolveModForBuild(mod, server.gameBuild);
      if (!resolvedMod) continue;
      const normalizedModId = resolvedMod.id.toLowerCase();

      if (!activeModIdsSet.has(normalizedModId)) {
        nextActiveModIds.push(resolvedMod.id);
        activeModIdsSet.add(normalizedModId);
      }
    }

    const optimisticServer = {
      ...server,
      activeModIds: nextActiveModIds,
      modsCount: nextActiveModIds.length,
    };

    setServersError(null);
    setSelectedServer(optimisticServer);
    updateServers((currentServers) =>
      currentServers.map((currentServer) =>
        currentServer.id === server.id ? optimisticServer : currentServer,
      ),
    );

    try {
      await updateServerMods(server, nextActiveModIds);
    } catch (error) {
      setSelectedServer(server);
      updateServers((currentServers) =>
        currentServers.map((currentServer) =>
          currentServer.id === server.id ? server : currentServer,
        ),
      );
      setServersError(getErrorMessage(error));
    }
  }

  async function createServer(data: {
    name: string;
    modIds: string[];
    gameBuild: "b41" | "b42";
    maxPlayers: number;
  }) {
    const resolvedModIds = data.modIds.flatMap((modId) => {
      const mod = mods.find((item) => item.id === modId);
      const resolved = mod
        ? resolveModForBuild(mod, data.gameBuild)
        : findModForServerId(mods, modId, data.gameBuild);
      return resolved ? [resolved.id] : [];
    });
    const workshopIds = getWorkshopIdsForModIds(
      resolvedModIds,
      mods,
      data.gameBuild,
      workshopMappings,
    );
    const createdServer = await invokeTauri<ZomboidServer>(
      isRemoteWorkspace && remoteConnection
        ? "create_remote_zomboid_server"
        : "create_zomboid_server",
      {
        ...(isRemoteWorkspace && remoteConnection
          ? { connection: remoteConnection }
          : {}),
        name: data.name,
        modIds: resolvedModIds,
        workshopIds,
        gameBuild: data.gameBuild,
        maxPlayers: data.maxPlayers,
      },
    );

    updateServers((currentServers) =>
      [
        ...currentServers.filter((server) => server.id !== createdServer.id),
        createdServer,
      ].sort((left, right) =>
        left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
      ),
    );
    setSelectedServer(createdServer);
    setServerConfigTarget(createdServer);
    setActiveTab("dashboard");
    window.dispatchEvent(
      new CustomEvent("pzmm-reveal-server", {
        detail: { serverId: createdServer.id },
      }),
    );
  }

  async function updateServerSettings(settings: ServerIniSettings) {
    if (!serverConfigTarget) return;

    const updatedServer = await invokeTauri<ZomboidServer>(
      isRemoteWorkspace && remoteConnection
        ? "update_remote_zomboid_server_settings"
        : "update_zomboid_server_settings",
      {
        ...(isRemoteWorkspace && remoteConnection
          ? { connection: remoteConnection }
          : {}),
        serverId: serverConfigTarget.id,
        settings,
      },
    );

    updateServers((currentServers) =>
      currentServers
        .map((server) =>
          server.id === updatedServer.id ? updatedServer : server,
        )
        .sort((left, right) =>
          left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
        ),
    );
    setSelectedServer((currentServer) =>
      currentServer?.id === updatedServer.id ? updatedServer : currentServer,
    );
    setServerConfigTarget(updatedServer);
  }

  async function installDownloadedDependencyForServer(
    server: ZomboidServer,
    dependencyId: string,
  ) {
    const refreshedMods = await loadMods();
    const normalizedDependencyId = dependencyId.trim().toLowerCase();
    const dependency = findModForServerId(
      refreshedMods,
      normalizedDependencyId,
      server.gameBuild,
    );

    if (!dependency) {
      throw new Error(t("dependency.downloadedMissing", { id: dependencyId }));
    }

    if (!dependency.isInstalled) {
      await installMods([dependency]);
    }

    await activateServerMods(server, [
      {
        ...dependency,
        isInstalled: true,
        source: dependency.source === "steam" ? "local" : dependency.source,
      },
    ]);
  }

  async function changeServerBuild(
    server: ZomboidServer,
    gameBuild: "b41" | "b42",
  ) {
    await invokeTauri<void>(
      isRemoteWorkspace && remoteConnection
        ? "update_remote_zomboid_server_build"
        : "update_zomboid_server_build",
      {
        ...(isRemoteWorkspace && remoteConnection
          ? { connection: remoteConnection }
          : {}),
        serverId: server.id,
        gameBuild,
      },
    );
    const updatedServer = { ...server, gameBuild };
    setSelectedServer(updatedServer);
    updateServers((currentServers) =>
      currentServers.map((item) =>
        item.id === server.id ? updatedServer : item,
      ),
    );
  }

  async function deleteServer(server: ZomboidServer) {
    try {
      const result = await invokeTauri<DeleteServerResult>(
        isRemoteWorkspace && remoteConnection
          ? "delete_remote_zomboid_server"
          : "delete_zomboid_server",
        {
          ...(isRemoteWorkspace && remoteConnection
            ? { connection: remoteConnection }
            : {}),
          serverId: server.id,
        },
      );

      updateServers((currentServers) =>
        currentServers.filter((item) => item.id !== server.id),
      );
      setSelectedServer((current) =>
        current?.id === server.id ? null : current,
      );
      await loadServers();
      addNotification({
        title: t("dashboard.deleteSuccessTitle"),
        message: t("dashboard.deleteSuccessBody", {
          name: server.name,
          backupPath: result.backupPath,
        }),
        tone: "success",
      });
    } catch (error) {
      const message = getErrorMessage(error);
      setServersError(message);
      addNotification({
        title: t("dashboard.deleteErrorTitle"),
        message: t("dashboard.deleteErrorBody", {
          name: server.name,
          error: message,
        }),
        tone: "error",
      });
    }
  }

  async function deleteMod(mod: ZomboidMod) {
    try {
      await invokeTauri("delete_zomboid_mod_command", {
        packagePath: mod.packagePath,
        connection: remoteConnection,
      });
      addNotification({
        tone: "success",
        title: t("mods.deleteSuccessTitle", "Mod Excluído"),
        message: t("mods.deleteSuccessMessage", "O mod foi excluído com sucesso do workspace."),
      });
      await loadMods();
    } catch (err) {
      addNotification({
        tone: "error",
        title: t("mods.deleteErrorTitle", "Falha ao Excluir"),
        message: t("mods.deleteErrorMessage", "Não foi possível excluir o mod: {{error}}", { error: getErrorMessage(err) }),
      });
    }
  }

  async function scanData() {
    if (isRemoteWorkspace) {
      await loadServers();
      void loadModsInBackground();
      return;
    }

    await loadServers();

    void loadModsInBackground();
  }

  async function rescanModsFromScratch() {
    if (isRemoteWorkspace) {
      clearModsLibraryCache(modsCacheKey);
      if (remoteConnection) {
        await invokeTauri<void>("clear_remote_zomboid_mods_and_images_cache", {
          connection: remoteConnection,
        });
      }
      await loadMods();
      return;
    }

    clearModsLibraryCache(modsCacheKey);
    await invokeTauri<void>("clear_zomboid_mods_cache");
    await loadMods();
  }

  async function loadInitialData() {
    if (isRemoteWorkspace) {
      if (!remoteConnection) return;
      setIsLoadingServers(true);
      setServersError(null);
      try {
        const foundServers = await invokeTauri<ZomboidServer[]>(
          "list_remote_zomboid_servers",
          {
            connection: remoteConnection,
          },
        );
        const nextServers = applyServers(foundServers);
        setSelectedServer((current) =>
          current
            ? (nextServers.find((server) => server.id === current.id) ?? null)
            : null,
        );
        void loadModsInBackground();
      } catch (error) {
        window.localStorage.removeItem(LAST_WORKSPACE_KEY);
        setWorkspaceMode(null);
        setRemoteConnection(null);
        addNotification({
          tone: "error",
          title: t("remoteSetup.errorHeader", "Falha na Conexão"),
          message: t("remoteSetup.connectionFailedNotice", {
            defaultValue: "Não foi possível estabelecer contato com a VM remota. Retornando ao seletor.",
          }),
        });
      } finally {
        setIsLoadingServers(false);
      }
      return;
    }

    await loadServers();
    void loadModsInBackground();
  }

  async function testServer(server: ZomboidServer, skipPortCheck = false) {
    const isCurrentServerTesting =
      isTestingServer || runningServerTestId === server.id;
    if (isCurrentServerTesting) {
      window.dispatchEvent(
        new CustomEvent("pzmm-open-server-test-panel", {
          detail: { serverId: server.id },
        }),
      );
      return;
    }

    setActiveStartServer(server);

    if (!remoteConnection && !skipPortCheck) {
      setIsCheckingPorts(true);

      try {
        const check = await invokeTauri<ServerPortCheck>(
          "check_zomboid_server_ports",
          {
            serverId: server.id,
          },
        );

        if (check.usages.length > 0) {
          setPortConflictCheck(check);
          return;
        }
      } catch (error) {
        window.dispatchEvent(
          new CustomEvent("pzmm-open-server-test-panel", {
            detail: { serverId: server.id, error: getErrorMessage(error) },
          }),
        );
        return;
      } finally {
        setIsCheckingPorts(false);
      }
    }

    setIsTestingServer(true);
    window.dispatchEvent(
      new CustomEvent("pzmm-open-server-test-panel", {
        detail: { serverId: server.id },
      }),
    );

    try {
      await invokeTauri(
        remoteConnection
          ? "start_remote_zomboid_server_test"
          : "start_zomboid_server_test",
        {
          ...(remoteConnection ? { connection: remoteConnection } : {}),
          serverId: server.id,
        },
      );
    } catch (error) {
      window.dispatchEvent(
        new CustomEvent("pzmm-open-server-test-panel", {
          detail: { serverId: server.id, error: getErrorMessage(error) },
        }),
      );
    } finally {
      setIsTestingServer(false);
    }
  }

  function startServer(server: ZomboidServer) {
    if (remoteConnection) {
      void openRemoteServerStart(server);
    }
  }

  async function cancelServerTest(serverId: string) {
    try {
      await invokeTauri<void>(
        remoteConnection
          ? "cancel_remote_zomboid_server_test"
          : "cancel_zomboid_server_test",
        {
          ...(remoteConnection ? { connection: remoteConnection } : {}),
          serverId,
        },
      );
    } catch (error) {
      addNotification({
        title: t("serverTest.failedTitle"),
        message: getErrorMessage(error),
        tone: "error",
      });
    } finally {
      setRunningServerTestId((currentServerId) =>
        currentServerId === serverId ? null : currentServerId,
      );
      setIsTestingServer(false);
    }
  }
  async function checkRemoteServerFirewall(server: ZomboidServer) {
    if (!remoteConnection || !server) {
      return;
    }

    setIsRemoteStartOpen(true);
    setRemoteStartResult(null);
    setRemoteStartError(null);
    setRemoteStartLogs([t("remoteStart.checkingSetup", { name: server.name })]);
    setIsCheckingRemoteFirewall(true);

    try {
      const check = await invokeTauri<RemoteServerFirewallCheck>(
        "check_remote_zomboid_server_firewall",
        {
          connection: remoteConnection,
          serverId: server.id,
        },
      );
      setRemoteFirewallCheck(check);
      setRemoteStartLogs([
        ...check.logs,
        check.isConfigured
          ? t("remoteStart.firewallReady")
          : t("remoteStart.firewallNeedsConfig"),
      ]);
    } catch (error) {
      setRemoteStartError(getErrorMessage(error));
      setRemoteStartLogs((currentLogs) => [
        ...currentLogs,
        getErrorMessage(error),
      ]);
    } finally {
      setIsCheckingRemoteFirewall(false);
    }
  }

  function appendRemoteStartLogs(nextLogs: string[]) {
    if (nextLogs.length === 0) return;

    setRemoteStartLogs((currentLogs) => {
      const seen = new Set(currentLogs);
      const merged = [...currentLogs];

      for (const line of nextLogs) {
        if (!seen.has(line)) {
          seen.add(line);
          merged.push(line);
        }
      }

      return merged;
    });
  }

  function isRemoteStartupPending(result: RemoteServerActionResult | null) {
    if (!result?.success) return false;

    return /not detected|keeps starting|still starting|not open yet|finalizing startup status/i.test(
      result.message,
    );
  }
  async function configureRemoteServerFirewall(server: ZomboidServer) {
    if (!remoteConnection || !server) {
      return;
    }

    setRemoteStartError(null);
    setIsConfiguringRemoteFirewall(true);
    setRemoteStartLogs((currentLogs) => [
      ...currentLogs,
      t("remoteStart.configuringFirewall"),
    ]);

    try {
      const result = await invokeTauri<RemoteServerActionResult>(
        "configure_remote_zomboid_server_firewall",
        {
          connection: remoteConnection,
          serverId: server.id,
        },
      );
      appendRemoteStartLogs([...result.logs, result.message]);

      const check = await invokeTauri<RemoteServerFirewallCheck>(
        "check_remote_zomboid_server_firewall",
        {
          connection: remoteConnection,
          serverId: server.id,
        },
      );
      setRemoteFirewallCheck(check);
      setRemoteStartLogs((currentLogs) => [
        ...currentLogs,
        ...check.logs,
        check.isConfigured
          ? t("remoteStart.firewallReady")
          : t("remoteStart.firewallStillNeedsAttention"),
      ]);
    } catch (error) {
      setRemoteStartError(getErrorMessage(error));
      setRemoteStartLogs((currentLogs) => [
        ...currentLogs,
        getErrorMessage(error),
      ]);
    } finally {
      setIsConfiguringRemoteFirewall(false);
    }
  }

  async function startRemoteServer(
    server: ZomboidServer,
    options: { noSteam: boolean },
  ) {
    if (!remoteConnection || !server) {
      return;
    }

    setRemoteStartError(null);
    setIsStartingRemoteServer(true);
    appendRemoteStartLogs([
      t("remoteStart.checkingFirewallBeforeStart"),
    ]);

    try {
      let firewallCheck = await invokeTauri<RemoteServerFirewallCheck>(
        "check_remote_zomboid_server_firewall",
        {
          connection: remoteConnection,
          serverId: server.id,
        },
      );
      setRemoteFirewallCheck(firewallCheck);
      appendRemoteStartLogs(firewallCheck.logs);

      if (!firewallCheck.isConfigured) {
        appendRemoteStartLogs([t("remoteStart.firewallMissingConfiguring")]);
        const firewallResult = await invokeTauri<RemoteServerActionResult>(
          "configure_remote_zomboid_server_firewall",
          {
            connection: remoteConnection,
            serverId: server.id,
          },
        );
        appendRemoteStartLogs([...firewallResult.logs, firewallResult.message]);

        firewallCheck = await invokeTauri<RemoteServerFirewallCheck>(
          "check_remote_zomboid_server_firewall",
          {
            connection: remoteConnection,
            serverId: server.id,
          },
        );
        setRemoteFirewallCheck(firewallCheck);
        appendRemoteStartLogs(firewallCheck.logs);

        if (!firewallCheck.isConfigured) {
          throw new Error(t("remoteStart.firewallStillMissing"));
        }
      }

      appendRemoteStartLogs([
        t("remoteStart.startingServer"),
        ...(options.noSteam ? ["Launch option enabled: -nosteam"] : []),
        t("remoteStart.streamingStartup"),
      ]);

      const result = await invokeTauri<RemoteServerActionResult>(
        "start_remote_zomboid_server",
        {
          connection: remoteConnection,
          serverId: server.id,
          noSteam: options.noSteam,
        },
      );
      setRemoteStartResult(result);
      appendRemoteStartLogs([...result.logs, result.message]);
    } catch (error) {
      setIsStartingRemoteServer(false);
      setRemoteStartError(getErrorMessage(error));
      setRemoteStartLogs((currentLogs) => [
        ...currentLogs,
        getErrorMessage(error),
      ]);
    }
  }

  async function sendRemoteServerCommand(
    server: ZomboidServer,
    command: string,
  ) {
    if (!remoteConnection || !server) {
      return;
    }

    setRemoteStartError(null);
    appendRemoteStartLogs([`> ${command}`]);

    try {
      const result = await invokeTauri<RemoteServerActionResult>(
        "send_remote_zomboid_server_command",
        {
          connection: remoteConnection,
          serverId: server.id,
          command,
        },
      );
      appendRemoteStartLogs([...result.logs, result.message]);
    } catch (error) {
      setRemoteStartError(getErrorMessage(error));
      setRemoteStartLogs((currentLogs) => [
        ...currentLogs,
        getErrorMessage(error),
      ]);
      throw error;
    }
  }

  async function stopRemoteServer(server: ZomboidServer) {
    await sendRemoteServerCommand(server, "quit");
    setRemoteStartResult({
      success: false,
      message: t("remoteStart.stopQueued"),
      command: "quit",
      logs: [],
    });
    setRemoteStartLogs((currentLogs) => [
      ...currentLogs,
      t("remoteStart.stopQueuedLog"),
    ]);
    window.setTimeout(() => {
      updateServerStatus(server.id, "offline");
      void loadServers();
    }, 5000);
  }

  async function openRemoteServerStart(server: ZomboidServer) {
    const isSameServer = activeStartServerRef.current?.id === server.id;
    setActiveStartServer(server);
    setIsRemoteStartOpen(true);

    if (
      isSameServer &&
      (isStartingRemoteServer || remoteStartLogs.length > 0 || remoteStartResult)
    ) {
      return;
    }

    await checkRemoteServerFirewall(server);
  }

  async function openRemoteServerConsole(server: ZomboidServer) {
    if (server.status !== "online") {
      return;
    }

    const isSameServer = activeStartServerRef.current?.id === server.id;
    setActiveStartServer(server);
    setIsRemoteStartOpen(true);
    setRemoteStartError(null);
    setRemoteStartResult({
      success: true,
      message:
        t("remoteStart.runningConsoleReady"),
      command: "send-server-command",
      logs: [],
    });
    setRemoteStartLogs((currentLogs) =>
      isSameServer && currentLogs.length > 0
        ? [...currentLogs, t("remoteStart.consoleOpened", { name: server.name })]
        : [
            t("remoteStart.consoleOpened", { name: server.name }),
            t("remoteStart.commandsQueued"),
          ],
    );

    try {
      if (isRemoteWorkspace && remoteConnection) {
        await invokeTauri<RemoteServerActionResult>(
          "stream_remote_zomboid_server_logs",
          {
            connection: remoteConnection,
            serverId: server.id,
          }
        );
      }
    } catch (err) {
      console.error("Could not stream remote server logs:", err);
    }
  }

  async function killPortConflictsAndContinue(server: ZomboidServer) {
    if (!portConflictCheck) {
      return;
    }

    setIsKillingPorts(true);

    try {
      await invokeTauri("kill_processes_by_pid", {
        pids: Array.from(
          new Set(portConflictCheck.usages.map((usage) => usage.pid)),
        ),
      });
      setPortConflictCheck(null);
      await testServer(server, true);
    } catch (error) {
      window.dispatchEvent(
        new CustomEvent("pzmm-open-server-test-panel", {
          detail: { serverId: server.id, error: getErrorMessage(error) },
        }),
      );
    } finally {
      setIsKillingPorts(false);
    }
  }

  function addNotification(
    notification: Omit<AppNotification, "id" | "createdAt" | "isRead">,
  ) {
    setNotifications((currentNotifications) =>
      [
        {
          ...notification,
          id: `${Date.now()}:${Math.random().toString(16).slice(2)}`,
          createdAt: new Date().toISOString(),
          isRead: false,
        },
        ...currentNotifications,
      ].slice(0, 30),
    );
  }

  const uploadLocalModToRemote = async (mod: ZomboidMod) => {
    if (!isRemoteWorkspace || !remoteConnection) {
      addNotification({
        title: t("remoteSetup.errorHeader", "Erro"),
        message: t("remoteSetup.noActiveConnection", "Nenhuma conexão remota ativa."),
        tone: "error",
      });
      return;
    }
    try {
      await invokeTauri("upload_local_mod_to_remote", {
        connection: remoteConnection,
        modId: mod.id,
        localModPath: mod.packagePath,
      });
      addNotification({
        title: t("mods.uploadSuccessHeader", "Mod Enviado"),
        message: t("mods.uploadSuccess", "Mod enviado com sucesso!"),
        tone: "success",
      });
    } catch (err) {
      addNotification({
        title: t("mods.uploadErrorHeader", "Falha no Envio"),
        message: t("mods.uploadError", { error: String(err) }),
        tone: "error",
      });
      throw err;
    }
  };

  function handleNotificationClick(notification: AppNotification) {
    setNotifications((currentNotifications) =>
      currentNotifications.map((currentNotification) =>
        currentNotification.id === notification.id
          ? { ...currentNotification, isRead: true }
          : currentNotification,
      ),
    );

    if (notification.action?.type === "server-test") {
      window.dispatchEvent(
        new CustomEvent("pzmm-open-server-test-panel", {
          detail: { serverId: notification.action.serverId },
        }),
      );
    }

    if (notification.action?.type === "download-result") {
      setSelectedServer(null);
      setActiveTab("download");
      downloadManager.openResultDetails(notification.action.result);
    }
  }

  function markAllNotificationsRead() {
    setNotifications((currentNotifications) =>
      currentNotifications.map((notification) => ({
        ...notification,
        isRead: true,
      })),
    );
  }

  useEffect(() => {
    activeStartServerRef.current = activeStartServer;
  }, [activeStartServer]);

  useEffect(() => {
    void loadInitialData();
  }, []);

  useEffect(() => {
    if (!isRemoteWorkspace || !remoteConnection) {
      remoteSetupPromptedConnectionRef.current = null;
      return;
    }

    const connectionKey = workspaceCacheId;
    if (remoteSetupPromptedConnectionRef.current === connectionKey) {
      return;
    }

    let isMounted = true;
    void invokeTauri<RemoteWorkspaceConfig | null>(
      "get_remote_workspace_config",
      { connection: remoteConnection },
    )
      .then((config) => {
        if (!isMounted || isRemoteSetupComplete(config)) return;

        remoteSetupPromptedConnectionRef.current = connectionKey;
        setIsRemoteSteamCmdModalOpen(true);
      })
      .catch(() => {
        if (!isMounted) return;

        remoteSetupPromptedConnectionRef.current = connectionKey;
        setIsRemoteSteamCmdModalOpen(true);
      });

    return () => {
      isMounted = false;
    };
  }, [isRemoteWorkspace, remoteConnection, workspaceCacheId]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<ServerTestEvent>("server-test-event", (event) => {
      const payload = event.payload;

      if (payload.event === "started") {
        setRunningServerTestId(payload.serverId);
        return;
      }

      if (payload.event === "finished" || payload.event === "error") {
        setRunningServerTestId((currentServerId) =>
          currentServerId === payload.serverId ? null : currentServerId,
        );
      }
    }).then((unsubscribe) => {
      unlisten = unsubscribe;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<ServerTestEvent>("remote-server-start-event", (event) => {
      const payload = event.payload;
      const currentStartServer = activeStartServerRef.current;
      if (!currentStartServer || payload.serverId !== currentStartServer.id) {
        return;
      }

      if (payload.event === "started") {
        setIsStartingRemoteServer(true);
        appendRemoteStartLogs([
          t("remoteStart.processStarted"),
        ]);
        return;
      }

      if (payload.event === "line" && payload.line) {
        appendRemoteStartLogs([payload.line]);
        if (/SERVER IS RUNNING|remote server is running on port|appears to be running|is running on port/i.test(payload.line)) {
          updateServerStatus(payload.serverId, "online");
          setIsStartingRemoteServer(false);
          setRemoteStartError(null);
          setRemoteStartResult({
            success: true,
            message:
              t("remoteStart.runningConsoleReady"),
            command: "start-server-streaming",
            logs: [],
          });
          window.setTimeout(() => {
            void loadServers();
          }, 1500);
        }
        return;
      }

      if (payload.event === "ready") {
        updateServerStatus(payload.serverId, "online");
        setIsStartingRemoteServer(false);
        setRemoteStartError(null);
        setRemoteStartResult({
          success: true,
          message:
            t("remoteStart.runningConsoleReady"),
          command: "start-server-streaming",
          logs: [],
        });
        window.setTimeout(() => {
          void loadServers();
        }, 1500);
        return;
      }

      if (payload.event === "finished") {
        updateServerStatus(payload.serverId, "offline");
        setIsStartingRemoteServer(false);
        setRemoteStartResult({
          success: false,
          message: t("remoteStart.processExited"),
          command: "start-server-streaming",
          logs: [],
        });
        appendRemoteStartLogs([t("remoteStart.processExited")]);
        window.setTimeout(() => {
          void loadServers();
        }, 1500);
        return;
      }

      if (payload.event === "error") {
        updateServerStatus(payload.serverId, "offline");
        setIsStartingRemoteServer(false);
        const message = payload.error ?? t("remoteStart.startFailed");
        setRemoteStartError(message);
        setRemoteStartResult({
          success: false,
          message,
          command: "start-server-streaming",
          logs: [],
        });
        appendRemoteStartLogs([message]);
      }
    }).then((unsubscribe) => {
      unlisten = unsubscribe;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (
      !remoteConnection ||
      !activeStartServer ||
      !isRemoteStartupPending(remoteStartResult)
    ) {
      return;
    }

    let cancelled = false;
    let timer: number | undefined;

    async function pollRemoteServerStatus() {
      if (!remoteConnection || !activeStartServer) return;

      try {
        const result = await invokeTauri<RemoteServerActionResult>(
          "check_remote_zomboid_server_status",
          {
            connection: remoteConnection,
            serverId: activeStartServer.id,
          },
        );

        if (cancelled) return;

        setRemoteStartResult(result);
        appendRemoteStartLogs([...result.logs, result.message]);

        if (
          result.success &&
          /appears to be running|is running on port|remote server is running on port/i.test(result.message)
        ) {
          updateServerStatus(activeStartServer.id, "online");
          setIsStartingRemoteServer(false);
          setRemoteStartError(null);
          setRemoteStartResult({
            success: true,
            message:
              t("remoteStart.runningConsoleReady"),
            command: result.command,
            logs: result.logs,
          });
          window.setTimeout(() => {
            void loadServers();
          }, 1500);
          return;
        }

        if (!result.success) {
          updateServerStatus(activeStartServer.id, "offline");
          return;
        }
      } catch (error) {
        if (cancelled) return;
        appendRemoteStartLogs([getErrorMessage(error)]);
      }

      if (!cancelled) {
        timer = window.setTimeout(pollRemoteServerStatus, 5000);
      }
    }

    timer = window.setTimeout(pollRemoteServerStatus, 3000);

    return () => {
      cancelled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
    };
  }, [
    remoteConnection,
    activeStartServer,
    remoteStartResult?.message,
    remoteStartResult?.success,
  ]);
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<string>("native-menu", (event) => {
      switch (event.payload) {
        case "new_server":
          setIsCreateServerModalOpen(true);
          break;
        case "show_dashboard":
          setSelectedServer(null);
          setActiveTab("dashboard");
          break;
        case "show_mods":
          setSelectedServer(null);
          setActiveTab("mods");
          void ensureModsLoaded();
          break;
        case "show_downloads":
          if (isRemoteWorkspace) {
            setActiveTab("dashboard");
            break;
          }
          setSelectedServer(null);
          setActiveTab("download");
          break;
        case "show_settings":
          setSelectedServer(null);
          setActiveTab("settings");
          break;
        case "scan_mods":
          void scanData();
          break;
        case "bring_steam_mods":
          if (!isRemoteWorkspace) {
            void installAllUninstalledMods();
          }
          break;
        case "reload":
          window.location.reload();
          break;
      }
    }).then((unsubscribe) => {
      unlisten = unsubscribe;
    });

    return () => {
      unlisten?.();
    };
  });

  return (
    <main className="flex h-screen w-screen overflow-hidden bg-[#22272b] text-white">
      <AppSidebar
        activeTab={activeTab}
        items={navItems}
        onChangeWorkspace={onChangeWorkspace}
        onTabChange={(tabId) => {
          setActiveTab(tabId);
          setSelectedServer(null);
          if (!isRemoteWorkspace && tabId === "mods") {
            void ensureModsLoaded();
          }
        }}
      />

      <div className="flex-1 flex flex-col min-w-0">
        <AppHeader
          onScanMods={scanData}
          onInstallAllMods={
            isRemoteWorkspace ? undefined : installAllUninstalledMods
          }
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

        {syncError && (
          <div className="flex items-center justify-between gap-2 px-6 py-2 bg-amber-500/10 border-b border-amber-500/20 text-xs text-amber-300 animate-in fade-in duration-300">
            <div className="flex items-center gap-2">
              <span className="h-1.5 w-1.5 rounded-full bg-amber-500 shrink-0" />
              <span>{syncError}</span>
            </div>
            <button
              onClick={() => setSyncError(null)}
              className="text-amber-300/60 hover:text-amber-300 font-bold transition-colors"
            >
              Fechar
            </button>
          </div>
        )}

        <div className="flex-1 overflow-hidden relative">
          {activeTab === "dashboard" &&
            (selectedServer ? (
              !canRenderSelectedServerDetails ? (
                <LoadingModsPanel
                  error={modsError}
                  isLoading={isLoadingMods}
                  onRetry={ensureModsLoaded}
                />
              ) : (
                <ServerDetail
                  server={selectedServer}
                  allMods={serverDetailMods ?? []}
                  onBack={() => setSelectedServer(null)}
                  onInstallMods={installMods}
                  workshopMappings={workshopMappings}
                  onSaveWorkshopMapping={saveWorkshopMappingAndSync}
                  onUpdateServerMods={updateServerMods}
                  onActivateMods={(modsToActivate) =>
                    activateServerMods(selectedServer, modsToActivate)
                  }
                  onToggleMod={(mod, action) =>
                    toggleServerMod(selectedServer, mod, action)
                  }
                  onMoveActiveMod={(mod, position) =>
                    moveServerMod(selectedServer, mod, position)
                  }
                  onRefreshMods={async () => {
                    await loadMods();
                  }}
                  onDependencyDownloaded={(dependencyId) =>
                    installDownloadedDependencyForServer(
                      selectedServer,
                      dependencyId,
                    )
                  }
                  onOpenSettings={() => setActiveTab("settings")}
                  runningServerTestId={runningServerTestId}
                  onChangeBuild={(gameBuild) =>
                    changeServerBuild(selectedServer, gameBuild)
                  }
                  onConfigureServer={setServerConfigTarget}
                  remoteConnection={remoteConnection}
                  isTestingServer={isTestingServer}
                  isCheckingPorts={isCheckingPorts}
                  isCheckingRemoteFirewall={isCheckingRemoteFirewall}
                  isConfiguringRemoteFirewall={isConfiguringRemoteFirewall}
                  isStartingRemoteServer={isStartingRemoteServer}
                  onTestServer={testServer}
                  onStartRemoteServer={startServer}
                  onOpenRemoteConsole={openRemoteServerConsole}
                  onStopRemoteServer={stopRemoteServer}
                />
              )
            ) : (
              <Dashboard
                servers={servers}
                isLoading={isLoadingServers}
                error={serversError}
                onRefresh={loadServers}
                onCreateServer={() => {
                  setIsCreateServerModalOpen(true);
                  void ensureModsLoaded();
                }}
                searchQuery={searchQuery}
                onDeleteServer={deleteServer}
                onConfigureServer={setServerConfigTarget}
                isReadOnly={isRemoteWorkspace}
                canCreateServer
                onServerClick={(server) => {
                  setSelectedServer(server);
                  void ensureModsLoaded();
                }}
                onTestServer={testServer}
                onStartServer={isRemoteWorkspace ? startServer : undefined}
                onOpenRemoteConsole={openRemoteServerConsole}
                onStopRemoteServer={stopRemoteServer}
                onDeployLocalServer={() => setIsDeployLocalModalOpen(true)}
              />
            ))}
          {activeTab === "mods" && (
            <ModsList
              mods={mods}
              isLoading={isLoadingMods}
              error={modsError}
              onRefresh={loadMods}
              workshopMappings={workshopMappings}
              onSaveWorkshopMapping={saveWorkshopMappingAndSync}
              onInstall={async (modsToInstall) => {
                await installMods(modsToInstall);
              }}
              onInstallAll={installAllUninstalledMods}
              isInstallingAll={isInstallingAllMods}
              onOpenSettings={() => setActiveTab("settings")}
              searchQuery={searchQuery}
              onSearchChange={setSearchQuery}
              remoteConnection={remoteConnection}
              onDelete={deleteMod}
              isReadOnly={false}
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
            <SettingsView
              onRescanMods={rescanModsFromScratch}
              remoteConnection={remoteConnection}
            />
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
            setIsRemoteStartOpen(false);
          }}
          onRecheck={() => void checkRemoteServerFirewall(activeStartServer)}
          onConfigureFirewall={() =>
            void configureRemoteServerFirewall(activeStartServer)
          }
          onStartServer={(noSteam) =>
            void startRemoteServer(activeStartServer, { noSteam })
          }
          onSendCommand={(command) =>
            sendRemoteServerCommand(activeStartServer, command)
          }
          onStopServer={() => stopRemoteServer(activeStartServer)}
        />
      )}

      {portConflictCheck && activeStartServer && (
        <ServerPortConflictModal
          check={portConflictCheck}
          isKilling={isKillingPorts}
          onCancel={() => {
            setPortConflictCheck(null);
            setActiveStartServer(null);
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
              window.dispatchEvent(
                new CustomEvent("pzmm-reveal-server", {
                  detail: { serverId: deployedServerId },
                }),
              );
              addNotification({
                tone: "success",
                title: t("deployLocalServer.successTitle", "Deploy concluído"),
                message: t("deployLocalServer.successMessage", {
                  name: deployedServerName,
                  defaultValue: `Servidor ${deployedServerName} implantado com sucesso na VM.`,
                }),
              });
              void loadServers();
              void loadMods();
            }}
          />
        </>
      )}

      <ServerTestPanel
        hasDownloadProgressCard={
          downloadManager.isDownloading && activeTab !== "download"
        }
        onNotification={addNotification}
        onCancelTest={cancelServerTest}
      />

      {downloadManager.isDownloading && activeTab !== "download" && (
        <DownloadProgressCard
          manager={downloadManager}
          onOpen={() => {
            setSelectedServer(null);
            setActiveTab("download");
          }}
        />
      )}
    </main>
  );
}

export default App;
