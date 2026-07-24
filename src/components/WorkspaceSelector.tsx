import {
  ArrowLeft,
  CheckCircle2,
  Clipboard,
  FileKey2,
  Folder,
  HelpCircle,
  KeyRound,
  Lock,
  MonitorCog,
  Network,
  Server,
  ShieldAlert,
  Trash2,
  Wifi,
  Search,
  Plus,
  Play,
  X,
  RefreshCw,
  Save,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import type {
  RemoteConnectionDraft,
  RemoteWorkspaceConfig,
} from "@/lib/commandRunner";
import { getErrorMessage } from "@/lib/errors";
import { invokeTauri } from "@/lib/tauri";

type WorkspaceSelectorProps = {
  onSelectLocal: () => void;
  onSelectRemote: (connection: RemoteConnectionDraft) => void;
  initialError?: string | null;
};

type RemoteServerConnectionResult = {
  name: string;
  host: string;
  port: number;
  serverPath: string;
  message: string;
  latencyMs: number;
  diagnosticLog: string;
};

type RemoteServerLatencyResult = {
  host: string;
  port: number;
  success: boolean;
  latencyMs?: number;
  error?: string;
};

type SavedRemoteConnection = RemoteWorkspaceConfig & {
  id: string;
  savedAt: string;
};

export const SAVED_REMOTE_CONNECTIONS_KEY = "pzmm:remote-connections";
export const SAVED_REMOTE_CONNECTIONS_VERSION = 1;
export const LAST_WORKSPACE_KEY = "pzmm:last-workspace";

const initialRemoteConnection: RemoteConnectionDraft = {
  name: "",
  host: "",
  port: "22",
  username: "",
  authMethod: "key",
  password: "",
  sshKeyPath: "",
  serverPath: "",
};

function isLegacyPzManagerPath(path?: string) {
  return Boolean(
    path
      ?.trim()
      .replace(/\//g, "\\")
      .toLowerCase()
      .startsWith("c:\\pzmanager\\"),
  );
}

function cleanLegacyPath(path?: string) {
  return path && !isLegacyPzManagerPath(path) ? path : "";
}

export function remoteConnectionId(
  connection: Pick<RemoteConnectionDraft, "host" | "port" | "username">,
) {
  return [
    connection.host.trim().toLowerCase(),
    connection.port.trim(),
    connection.username.trim().toLowerCase(),
  ]
    .map(encodeURIComponent)
    .join(":");
}

export function remoteConfigToDraft(
  config: Partial<RemoteWorkspaceConfig>,
): RemoteConnectionDraft {
  return {
    name: config.name ?? "",
    host: config.host ?? "",
    port: config.port || "22",
    username: config.username ?? "",
    authMethod: "key",
    password: "",
    sshKeyPath: config.sshKeyPath ?? "",
    serverPath: cleanLegacyPath(config.serverPath),
  };
}

function defaultRemoteConfig(
  connection: RemoteConnectionDraft,
  existing?: Partial<RemoteWorkspaceConfig>,
): RemoteWorkspaceConfig {
  return {
    ...connection,
    password: "",
    sshKeyPath: connection.authMethod === "key" ? connection.sshKeyPath : "",
    serverPath:
      cleanLegacyPath(existing?.serverPath) ||
      cleanLegacyPath(connection.serverPath),
    remoteSteamcmdDir: cleanLegacyPath(existing?.remoteSteamcmdDir),
    remoteSteamcmdPath: cleanLegacyPath(existing?.remoteSteamcmdPath),
    remoteZomboidServerDir: cleanLegacyPath(existing?.remoteZomboidServerDir),
    remoteZomboidServerPath: cleanLegacyPath(existing?.remoteZomboidServerPath),
    remoteZomboidServerOwner: existing?.remoteZomboidServerOwner || "pzmm",
    remoteZomboidDataOwner: existing?.remoteZomboidDataOwner || "pzmm",
    remoteClientRam: existing?.remoteClientRam || "4.00",
    remoteServerRam: existing?.remoteServerRam || "4.00",
    remoteSetupCompletedStep: existing?.remoteSetupCompletedStep ?? 0,
    remoteModLocations: existing?.remoteModLocations || [],
  };
}

export function readSavedRemoteConnections() {
  try {
    const raw = window.localStorage.getItem(SAVED_REMOTE_CONNECTIONS_KEY);

    if (!raw) {
      return [];
    }

    const cache = JSON.parse(raw) as {
      version?: number;
      connections?: Partial<SavedRemoteConnection>[];
    };

    if (
      cache.version !== SAVED_REMOTE_CONNECTIONS_VERSION ||
      !Array.isArray(cache.connections)
    ) {
      window.localStorage.removeItem(SAVED_REMOTE_CONNECTIONS_KEY);
      return [];
    }

    const normalizedConnections = cache.connections
      .filter(
        (connection): connection is SavedRemoteConnection =>
          typeof connection.id === "string" &&
          typeof connection.savedAt === "string" &&
          typeof connection.name === "string" &&
          typeof connection.host === "string" &&
          typeof connection.port === "string" &&
          typeof connection.username === "string" &&
          typeof connection.authMethod === "string" &&
          typeof connection.sshKeyPath === "string" &&
          typeof connection.serverPath === "string",
      )
      .map((connection) => ({
        ...connection,
        id: remoteConnectionId(connection),
        password: "",
      }));

    return normalizedConnections.filter(
      (connection, index, connections) =>
        connections.findIndex((current) => current.id === connection.id) ===
        index,
    );
  } catch {
    return [];
  }
}

export function writeSavedRemoteConnections(connections: SavedRemoteConnection[]) {
  try {
    window.localStorage.setItem(
      SAVED_REMOTE_CONNECTIONS_KEY,
      JSON.stringify({
        version: SAVED_REMOTE_CONNECTIONS_VERSION,
        connections,
      }),
    );
  } catch {
    // Best effort cache.
  }
}

function upsertSavedRemoteConnection(
  connections: SavedRemoteConnection[],
  config: RemoteWorkspaceConfig,
) {
  const id = remoteConnectionId(config);
  const savedConnection: SavedRemoteConnection = {
    ...config,
    password: "",
    id,
    savedAt: new Date().toISOString(),
  };
  const nextConnections = [
    savedConnection,
    ...connections.filter((connection) => connection.id !== id),
  ].slice(0, 12);

  writeSavedRemoteConnections(nextConnections);
  return nextConnections;
}

function hasConnectionAuthentication(connection: RemoteConnectionDraft) {
  return connection.authMethod === "password"
    ? connection.password.trim().length > 0
    : connection.sshKeyPath.trim().length > 0;
}

function hasRequiredConnectionFields(connection: RemoteConnectionDraft) {
  return (
    connection.name.trim().length > 0 &&
    connection.host.trim().length > 0 &&
    connection.port.trim().length > 0 &&
    connection.username.trim().length > 0
  );
}

function latencyTone(latency?: number) {
  if (latency === undefined) return "text-gray-500";
  if (latency <= 80) return "text-green-400";
  if (latency <= 180) return "text-yellow-300";
  return "text-red-300";
}

export function WorkspaceSelector({
  onSelectLocal,
  onSelectRemote,
  initialError,
}: WorkspaceSelectorProps) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"choose" | "remote">("choose");

  if (mode === "remote") {
    return (
      <RemoteWorkspaceSetup
        onBack={() => setMode("choose")}
        onConnected={onSelectRemote}
      />
    );
  }

  return (
    <main className="flex min-h-screen bg-[#22272b] text-white">
      <section className="flex w-full flex-col justify-center px-6 py-10 sm:px-10 lg:px-16">
        <div className="mx-auto flex w-full max-w-5xl flex-col gap-10">
          <div className="max-w-2xl">
            <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl border border-orange-400/20 bg-orange-500/10 text-orange-300 shadow-[0_0_24px_rgba(249,115,22,0.12)]">
              <Server size={28} />
            </div>
            <p className="text-xs font-black uppercase tracking-[0.24em] text-orange-300">
              PZ Manager 0.4.0
            </p>
            <h1 className="mt-3 text-4xl font-black tracking-tight text-white sm:text-5xl">
              {t("workspaceSelector.title")}
            </h1>
            <p className="mt-4 max-w-xl text-base leading-7 text-gray-400">
              {t("workspaceSelector.subtitle")}
            </p>
            {initialError && (
              <div className="mt-5 rounded-2xl border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-300">
                {initialError}
              </div>
            )}
          </div>

          <div className="grid gap-5 lg:grid-cols-2">
            <button
              type="button"
              onClick={() => {
                window.localStorage.setItem(LAST_WORKSPACE_KEY, "local");
                onSelectLocal();
              }}
              className="group min-h-[260px] rounded-[8px] border border-white/10 bg-[#2b3238] p-7 text-left transition-all hover:border-orange-400/40 hover:bg-[#333b42] hover:shadow-[0_0_24px_rgba(249,115,22,0.08)]"
            >
              <div className="flex items-start justify-between gap-6">
                <div className="flex h-12 w-12 items-center justify-center rounded-[8px] bg-[#1e2327] text-orange-300 transition-colors group-hover:bg-orange-500 group-hover:text-white">
                  <Folder size={24} />
                </div>
                <span className="rounded-full border border-green-400/20 bg-green-500/10 px-3 py-1 text-[10px] font-black uppercase tracking-widest text-green-300">
                  {t("workspaceSelector.readyBadge")}
                </span>
              </div>
              <h2 className="mt-8 text-2xl font-black tracking-tight">
                {t("workspaceSelector.localWorkspaceTitle")}
              </h2>
              <p className="mt-3 text-sm leading-6 text-gray-400">
                {t("workspaceSelector.localWorkspaceDesc")}
              </p>
              <div className="mt-7 flex items-center gap-2 text-sm font-bold text-orange-300">
                <CheckCircle2 size={17} />
                {t("workspaceSelector.localWorkspaceFooter")}
              </div>
            </button>

            <button
              type="button"
              onClick={() => setMode("remote")}
              className="group min-h-[260px] rounded-[8px] border border-white/10 bg-[#2b3238] p-7 text-left transition-all hover:border-cyan-300/40 hover:bg-[#303941] hover:shadow-[0_0_24px_rgba(34,211,238,0.08)]"
            >
              <div className="flex items-start justify-between gap-6">
                <div className="flex h-12 w-12 items-center justify-center rounded-[8px] bg-[#1e2327] text-cyan-300 transition-colors group-hover:bg-cyan-500 group-hover:text-white">
                  <Network size={24} />
                </div>
                <span className="rounded-full border border-cyan-300/20 bg-cyan-500/10 px-3 py-1 text-[10px] font-black uppercase tracking-widest text-cyan-200">
                  {t("workspaceSelector.sshBadge")}
                </span>
              </div>
              <h2 className="mt-8 text-2xl font-black tracking-tight">
                {t("workspaceSelector.remoteWorkspaceTitle")}
              </h2>
              <p className="mt-3 text-sm leading-6 text-gray-400">
                {t("workspaceSelector.remoteWorkspaceDesc")}
              </p>
              <div className="mt-7 flex items-center gap-2 text-sm font-bold text-cyan-200">
                <Wifi size={17} />
                {t("workspaceSelector.remoteWorkspaceFooter")}
              </div>
            </button>
          </div>
        </div>
      </section>
    </main>
  );
}

function RemoteWorkspaceSetup({
  onBack,
  onConnected,
}: {
  onBack: () => void;
  onConnected: (connection: RemoteConnectionDraft) => void;
}) {
  const { t } = useTranslation();
  const [connection, setConnection] = useState(initialRemoteConnection);
  const [savedConnections, setSavedConnections] = useState<
    SavedRemoteConnection[]
  >(() => readSavedRemoteConnections());
  const [status, setStatus] = useState<
    "idle" | "connecting" | "connected" | "error"
  >("idle");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [isSshHelpOpen, setIsSshHelpOpen] = useState(false);
  const [publicKeyContent, setPublicKeyContent] = useState("");
  const [publicKeyError, setPublicKeyError] = useState<string | null>(null);
  const [isPublicKeyModalOpen, setIsPublicKeyModalOpen] = useState(false);
  const [isGeneratingPublicKey, setIsGeneratingPublicKey] = useState(false);
  const [isFixingSshKeyPermissions, setIsFixingSshKeyPermissions] =
    useState(false);
  const hasUserEditedConnection = useRef(false);
  const deletedConnectionIdsRef = useRef(new Set<string>());
  const hasSshClient = true;
  const hasAuthentication = hasConnectionAuthentication(connection);
  const canConnect =
    hasSshClient &&
    hasRequiredConnectionFields(connection) &&
    hasAuthentication;
  const canSaveConnection =
    hasSshClient &&
    hasRequiredConnectionFields(connection) &&
    hasAuthentication;

  const [searchQuery, setSearchQuery] = useState("");
  const [connectionStatuses, setConnectionStatuses] = useState<
    Record<
      string,
      {
        status: "idle" | "checking" | "online" | "offline";
        latency?: number;
        error?: string;
      }
    >
  >({});

  const displayFeedback = useMemo(() => {
    if (!feedback) return "";
    return feedback.split("\n\n")[0].trim();
  }, [feedback]);
  const hasSshKeyPermissionError = Boolean(
    feedback &&
    /private key.*permissions|UNPROTECTED PRIVATE KEY FILE|bad permissions|Permissions for/i.test(
      feedback,
    ) &&
    connection.authMethod === "key" &&
    connection.sshKeyPath.trim(),
  );

  useEffect(() => {
    let isMounted = true;

    void invokeTauri<RemoteWorkspaceConfig | null>(
      "get_remote_workspace_config",
    )
      .then((config) => {
        if (!isMounted || !config) return;

        const configConnection = remoteConfigToDraft(config);
        const configConnectionId = remoteConnectionId(configConnection);
        if (deletedConnectionIdsRef.current.has(configConnectionId)) return;

        const savedConfig = defaultRemoteConfig(configConnection, config);

        if (!hasUserEditedConnection.current) {
          setConnection(configConnection);
        }
        setSavedConnections((currentConnections) =>
          currentConnections.some(
            (current) => current.id === remoteConnectionId(configConnection),
          )
            ? currentConnections
            : upsertSavedRemoteConnection(currentConnections, savedConfig),
        );
      })
      .catch((error) => {
        if (!isMounted) return;
        setStatus("error");
        setFeedback(formatConnectionError(getErrorMessage(error)));
      });

    return () => {
      isMounted = false;
    };
  }, []);

  useEffect(() => {
    setConnectionStatuses((currentStatuses) => {
      const savedIds = new Set(
        savedConnections.map((connection) => connection.id),
      );
      return Object.fromEntries(
        Object.entries(currentStatuses).filter(([connectionId]) =>
          savedIds.has(connectionId),
        ),
      );
    });
  }, [savedConnections]);

  const filteredConnections = useMemo(() => {
    if (!searchQuery.trim()) return savedConnections;
    const query = searchQuery.toLowerCase();
    return savedConnections.filter(
      (conn) =>
        conn.name.toLowerCase().includes(query) ||
        conn.host.toLowerCase().includes(query) ||
        conn.username.toLowerCase().includes(query),
    );
  }, [savedConnections, searchQuery]);

  function updateField<K extends keyof RemoteConnectionDraft>(
    field: K,
    value: RemoteConnectionDraft[K],
  ) {
    hasUserEditedConnection.current = true;
    setConnection((current) => ({ ...current, [field]: value }));
    setStatus("idle");
    setFeedback(null);
    setPublicKeyContent("");
    setPublicKeyError(null);
    setIsPublicKeyModalOpen(false);
  }

  function useSavedConnection(savedConnection: SavedRemoteConnection) {
    hasUserEditedConnection.current = false;
    const nextConnection = remoteConfigToDraft(savedConnection);

    setConnection(nextConnection);
    setStatus("idle");
    setFeedback(null);

    if (nextConnection.authMethod === "password" && !nextConnection.password) {
      setFeedback(
        "Enter the password for this saved connection before connecting.",
      );
      return;
    }

    void connectRemote(nextConnection, savedConnection);
  }

  function loadSavedConnectionForEdit(savedConnection: SavedRemoteConnection) {
    hasUserEditedConnection.current = false;
    const nextConnection = remoteConfigToDraft(savedConnection);
    setConnection(nextConnection);
    setStatus("idle");
    setFeedback(null);
  }

  async function testSavedConnectionLatency(
    savedConnection: SavedRemoteConnection,
  ) {
    setConnectionStatuses((currentStatuses) => ({
      ...currentStatuses,
      [savedConnection.id]: { status: "checking" },
    }));

    try {
      const result = await invokeTauri<RemoteServerLatencyResult>(
        "test_remote_server_latency",
        {
          connection: remoteConfigToDraft(savedConnection),
        },
      );

      setConnectionStatuses((currentStatuses) => ({
        ...currentStatuses,
        [savedConnection.id]: result.success
          ? { status: "online", latency: result.latencyMs }
          : { status: "offline", error: result.error ?? "Offline" },
      }));
    } catch (error) {
      setConnectionStatuses((currentStatuses) => ({
        ...currentStatuses,
        [savedConnection.id]: {
          status: "offline",
          error: getErrorMessage(error),
        },
      }));
    }
  }

  function startNewConnection() {
    hasUserEditedConnection.current = true;
    setConnection(initialRemoteConnection);
    setStatus("idle");
    setFeedback(null);
    setPublicKeyContent("");
    setPublicKeyError(null);
    setIsPublicKeyModalOpen(false);
  }

  function removeSavedConnection(connectionId: string) {
    deletedConnectionIdsRef.current.add(connectionId);
    const connectionToDelete = savedConnections.find(
      (savedConnection) => savedConnection.id === connectionId,
    );

    setSavedConnections((currentConnections) => {
      const nextConnections = currentConnections.filter(
        (savedConnection) => savedConnection.id !== connectionId,
      );

      writeSavedRemoteConnections(nextConnections);
      return nextConnections;
    });

    if (window.localStorage.getItem(LAST_WORKSPACE_KEY) === `remote:${connectionId}`) {
      window.localStorage.removeItem(LAST_WORKSPACE_KEY);
    }

    if (connectionToDelete) {
      void invokeTauri("delete_remote_workspace_config", {
        connection: remoteConfigToDraft(connectionToDelete),
      }).catch(() => {
        // Best effort: the local saved connection is already removed.
      });
    }
  }

  async function selectSshKeyFile() {
    setFeedback(null);
    setPublicKeyError(null);
    setPublicKeyContent("");
    setIsPublicKeyModalOpen(false);

    try {
      const selectedPath = await invokeTauri<string | null>(
        "select_ssh_key_file",
      );

      if (selectedPath) {
        updateField("sshKeyPath", selectedPath);
      }
    } catch (error) {
      setStatus("error");
      setFeedback(formatConnectionError(getErrorMessage(error)));
    }
  }

  async function saveCurrentConnection() {
    if (!canSaveConnection) return;

    setStatus("idle");
    setFeedback(null);

    try {
      const savedConfig = await invokeTauri<RemoteWorkspaceConfig | null>(
        "get_remote_workspace_config",
        { connection },
      );
      const configToSave = defaultRemoteConfig(
        connection,
        savedConfig ?? undefined,
      );
      const persistedConfig = await invokeTauri<RemoteWorkspaceConfig>(
        "save_remote_workspace_config",
        {
          config: {
            ...configToSave,
            name: connection.name,
            host: connection.host,
            port: connection.port,
            username: connection.username,
            authMethod: connection.authMethod,
            sshKeyPath:
              connection.authMethod === "key" ? connection.sshKeyPath : "",
            serverPath: connection.serverPath,
          },
        },
      );

      const nextConnections = upsertSavedRemoteConnection(savedConnections, persistedConfig);
      setSavedConnections(nextConnections);
      setConnection(remoteConfigToDraft(persistedConfig));
      hasUserEditedConnection.current = false;
      setFeedback("Connection profile saved.");
    } catch (saveError) {
      setStatus("error");
      setFeedback(getErrorMessage(saveError));
    }
  }
  async function generatePublicKey() {
    const keyPath = connection.sshKeyPath.trim();
    if (!keyPath) {
      setPublicKeyError("Select a private key file first.");
      return;
    }

    setPublicKeyError(null);
    setPublicKeyContent("");
    setIsPublicKeyModalOpen(false);
    setIsGeneratingPublicKey(true);

    try {
      const publicKey = await invokeTauri<string>("generate_ssh_public_key", {
        sshKeyPath: keyPath,
      });
      setPublicKeyContent(publicKey);
      setIsPublicKeyModalOpen(true);
    } catch (error) {
      setPublicKeyError(getErrorMessage(error));
    } finally {
      setIsGeneratingPublicKey(false);
    }
  }

  async function fixSshKeyPermissionsAndRetry() {
    const keyPath = connection.sshKeyPath.trim();
    if (!keyPath || isFixingSshKeyPermissions) return;

    setIsFixingSshKeyPermissions(true);
    setFeedback("Fixing SSH key permissions...");

    try {
      const message = await invokeTauri<string>("fix_ssh_key_permissions", {
        sshKeyPath: keyPath,
      });
      setFeedback(`${message} Trying the SSH connection again...`);
      await connectRemote(connection);
    } catch (error) {
      setStatus("error");
      setFeedback(formatConnectionError(getErrorMessage(error)));
    } finally {
      setIsFixingSshKeyPermissions(false);
    }
  }

  async function connectRemote(
    connectionToUse: RemoteConnectionDraft = connection,
    savedConfigToUse?: SavedRemoteConnection,
  ) {
    if (
      !hasSshClient ||
      !hasRequiredConnectionFields(connectionToUse) ||
      !hasConnectionAuthentication(connectionToUse)
    )
      return;

    setStatus("connecting");
    setFeedback(
      `Testing SSH connection to ${connectionToUse.username}@${connectionToUse.host}:${connectionToUse.port}...`,
    );

    try {
      const result = await invokeTauri<RemoteServerConnectionResult>(
        "test_remote_server_connection",
        {
          connection: connectionToUse,
        },
      );
      const connectedConnection = {
        ...connectionToUse,
        host: result.host,
        port: String(result.port),
        serverPath: result.serverPath,
      };

      const savedConfig = await invokeTauri<RemoteWorkspaceConfig | null>(
        "get_remote_workspace_config",
        { connection: connectedConnection },
      );
      const mergedExistingConfig = {
        ...(savedConfigToUse ?? {}),
        ...(savedConfig ?? {}),
      };
      const configToSave = defaultRemoteConfig(
        connectedConnection,
        mergedExistingConfig,
      );

      const persistedConfig = await invokeTauri<RemoteWorkspaceConfig>(
        "save_remote_workspace_config",
        {
          config: {
            ...configToSave,
            name: result.name,
            host: result.host,
            port: String(result.port),
            username: connectedConnection.username,
            authMethod: connectedConnection.authMethod,
            sshKeyPath:
              connectedConnection.authMethod === "key"
                ? connectedConnection.sshKeyPath
                : "",
            serverPath: configToSave.serverPath || result.serverPath,
          },
        },
      );
      const persistedConnection = remoteConfigToDraft(persistedConfig);
      const nextConnections = upsertSavedRemoteConnection(savedConnections, persistedConfig);
      setSavedConnections(nextConnections);
      setConnection(persistedConnection);
      setConnectionStatuses((currentStatuses) => ({
        ...currentStatuses,
        [remoteConnectionId(persistedConfig)]: {
          status: "online",
          latency: result.latencyMs,
        },
      }));
      window.localStorage.setItem(
        LAST_WORKSPACE_KEY,
        `remote:${remoteConnectionId(persistedConfig)}`,
      );
      onConnected(persistedConnection);
    } catch (error) {
      setStatus("error");
      setFeedback(formatConnectionError(getErrorMessage(error)));
    }
  }

  return (
    <main className="flex min-h-screen flex-col bg-[#1c2024] text-white">
      {/* Top Header */}
      <header className="border-b border-white/5 bg-[#22272b]/50 backdrop-blur-md px-6 py-4 sticky top-0 z-10">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            <button
              type="button"
              onClick={onBack}
              className="flex h-9 w-9 items-center justify-center rounded-lg border border-white/10 text-gray-400 transition-all hover:bg-white/5 hover:text-white"
            >
              <ArrowLeft size={18} />
            </button>
            <div>
              <div className="flex items-center gap-2">
                <span className="text-[10px] font-black uppercase tracking-[0.2em] text-cyan-400">
                  {t("workspaceSelector.remoteWorkspaceTitle")}
                </span>
                <span className="h-1 w-1 rounded-full bg-cyan-400 animate-pulse"></span>
              </div>
              <h1 className="text-xl font-black tracking-tight text-white sm:text-2xl">
                {t("workspaceSelector.connectTitle")}
              </h1>
            </div>
          </div>

          <div className="hidden sm:flex items-center gap-3">
            <span
              className={`flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-black uppercase tracking-wider ${
                hasSshClient
                  ? "border-green-500/20 bg-green-500/10 text-green-400"
                  : "border-red-500/20 bg-red-500/10 text-red-400"
              }`}
            >
              <span
                className={`h-1.5 w-1.5 rounded-full ${hasSshClient ? "bg-green-400" : "bg-red-400 animate-ping"}`}
              ></span>
              {hasSshClient ? "SSH command" : "SSH command"}
            </span>
          </div>
        </div>
      </header>

      {/* Main Grid Content */}
      <section className="flex-grow px-6 py-8 mx-auto w-full max-w-7xl">
        <div className="grid gap-8 lg:grid-cols-[420px_1fr]">
          {/* Left Column: Saved Profiles list */}
          <div className="flex flex-col rounded-xl border border-white/10 bg-[#22272b]/80 backdrop-blur-md overflow-hidden shadow-[0_4px_30px_rgba(0,0,0,0.2)]">
            <div className="border-b border-white/5 bg-[#2b3238]/45 p-5">
              <div className="flex items-center justify-between gap-3 mb-4">
                <div className="flex items-center gap-2">
                  <Server size={18} className="text-cyan-400" />
                  <h2 className="text-sm font-bold text-white">
                    {t("workspaceSelector.savedConnectionsTitle")}
                  </h2>
                </div>
                <button
                  type="button"
                  onClick={startNewConnection}
                  className="flex items-center gap-1.5 rounded-lg bg-cyan-500/10 border border-cyan-500/20 px-2.5 py-1.5 text-xs font-bold text-cyan-300 transition-all hover:bg-cyan-500/20 hover:text-white"
                >
                  <Plus size={14} />
                  {t("workspaceSelector.newProfileBtn")}
                </button>
              </div>

              {/* Search Bar */}
              <div className="relative">
                <Search className="absolute left-3 top-2.5 h-4 w-4 text-gray-500" />
                <input
                  type="text"
                  placeholder={t("workspaceSelector.searchPlaceholder")}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full rounded-lg border border-white/5 bg-[#161a1d] pl-9 pr-4 py-2 text-sm text-white placeholder:text-gray-600 focus:border-cyan-500/50 focus:outline-none focus:ring-1 focus:ring-cyan-500/30"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery("")}
                    className="absolute right-2.5 top-2.5 text-gray-500 hover:text-white"
                  >
                    <X size={14} />
                  </button>
                )}
              </div>
            </div>

            {/* Saved connections scroll area */}
            <div className="flex-1 overflow-y-auto max-h-[520px] p-4 space-y-3">
              {filteredConnections.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                  <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-[#1e2327] text-gray-600 border border-white/5">
                    <Network size={20} />
                  </div>
                  <p className="text-sm font-bold text-gray-400">
                    {searchQuery
                      ? t("workspaceSelector.noMatchingProfiles")
                      : t("workspaceSelector.noSavedProfiles")}
                  </p>
                  <p className="mt-1 text-xs text-gray-600 max-w-[240px]">
                    {searchQuery
                      ? t("workspaceSelector.searchHint")
                      : t("workspaceSelector.noSavedProfilesHint")}
                  </p>
                </div>
              ) : (
                filteredConnections.map((savedConnection) => {
                  const isSelected =
                    remoteConnectionId(connection) === savedConnection.id;
                  const statusInfo = connectionStatuses[savedConnection.id] || {
                    status: "idle",
                  };
                  const canQuickConnect =
                    savedConnection.authMethod === "key" &&
                    savedConnection.sshKeyPath.trim().length > 0;

                  return (
                    <div
                      key={savedConnection.id}
                      onClick={() =>
                        loadSavedConnectionForEdit(savedConnection)
                      }
                      className={`group relative flex flex-col rounded-xl border p-4 cursor-pointer transition-all hover:translate-y-[-2px] ${
                        isSelected
                          ? "border-cyan-500 bg-cyan-500/10 shadow-[0_0_15px_rgba(6,182,212,0.15)]"
                          : "border-white/5 bg-[#1b1f22] hover:border-white/15 hover:bg-[#202529]"
                      }`}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <span className="truncate text-sm font-bold text-white group-hover:text-cyan-300 transition-colors">
                              {savedConnection.name}
                            </span>
                            <span
                              className={`shrink-0 rounded-full border px-2 py-0.5 text-[9px] font-black uppercase tracking-wider ${
                                canQuickConnect
                                  ? "border-green-400/20 bg-green-500/10 text-green-300"
                                  : "border-yellow-400/20 bg-yellow-500/10 text-yellow-200"
                              }`}
                            >
                              {canQuickConnect
                                ? t("workspaceSelector.quickConnectAuthMethodKey")
                                : t("workspaceSelector.quickConnectAuthMethodPassword")}
                            </span>
                          </div>

                          <p className="truncate text-xs text-gray-400">
                            {savedConnection.username}@{savedConnection.host}:
                            {savedConnection.port}
                          </p>
                        </div>

                        {/* Latency Indicator */}
                        <div
                          className="flex items-center gap-1.5 shrink-0 bg-[#161a1d] px-2 py-1 rounded-md border border-white/5"
                          title={
                            statusInfo.status === "offline"
                              ? statusInfo.error
                              : undefined
                          }
                        >
                          {statusInfo.status === "idle" ? (
                            <>
                              <span className="text-[10px] text-gray-500 font-medium font-mono">
                                {t("workspaceSelector.testBtn")}
                              </span>
                            </>
                          ) : statusInfo.status === "checking" ? (
                            <>
                              <RefreshCw
                                size={10}
                                className="text-cyan-400 animate-spin"
                              />
                              <span className="text-[10px] text-gray-500 font-medium font-mono">
                                {t("workspaceSelector.testBtn")}
                              </span>
                            </>
                          ) : statusInfo.status === "offline" ? (
                            <>
                              <span className="relative flex h-1.5 w-1.5">
                                <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-red-500"></span>
                              </span>
                              <span className="text-[10px] text-red-300 font-bold font-mono">
                                {t("workspaceSelector.offStatus")}
                              </span>
                            </>
                          ) : (
                            <>
                              <span className="relative flex h-1.5 w-1.5">
                                <span className="absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-40"></span>
                                <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-green-500"></span>
                              </span>
                              <span
                                className={`text-[10px] font-bold font-mono ${latencyTone(statusInfo.latency)}`}
                              >
                                {statusInfo.latency ?? "-"}ms
                              </span>
                            </>
                          )}
                        </div>
                      </div>

                      {/* Floating actions on card hover */}
                      <div className="absolute right-3 bottom-3 flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity bg-inherit pl-2 rounded-md">
                        <button
                          type="button"
                          title={t("workspaceSelector.latencyTooltip")}
                          onClick={(e) => {
                            e.stopPropagation();
                            void testSavedConnectionLatency(savedConnection);
                          }}
                          className="flex h-7 w-7 items-center justify-center rounded-md bg-cyan-500/10 border border-cyan-500/20 text-cyan-300 hover:bg-cyan-500 hover:text-white transition-all shadow-sm"
                        >
                          <RefreshCw size={12} />
                        </button>
                        <button
                          type="button"
                          title={t("workspaceSelector.quickConnectTooltip")}
                          onClick={(e) => {
                            e.stopPropagation();
                            useSavedConnection(savedConnection);
                          }}
                          className="flex h-7 w-7 items-center justify-center rounded-md bg-green-500/20 border border-green-500/30 text-green-400 hover:bg-green-500 hover:text-white transition-all shadow-sm"
                        >
                          <Play size={12} fill="currentColor" />
                        </button>
                        <button
                          type="button"
                          title={t("workspaceSelector.removeProfileTooltip")}
                          onClick={(e) => {
                            e.stopPropagation();
                            removeSavedConnection(savedConnection.id);
                          }}
                          className="flex h-7 w-7 items-center justify-center rounded-md bg-red-500/10 border border-red-500/20 text-red-400 hover:bg-red-500 hover:text-white transition-all shadow-sm"
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </div>

          {/* Right Column: Connection details form */}
          <div className="rounded-xl border border-white/10 bg-[#22272b]/80 backdrop-blur-md p-6 sm:p-8 flex flex-col justify-between shadow-[0_4px_30px_rgba(0,0,0,0.2)]">
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void connectRemote();
              }}
              className="space-y-6"
            >
              <div className="flex items-start justify-between gap-6 border-b border-white/5 pb-4">
                <div>
                  <h2 className="text-lg font-bold text-white flex items-center gap-2">
                    <MonitorCog size={18} className="text-cyan-400" />
                    {t("workspaceSelector.connectionDetailsTitle")}
                  </h2>
                  <p className="mt-1 text-xs text-gray-400">
                    {remoteConnectionId(connection) &&
                    savedConnections.some(
                      (c) => c.id === remoteConnectionId(connection),
                    )
                      ? t("workspaceSelector.savedProfileNotice")
                      : t("workspaceSelector.newProfileNotice")}
                  </p>
                </div>
              </div>

              {/* Form Input fields */}
              <div className="grid gap-5 md:grid-cols-2">
                <RemoteInput
                  label={t("workspaceSelector.fieldProfileName")}
                  value={connection.name}
                  placeholder="e.g. Server 1"
                  onChange={(value) => updateField("name", value)}
                />
                <RemoteInput
                  label={t("workspaceSelector.fieldHost")}
                  value={connection.host}
                  placeholder="e.g. 192.168.1.100"
                  onChange={(value) => updateField("host", value)}
                />
                <RemoteInput
                  label={t("workspaceSelector.fieldPort")}
                  value={connection.port}
                  placeholder="22"
                  onChange={(value) => updateField("port", value)}
                />
                <RemoteInput
                  label={t("workspaceSelector.fieldUsername")}
                  value={connection.username}
                  placeholder="e.g. ubuntu"
                  onChange={(value) => updateField("username", value)}
                />
              </div>

              {/* SSH Authentication Section */}
              <div className="rounded-xl border border-white/5 bg-[#1b1f22] p-5">
                <div className="flex items-center justify-between gap-3 mb-3">
                  <h3 className="text-xs font-black uppercase tracking-[0.2em] text-cyan-400 flex items-center gap-1.5">
                    <KeyRound size={13} />
                    {t("workspaceSelector.authStrategyTitle")}
                  </h3>
                  <button
                    type="button"
                    onClick={() => setIsSshHelpOpen(true)}
                    className="flex items-center gap-1 text-xs font-bold text-cyan-400 hover:text-cyan-300 transition-colors"
                  >
                    <HelpCircle size={14} />
                    {t("workspaceSelector.helpSshBtn")}
                  </button>
                </div>

                <div className="grid gap-3 sm:grid-cols-2">
                  <button
                    type="button"
                    onClick={() => updateField("authMethod", "password")}
                    className={`flex items-center gap-3 rounded-lg border px-4 py-3 text-left transition-all ${
                      connection.authMethod === "password"
                        ? "border-cyan-500 bg-cyan-500/10 text-white shadow-sm"
                        : "border-white/5 bg-[#22272b] text-gray-400 hover:border-white/10 hover:text-white"
                    }`}
                  >
                    <Lock
                      size={16}
                      className={
                        connection.authMethod === "password"
                          ? "text-cyan-400"
                          : ""
                      }
                    />
                    <span className="text-sm font-semibold">{t("workspaceSelector.authMethodPassword")}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => updateField("authMethod", "key")}
                    className={`flex items-center gap-3 rounded-lg border px-4 py-3 text-left transition-all ${
                      connection.authMethod === "key"
                        ? "border-cyan-500 bg-cyan-500/10 text-white shadow-sm"
                        : "border-white/5 bg-[#22272b] text-gray-400 hover:border-white/10 hover:text-white"
                    }`}
                  >
                    <FileKey2
                      size={16}
                      className={
                        connection.authMethod === "key" ? "text-cyan-400" : ""
                      }
                    />
                    <span className="text-sm font-semibold">
                      {t("workspaceSelector.authMethodKey")}
                    </span>
                  </button>
                </div>

                {connection.authMethod === "password" ? (
                  <div className="mt-4">
                    <RemoteInput
                      label={t("workspaceSelector.fieldPassword")}
                      type="password"
                      value={connection.password}
                      placeholder="Password for the user account"
                      onChange={(value) => updateField("password", value)}
                    />
                  </div>
                ) : (
                  <div className="mt-4 grid gap-3 sm:grid-cols-[1fr_auto_auto] sm:items-end">
                    <div className="flex-1">
                      <RemoteInput
                        label={t("workspaceSelector.fieldKeyPath")}
                        value={connection.sshKeyPath}
                        placeholder="~/.ssh/id_ed25519"
                        onChange={(value) => updateField("sshKeyPath", value)}
                      />
                    </div>
                    <button
                      type="button"
                      onClick={() => void selectSshKeyFile()}
                      className="flex h-[44px] items-center justify-center gap-2 rounded-lg border border-white/10 px-4 text-xs font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white shrink-0 mb-0.5"
                    >
                      <Folder size={14} />
                      {t("workspaceSelector.chooseFileBtn")}
                    </button>
                    <button
                      type="button"
                      onClick={() => void generatePublicKey()}
                      disabled={
                        !connection.sshKeyPath.trim() || isGeneratingPublicKey
                      }
                      className="flex h-[44px] items-center justify-center gap-2 rounded-lg border border-cyan-400/25 bg-cyan-400/10 px-4 text-xs font-bold text-cyan-200 transition-colors hover:bg-cyan-400 hover:text-[#071014] disabled:cursor-not-allowed disabled:opacity-50 shrink-0 mb-0.5"
                    >
                      {isGeneratingPublicKey ? (
                        <RefreshCw size={14} className="animate-spin" />
                      ) : (
                        <KeyRound size={14} />
                      )}
                      {t("workspaceSelector.publicKeyBtn")}
                    </button>
                    {publicKeyError && (
                      <p className="sm:col-span-3 break-words rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 font-mono text-xs text-red-300">
                        {publicKeyError}
                      </p>
                    )}
                  </div>
                )}
              </div>

              {/* Feedback Messages */}
              {status === "error" && displayFeedback && (
                <div className="rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-xs font-medium text-red-400 flex items-start gap-2.5">
                  <ShieldAlert size={16} className="shrink-0 mt-0.5" />
                  <div className="min-w-0 flex-1 space-y-3">
                    <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed">
                      {displayFeedback}
                    </pre>
                    {hasSshKeyPermissionError && (
                      <button
                        type="button"
                        onClick={() => void fixSshKeyPermissionsAndRetry()}
                        disabled={isFixingSshKeyPermissions}
                        className="inline-flex items-center gap-2 rounded-lg border border-orange-300/30 bg-orange-400/10 px-3 py-2 text-xs font-black text-orange-100 transition-colors hover:bg-orange-400 hover:text-[#1c2024] disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        {isFixingSshKeyPermissions ? (
                          <RefreshCw size={14} className="animate-spin" />
                        ) : (
                          <KeyRound size={14} />
                        )}
                        {t("workspaceSelector.fixPermissionsBtn")}
                      </button>
                    )}
                  </div>
                </div>
              )}

              {status === "connected" && feedback && (
                <div className="rounded-lg border border-green-500/20 bg-green-500/10 px-4 py-3 text-xs font-medium text-green-400 flex items-start gap-2.5">
                  <CheckCircle2 size={16} className="shrink-0 mt-0.5" />
                  <p className="leading-relaxed">{feedback}</p>
                </div>
              )}

              {/* Actions Footer */}
              <div className="flex flex-col-reverse gap-3 sm:flex-row sm:justify-end border-t border-white/5 pt-5">
                <button
                  type="button"
                  onClick={onBack}
                  className="rounded-lg border border-white/10 px-5 py-2.5 text-xs font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
                >
                  {t("common.cancel")}
                </button>
                <button
                  type="button"
                  disabled={!canSaveConnection || status === "connecting"}
                  onClick={() => void saveCurrentConnection()}
                  className="flex items-center justify-center gap-2 rounded-lg border border-white/10 px-5 py-2.5 text-xs font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <Save size={14} />
                  {t("workspaceSelector.saveProfileBtn")}
                </button>
                <button
                  type="submit"
                  disabled={!canConnect || status === "connecting"}
                  className="flex items-center justify-center gap-2 rounded-lg bg-cyan-500 px-6 py-2.5 text-xs font-black text-white transition-all hover:bg-cyan-400 hover:shadow-[0_0_12px_rgba(6,182,212,0.3)] disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500 disabled:shadow-none"
                >
                  {status === "connecting" ? (
                    <>
                      <RefreshCw size={14} className="animate-spin" />
                      {t("workspaceSelector.testingConnectionBtn")}
                    </>
                  ) : (
                    <>
                      <KeyRound size={14} />
                      {t("workspaceSelector.connectServerBtn")}
                    </>
                  )}
                </button>
              </div>
            </form>
          </div>
        </div>
      </section>

      {isSshHelpOpen && (
        <SshHelpModal onClose={() => setIsSshHelpOpen(false)} />
      )}

      {isPublicKeyModalOpen && publicKeyContent && (
        <PublicKeyModal
          publicKey={publicKeyContent}
          onClose={() => setIsPublicKeyModalOpen(false)}
        />
      )}
    </main>
  );
}

function formatConnectionError(message: string) {
  return message.split("\n\n[COMMAND]")[0].split("\n[COMMAND]")[0].trim();
}
function PublicKeyModal({
  publicKey,
  onClose,
}: {
  publicKey: string;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);

  async function copyPublicKey() {
    await navigator.clipboard.writeText(publicKey);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 p-4 backdrop-blur-md"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("workspaceSelector.modalPublicKeyTitle")}
        className="w-full max-w-2xl rounded-3xl border border-white/10 bg-[#22272b] p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4 border-b border-white/5 pb-4">
          <div className="flex items-center gap-3">
            <div className="rounded-xl border border-cyan-500/20 bg-cyan-500/10 p-2 text-cyan-300">
              <KeyRound size={22} />
            </div>
            <div>
              <h3 className="text-lg font-black text-white">
                {t("workspaceSelector.modalPublicKeyTitle")}
              </h3>
              <p className="mt-1 text-xs text-gray-400">
                {t("workspaceSelector.modalPublicKeySubtitle")}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-2 text-gray-500 transition-colors hover:bg-white/5 hover:text-white"
            aria-label="Close public key modal"
          >
            <X size={20} />
          </button>
        </div>

        <pre className="mt-4 max-h-64 overflow-auto whitespace-pre-wrap break-all rounded-xl border border-white/5 bg-[#161a1d] p-4 font-mono text-xs leading-relaxed text-cyan-100 custom-scrollbar">
          {publicKey}
        </pre>

        <div className="mt-5 flex flex-col-reverse gap-3 sm:flex-row sm:justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl border border-white/10 px-5 py-2.5 text-xs font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
          >
            {t("workspaceSelector.modalPublicKeyClose")}
          </button>
          <button
            type="button"
            onClick={() => void copyPublicKey()}
            className="flex items-center justify-center gap-2 rounded-xl bg-cyan-500 px-5 py-2.5 text-xs font-black text-white transition-all hover:bg-cyan-400 hover:shadow-[0_0_12px_rgba(6,182,212,0.3)]"
          >
            <Clipboard size={14} />
            {copied ? t("workspaceSelector.modalPublicKeyCopied") : t("workspaceSelector.modalPublicKeyCopy")}
          </button>
        </div>
      </div>
    </div>
  );
}

function RemoteInput({
  label,
  value,
  placeholder,
  type = "text",
  onChange,
}: {
  label: string;
  value: string;
  placeholder: string;
  type?: "text" | "password";
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-2">
      <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
        {label}
      </span>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="w-full rounded-[8px] border border-white/5 bg-[#1e2327] px-4 py-3 text-sm font-semibold text-white transition-all placeholder:text-gray-600 focus:border-cyan-300/50 focus:outline-none focus:ring-1 focus:ring-cyan-300/20"
      />
    </label>
  );
}

function CodeBlock({ code }: { code: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return (
    <div className="relative group mt-2 rounded-xl border border-white/5 bg-[#161a1d] px-4 py-3 font-mono text-xs text-cyan-300 whitespace-pre overflow-x-auto leading-relaxed custom-scrollbar">
      <code>{code}</code>
      <button
        type="button"
        onClick={copy}
        className="absolute right-2 top-2 rounded-md bg-white/5 border border-white/10 px-2 py-1 text-[10px] font-bold text-gray-400 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-white/10 hover:text-white transition-all"
      >
        {copied ? "Copied!" : "Copy"}
      </button>
    </div>
  );
}

function SshHelpModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4 backdrop-blur-md"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        className="w-full max-w-2xl rounded-3xl border border-white/10 bg-[#22272b] p-6 shadow-2xl flex flex-col max-h-[85vh]"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-white/5 pb-4 mb-4">
          <div className="flex items-center gap-3">
            <div className="rounded-xl border border-cyan-500/20 bg-cyan-500/10 p-2 text-cyan-300">
              <HelpCircle size={24} />
            </div>
            <div>
              <h3 className="text-xl font-black text-white">
                {t("workspaceSelector.sshHelpModalTitle")}
              </h3>
              <p className="text-xs text-gray-400">
                {t("workspaceSelector.helpSshIntro")}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-2 text-gray-500 transition-colors hover:bg-white/5 hover:text-white"
          >
            <X size={20} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto space-y-6 pr-1 custom-scrollbar text-sm leading-relaxed text-gray-300">
          <div>
            <h4 className="font-bold text-white flex items-center gap-2">
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-cyan-500/15 text-[11px] font-black text-cyan-300">
                1
              </span>
              {t("workspaceSelector.sshHelpStep1Title")}
            </h4>
            <p className="mt-2 text-xs text-gray-400">
              {t("workspaceSelector.sshHelpStep1Body")}
            </p>
            <CodeBlock
              code={
                "sudo apt-get update\nsudo apt-get install -y openssh-server\nsudo systemctl enable --now ssh"
              }
            />
          </div>

          <div>
            <h4 className="font-bold text-white flex items-center gap-2">
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-cyan-500/15 text-[11px] font-black text-cyan-300">
                2
              </span>
              {t("workspaceSelector.sshHelpStep2Title")}
            </h4>
            <p className="mt-2 text-xs text-gray-400">
              {t("workspaceSelector.sshHelpStep2Body")}
            </p>
            <CodeBlock
              code={"sudo usermod -aG sudo <SSH_USER>\nsudo -n true"}
            />
          </div>

          <div>
            <h4 className="font-bold text-white flex items-center gap-2">
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-cyan-500/15 text-[11px] font-black text-cyan-300">
                3
              </span>
              {t("workspaceSelector.sshHelpStep3Title")}
            </h4>
            <p className="mt-2 text-xs text-gray-400">
              {t("workspaceSelector.sshHelpStep3Body")}
            </p>
          </div>

          <div>
            <h4 className="font-bold text-white flex items-center gap-2">
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-cyan-500/15 text-[11px] font-black text-cyan-300">
                4
              </span>
              {t("workspaceSelector.sshHelpStep4Title")}
            </h4>
            <p className="mt-2 text-xs text-gray-400">
              {t("workspaceSelector.sshHelpStep4Body")}
            </p>
            <CodeBlock
              code={
                'mkdir -p ~/.ssh\nprintf "%s\\n" "<PASTE_PUBLIC_KEY_HERE>" >> ~/.ssh/authorized_keys\nchmod 700 ~/.ssh\nchmod 600 ~/.ssh/authorized_keys'
              }
            />
          </div>
        </div>
        <div className="border-t border-white/5 pt-4 mt-4 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl border border-white/10 px-5 py-2.5 text-xs font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
          >
            {t("workspaceSelector.sshHelpClose")}
          </button>
        </div>
      </div>
    </div>
  );
}
