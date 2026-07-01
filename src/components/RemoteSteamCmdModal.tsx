import { listen } from "@tauri-apps/api/event"
import { CheckCircle2, FileArchive, HardDriveDownload, Loader2, Save, UploadCloud, X } from "lucide-react"
import { useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import type { RemoteConnectionDraft, RemoteWorkspaceConfig } from "@/lib/commandRunner"
import { getErrorMessage } from "@/lib/errors"
import { invokeTauri } from "@/lib/tauri"

type RemoteSteamCmdUploadResult = {
  localPath: string
  remotePath: string
  steamcmdExecutablePath: string
  command: string
  exitCode: number | null
  success: boolean
  stdout: string
  stderr: string
}

type RemoteHelperSetupResult = {
  localPath: string
  remotePath: string
  command: string
  exitCode: number | null
  success: boolean
  stdout: string
  stderr: string
}

type RemoteZomboidServerInstallResult = {
  installDirectory: string
  serverExecutablePath: string
  command: string
  exitCode: number | null
  success: boolean
  stdout: string
  stderr: string
}

type RemoteSetupResult = RemoteHelperSetupResult | RemoteSteamCmdUploadResult | RemoteZomboidServerInstallResult

type RemoteSteamCmdModalProps = {
  connection: RemoteConnectionDraft
  isOpen: boolean
  onClose: () => void
}

type RemoteSetupLogEvent = {
  phase: "steamcmd" | "zomboid-server" | string
  stream: "stdout" | "stderr" | "info" | string
  line: string
}

type StepStatus = "idle" | "running" | "success" | "error"
type ZomboidSetupMode = "existing" | "download"
type ZomboidServerBranch = "default" | "unstable"

function remoteAppDataBase(_username: string) {
  return "/var/lib/pzmm"
}

function remoteSteamcmdDir(username: string) {
  return `${remoteAppDataBase(username)}/steamcmd`
}

function remoteHelperDir(username: string) {
  return "/opt/pzmm"
}

function remoteZomboidServerDir(username: string) {
  return `${remoteAppDataBase(username)}/zomboid-server`
}

function joinRemotePath(directory: string, fileName: string) {
  return `${directory.replace(/[\\/]+$/, "")}/${fileName}`
}

function parentRemotePath(path: string) {
  const normalized = path.replace(/\\/g, "/")
  const index = normalized.lastIndexOf("/")
  return index > 0 ? normalized.slice(0, index) : normalized
}

function isWindowsPath(path?: string) {
  const value = path?.trim() ?? ""
  return /^[a-zA-Z]:[\\/]/.test(value) || value.includes("\\")
}

function isAbsoluteLinuxPath(path?: string) {
  const value = path?.trim() ?? ""
  return value.startsWith("/") && !isWindowsPath(value)
}

function isLegacyPzManagerPath(path?: string) {
  return Boolean(path?.trim().replace(/\//g, "\\").toLowerCase().startsWith("c:\\pzmanager\\"))
}

function cleanRemoteLinuxPath(path?: string) {
  const value = path?.trim() ?? ""
  return value && !isLegacyPzManagerPath(value) && isAbsoluteLinuxPath(value) ? value : ""
}
export function RemoteSteamCmdModal({ connection, isOpen, onClose }: RemoteSteamCmdModalProps) {
  const { t } = useTranslation()
  const defaultSteamcmdDir = useMemo(() => remoteSteamcmdDir(connection.username), [connection.username])
  const defaultHelperDir = useMemo(() => remoteHelperDir(connection.username), [connection.username])
  const defaultZomboidServerDir = useMemo(() => remoteZomboidServerDir(connection.username), [connection.username])
  const [config, setConfig] = useState<RemoteWorkspaceConfig | null>(null)
  const [activeStep, setActiveStep] = useState(1)
  const [zomboidMode, setZomboidMode] = useState<ZomboidSetupMode>("existing")
  const [zomboidBranch, setZomboidBranch] = useState<ZomboidServerBranch>("default")
  const [steamcmdDir, setSteamcmdDir] = useState(defaultSteamcmdDir)
  const [steamcmdPath, setSteamcmdPath] = useState("")
  const [zomboidServerDir, setZomboidServerDir] = useState(defaultZomboidServerDir)
  const [zomboidServerPath, setZomboidServerPath] = useState("")
  const [steamcmdStatus, setSteamcmdStatus] = useState<StepStatus>("idle")
  const [helperStatus, setHelperStatus] = useState<StepStatus>("idle")
  const [zomboidStatus, setZomboidStatus] = useState<StepStatus>("idle")
  const [uploadResult, setUploadResult] = useState<RemoteSteamCmdUploadResult | null>(null)
  const [helperResult, setHelperResult] = useState<RemoteHelperSetupResult | null>(null)
  const [installResult, setInstallResult] = useState<RemoteZomboidServerInstallResult | null>(null)
  const [liveLogLines, setLiveLogLines] = useState<RemoteSetupLogEvent[]>([])
  const [helperStartedAt, setHelperStartedAt] = useState<number | null>(null)
  const [steamcmdStartedAt, setSteamcmdStartedAt] = useState<number | null>(null)
  const [zomboidStartedAt, setZomboidStartedAt] = useState<number | null>(null)
  const [helperElapsedSeconds, setHelperElapsedSeconds] = useState(0)
  const [steamcmdElapsedSeconds, setSteamcmdElapsedSeconds] = useState(0)
  const [zomboidElapsedSeconds, setZomboidElapsedSeconds] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const isRunning = helperStatus === "running" || steamcmdStatus === "running" || zomboidStatus === "running"
  const resolvedSteamcmdPath = steamcmdPath || "/usr/games/steamcmd"
  const resolvedZomboidServerPath = zomboidServerPath || joinRemotePath(zomboidServerDir, "start-server.sh")
  const selectedZomboidBranchCommand = zomboidBranch === "unstable"
    ? "app_update 380870 -beta unstable validate"
    : "app_update 380870 validate"
  const isSteamcmdDirValid = isAbsoluteLinuxPath(steamcmdDir)
  const isZomboidServerDirValid = isAbsoluteLinuxPath(zomboidServerDir)
  const isZomboidServerPathValid = isAbsoluteLinuxPath(resolvedZomboidServerPath)
  const canSetupHelper =
    connection.authMethod === "key" &&
    connection.sshKeyPath.trim().length > 0 &&
    !isRunning
  const canInstallSteamcmd =
    helperStatus === "success" &&
    connection.authMethod === "key" &&
    connection.sshKeyPath.trim().length > 0 &&
    steamcmdDir.trim().length > 0 &&
    isSteamcmdDirValid &&
    !isRunning
  const canSaveExistingZomboid =
    zomboidMode === "existing" &&
    zomboidServerDir.trim().length > 0 &&
    resolvedZomboidServerPath.trim().length > 0 &&
    isZomboidServerDirValid &&
    isZomboidServerPathValid &&
    !isRunning
  const canDownloadZomboid =
    steamcmdStatus === "success" &&
    zomboidModeRequiresSteamcmd(zomboidMode) &&
    resolvedSteamcmdPath.trim().length > 0 &&
    zomboidServerDir.trim().length > 0 &&
    isZomboidServerDirValid &&
    !isRunning

  useEffect(() => {
    if (!isOpen) return

    let isMounted = true

    setError(null)
    setHelperStatus("idle")
    setHelperResult(null)
    setHelperStartedAt(null)
    setHelperElapsedSeconds(0)
    setLiveLogLines([])
    void invokeTauri<RemoteWorkspaceConfig | null>("get_remote_workspace_config")
      .then((loadedConfig) => {
        if (!isMounted) return

        const nextConfig = loadedConfig ?? null
        const loadedSteamcmdDir = cleanRemoteLinuxPath(nextConfig?.remoteSteamcmdDir) || defaultSteamcmdDir
        const loadedSteamcmdPath = cleanRemoteLinuxPath(nextConfig?.remoteSteamcmdPath)
        const loadedZomboidServerDir = cleanRemoteLinuxPath(nextConfig?.remoteZomboidServerDir) || defaultZomboidServerDir
        const loadedZomboidServerPath = cleanRemoteLinuxPath(nextConfig?.remoteZomboidServerPath)

        setConfig(nextConfig)
        setSteamcmdDir(loadedSteamcmdDir)
        setSteamcmdPath(loadedSteamcmdPath)
        setZomboidServerDir(loadedZomboidServerDir)
        setZomboidServerPath(loadedZomboidServerPath)
        setSteamcmdStatus(loadedSteamcmdPath ? "success" : "idle")
        setZomboidStatus(loadedZomboidServerPath ? "success" : "idle")
        setActiveStep(1)
      })
      .catch((configError) => {
        if (!isMounted) return
        setError(getErrorMessage(configError))
      })

    return () => {
      isMounted = false
    }
  }, [defaultSteamcmdDir, defaultZomboidServerDir, isOpen])

  useEffect(() => {
    if (!isOpen) return

    let unlisten: (() => void) | null = null

    void listen<RemoteSetupLogEvent>("remote-setup-log", ({ payload }) => {
      setLiveLogLines((currentLines) => [...currentLines, payload].slice(-300))
    }).then((unsubscribe) => {
      unlisten = unsubscribe
    })

    return () => {
      unlisten?.()
    }
  }, [isOpen])

  useEffect(() => {
    if (helperStatus !== "running" || helperStartedAt === null) return

    const interval = window.setInterval(() => {
      setHelperElapsedSeconds(Math.max(0, Math.floor((Date.now() - helperStartedAt) / 1000)))
    }, 1000)

    return () => window.clearInterval(interval)
  }, [helperStartedAt, helperStatus])

  useEffect(() => {
    if (steamcmdStatus !== "running" || steamcmdStartedAt === null) return

    const interval = window.setInterval(() => {
      setSteamcmdElapsedSeconds(Math.max(0, Math.floor((Date.now() - steamcmdStartedAt) / 1000)))
    }, 1000)

    return () => window.clearInterval(interval)
  }, [steamcmdStartedAt, steamcmdStatus])

  useEffect(() => {
    if (zomboidStatus !== "running" || zomboidStartedAt === null) return

    const interval = window.setInterval(() => {
      setZomboidElapsedSeconds(Math.max(0, Math.floor((Date.now() - zomboidStartedAt) / 1000)))
    }, 1000)

    return () => window.clearInterval(interval)
  }, [zomboidStartedAt, zomboidStatus])

  if (!isOpen) return null

  async function setupHelperStep() {
    if (!canSetupHelper) return

    const startedAt = Date.now()
    setHelperStatus("running")
    setHelperStartedAt(startedAt)
    setHelperElapsedSeconds(0)
    setError(null)
    setHelperResult(null)
    setLiveLogLines((currentLines) => currentLines.filter((line) => line.phase !== "helper"))

    try {
      const result = await invokeTauri<RemoteHelperSetupResult>("setup_remote_helper", {
        connection,
      })

      setHelperResult(result)
      setHelperStatus(result.success ? "success" : "error")
      finishHelperTimer(startedAt)

      if (result.success) {
        setActiveStep(steamcmdStatus === "success" ? (zomboidStatus === "success" ? 4 : 3) : 2)
      } else {
        setError(formatSetupFailure(result, "Remote setup upload failed."))
      }
    } catch (setupError) {
      setHelperStatus("error")
      finishHelperTimer(startedAt)
      setError(getErrorMessage(setupError))
    }
  }

  async function installSteamcmdStep() {
    if (!canInstallSteamcmd) return

    const startedAt = Date.now()
    setSteamcmdStatus("running")
    setSteamcmdStartedAt(startedAt)
    setSteamcmdElapsedSeconds(0)
    setError(null)
    setUploadResult(null)
    setLiveLogLines((currentLines) => currentLines.filter((line) => line.phase !== "steamcmd"))

    try {
      const result = await invokeTauri<RemoteSteamCmdUploadResult>("upload_steamcmd_to_remote", {
        request: {
          connection,
          remoteDirectory: steamcmdDir,
        },
      })

      setUploadResult(result)
      setSteamcmdStatus(result.success ? "success" : "error")
      finishSteamcmdTimer(startedAt)

      if (result.success) {
        setSteamcmdPath(result.steamcmdExecutablePath)
        try {
          await refreshConfig()
        } catch (refreshError) {
          setError(`SteamCMD was configured, but the saved config could not be refreshed: ${getErrorMessage(refreshError)}`)
        }
        setActiveStep(3)
      }
    } catch (uploadError) {
      setSteamcmdStatus("error")
      finishSteamcmdTimer(startedAt)
      setError(getErrorMessage(uploadError))
    }
  }

  async function saveExistingZomboidStep() {
    if (!canSaveExistingZomboid) return

    const startedAt = Date.now()
    setZomboidStatus("running")
    setZomboidStartedAt(startedAt)
    setZomboidElapsedSeconds(0)
    setError(null)

    try {
      const savedConfig = await invokeTauri<RemoteWorkspaceConfig>("save_remote_zomboid_server_path", {
        request: {
          connection,
          serverDirectory: zomboidServerDir,
          serverLaunchPath: resolvedZomboidServerPath,
        },
      })
      setConfig(savedConfig)
      setZomboidServerDir(savedConfig.remoteZomboidServerDir)
      setZomboidServerPath(savedConfig.remoteZomboidServerPath)
      setZomboidStatus("success")
      finishZomboidTimer(startedAt)
      setActiveStep(4)
    } catch (saveError) {
      setZomboidStatus("error")
      finishZomboidTimer(startedAt)
      setError(getErrorMessage(saveError))
    }
  }

  async function downloadZomboidStep() {
    if (!canDownloadZomboid) return

    const startedAt = Date.now()
    setZomboidStatus("running")
    setZomboidStartedAt(startedAt)
    setZomboidElapsedSeconds(0)
    setError(null)
    setInstallResult(null)
    setLiveLogLines((currentLines) => currentLines.filter((line) => line.phase !== "zomboid-server"))

    try {
      const result = await invokeTauri<RemoteZomboidServerInstallResult>("install_zomboid_server_on_remote", {
        request: {
          connection,
          steamcmdPath: resolvedSteamcmdPath,
          installDirectory: zomboidServerDir,
          branch: zomboidBranch,
        },
      })

      setInstallResult(result)
      setZomboidStatus(result.success ? "success" : "error")
      finishZomboidTimer(startedAt)

      if (result.success) {
        setZomboidServerPath(result.serverExecutablePath)
        try {
          await refreshConfig()
        } catch (refreshError) {
          setError(`Server path was saved, but the saved config could not be refreshed: ${getErrorMessage(refreshError)}`)
        }
        setActiveStep(4)
      }
    } catch (installError) {
      setZomboidStatus("error")
      finishZomboidTimer(startedAt)
      setError(getErrorMessage(installError))
    }
  }

  function finishHelperTimer(startedAt: number | null = helperStartedAt) {
    setHelperElapsedSeconds((currentSeconds) =>
      startedAt === null ? currentSeconds : Math.max(currentSeconds, Math.floor((Date.now() - startedAt) / 1000)),
    )
  }

  function finishSteamcmdTimer(startedAt: number | null = steamcmdStartedAt) {
    setSteamcmdElapsedSeconds((currentSeconds) =>
      startedAt === null ? currentSeconds : Math.max(currentSeconds, Math.floor((Date.now() - startedAt) / 1000)),
    )
  }

  function finishZomboidTimer(startedAt: number | null = zomboidStartedAt) {
    setZomboidElapsedSeconds((currentSeconds) =>
      startedAt === null ? currentSeconds : Math.max(currentSeconds, Math.floor((Date.now() - startedAt) / 1000)),
    )
  }

  async function saveRemoteConfig(values: Partial<RemoteWorkspaceConfig>) {
    const baseConfig = config ?? {
      ...connection,
      remoteSteamcmdDir: steamcmdDir,
      remoteSteamcmdPath: resolvedSteamcmdPath,
      remoteZomboidServerDir: zomboidServerDir,
      remoteZomboidServerPath: resolvedZomboidServerPath,
      remoteClientRam: "4.00",
      remoteServerRam: "4.00",
      remoteModLocations: [],
    }
    const savedConfig = await invokeTauri<RemoteWorkspaceConfig>("save_remote_workspace_config", {
      config: {
        ...baseConfig,
        ...connection,
        remoteSteamcmdDir: steamcmdDir,
        remoteSteamcmdPath: resolvedSteamcmdPath,
        remoteZomboidServerDir: zomboidServerDir,
        remoteZomboidServerPath: resolvedZomboidServerPath,
        ...values,
      },
    })

    setConfig(savedConfig)
    return savedConfig
  }

  async function refreshConfig() {
    const nextConfig = await invokeTauri<RemoteWorkspaceConfig | null>("get_remote_workspace_config")
    setConfig(nextConfig)
    return nextConfig
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md">
      <div className="flex max-h-[90vh] w-full max-w-4xl flex-col overflow-hidden rounded-[8px] border border-white/10 bg-[#22272b] shadow-2xl">
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1e2327] px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="rounded-[8px] bg-cyan-500/10 p-2 text-cyan-200">
              <FileArchive size={22} />
            </div>
            <div>
              <h2 className="text-lg font-black text-white">{t("remoteSetup.modalTitle")}</h2>
              <p className="text-xs text-gray-500">{connection.username}@{connection.host}</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full bg-white/5 p-2 text-gray-400 transition-colors hover:bg-white/10 hover:text-white"
          >
            <X size={18} />
          </button>
        </div>

        <div className="grid min-h-0 flex-1 overflow-hidden lg:grid-cols-[0.8fr_1.2fr]">
          <aside className="border-b border-white/5 bg-[#1e2327]/70 p-6 lg:border-b-0 lg:border-r">
            <div className="space-y-4">
              <SetupStep
                index={1}
                title={t("remoteSetup.step1Title")}
                description={t("remoteSetup.step1Description")}
                status={helperStatus}
                active={activeStep === 1}
                onClick={() => setActiveStep(1)}
                disabled={isRunning}
              />
              <SetupStep
                index={2}
                title={t("remoteSetup.step2Title")}
                description={t("remoteSetup.step2Description")}
                status={steamcmdStatus}
                active={activeStep === 2}
                onClick={() => setActiveStep(2)}
                disabled={isRunning}
              />
              <SetupStep
                index={3}
                title={t("remoteSetup.step3Title")}
                description={t("remoteSetup.step3Description")}
                status={zomboidStatus}
                active={activeStep === 3}
                onClick={() => setActiveStep(3)}
                disabled={isRunning}
              />
              <SetupStep
                index={4}
                title={t("remoteSetup.step4Title")}
                description={t("remoteSetup.step4Description")}
                status={helperStatus === "success" && steamcmdStatus === "success" && zomboidStatus === "success" ? "success" : "idle"}
                active={activeStep === 4}
                onClick={() => setActiveStep(4)}
                disabled={isRunning}
              />
            </div>

            {connection.authMethod !== "key" && (
              <div className="mt-6 rounded-[8px] border border-yellow-400/20 bg-yellow-500/10 px-4 py-3 text-sm text-yellow-100">
                {t("remoteSetup.sshKeyNotice")}
              </div>
            )}
          </aside>

          <div className="min-h-0 overflow-y-auto p-6 custom-scrollbar">
            {activeStep === 1 && (
              <section className="space-y-5">
                <div>
                  <h3 className="text-xl font-black text-white">{t("remoteSetup.step1Header")}</h3>
                  <p className="mt-1 text-sm text-gray-400">
                    {t("remoteSetup.step1Subheader")}
                  </p>
                </div>

                <div className="grid gap-4">
                  <RemotePathInput
                    label="Remote setup folder"
                    value={helperResult?.remotePath ? parentRemotePath(helperResult.remotePath) : defaultHelperDir}
                    placeholder={defaultHelperDir}
                    disabled
                    onChange={() => undefined}
                  />
                </div>

                <ResultPanel
                  title="Remote setup upload"
                  status={helperStatus}
                  result={helperResult}
                  liveLines={liveLogLines.filter((line) => line.phase === "helper")}
                  elapsedSeconds={helperElapsedSeconds}
                />

                <div className="flex justify-end">
                  <button
                    type="button"
                    disabled={!canSetupHelper}
                    onClick={() => void setupHelperStep()}
                    className="flex items-center justify-center gap-2 rounded-[8px] bg-cyan-500 px-5 py-2 text-sm font-black text-white transition-colors hover:bg-cyan-400 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
                  >
                    {helperStatus === "running" ? <Loader2 size={17} className="animate-spin" /> : <UploadCloud size={17} />}
                    {helperStatus === "running" ? "Sending..." : "Send setup"}
                  </button>
                </div>
              </section>
            )}

            {activeStep === 2 && (
              <section className="space-y-5">
                <div>
                  <h3 className="text-xl font-black text-white">{t("remoteSetup.step2Header")}</h3>
                  <p className="mt-1 text-sm text-gray-400">
                    {t("remoteSetup.step2Subheader")}
                  </p>
                </div>

                <div className="grid gap-4">
                  <RemotePathInput
                    label={t("remoteSetup.step2DirLabel")}
                    value={steamcmdDir}
                    placeholder={defaultSteamcmdDir}
                    disabled={isRunning}
                    onChange={setSteamcmdDir}
                  />
                  <RemotePathInput
                    label={t("remoteSetup.step2PathLabel")}
                    value={resolvedSteamcmdPath}
                    placeholder="/usr/games/steamcmd"
                    disabled
                    onChange={setSteamcmdPath}
                  />
                  {steamcmdDir.trim().length > 0 && !isSteamcmdDirValid ? (
                    <p className="rounded-[8px] border border-yellow-400/20 bg-yellow-500/10 px-4 py-3 text-sm text-yellow-100">
                      {t("remoteSetup.linuxPathRequired")}
                    </p>
                  ) : null}
                </div>

                <ResultPanel
                   title={t("remoteSetup.step2PanelTitle")}
                  status={steamcmdStatus}
                  result={uploadResult}
                  liveLines={liveLogLines.filter((line) => line.phase === "steamcmd")}
                  elapsedSeconds={steamcmdElapsedSeconds}
                />

                <div className="flex justify-between gap-3">
                  <button
                    type="button"
                    disabled={isRunning}
                    onClick={() => setActiveStep(1)}
                    className="rounded-[8px] border border-white/10 px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {t("remoteSetup.btnBack")}
                  </button>
                  <button
                    type="button"
                    disabled={!canInstallSteamcmd}
                    onClick={() => void installSteamcmdStep()}
                    className="flex items-center justify-center gap-2 rounded-[8px] bg-cyan-500 px-5 py-2 text-sm font-black text-white transition-colors hover:bg-cyan-400 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
                  >
                    {steamcmdStatus === "running" ? <Loader2 size={17} className="animate-spin" /> : <UploadCloud size={17} />}
                    {steamcmdStatus === "running" ? t("remoteSetup.step2Installing") : t("remoteSetup.step2InstallBtn")}
                  </button>
                </div>
              </section>
            )}

            {activeStep === 3 && (
              <section className="space-y-5">
                <div>
                  <h3 className="text-xl font-black text-white">{t("remoteSetup.step3Header")}</h3>
                  <p className="mt-1 text-sm text-gray-400">
                    {t("remoteSetup.step3Subheader")}
                  </p>
                </div>

                <div className="grid gap-3 sm:grid-cols-2">
                  <ModeButton
                    active={zomboidMode === "existing"}
                    title={t("remoteSetup.step3ModeExistingTitle")}
                    description={t("remoteSetup.step3ModeExistingDesc")}
                    onClick={() => setZomboidMode("existing")}
                  />
                  <ModeButton
                    active={zomboidMode === "download"}
                    title={t("remoteSetup.step3ModeDownloadTitle")}
                    description={t("remoteSetup.step3ModeDownloadDesc")}
                    onClick={() => setZomboidMode("download")}
                  />
                </div>

                <div className="grid gap-4">
                  <RemotePathInput
                    label={t("remoteSetup.step3DirLabel")}
                    value={zomboidServerDir}
                    placeholder={defaultZomboidServerDir}
                    disabled={isRunning}
                    onChange={setZomboidServerDir}
                  />
                  <RemotePathInput
                    label={t("remoteSetup.step3PathLabel")}
                    value={resolvedZomboidServerPath}
                    placeholder={joinRemotePath(defaultZomboidServerDir, "start-server.sh")}
                    disabled={isRunning || zomboidMode === "download"}
                    onChange={setZomboidServerPath}
                  />
                  {(zomboidServerDir.trim().length > 0 && !isZomboidServerDirValid) ||
                  (zomboidMode === "existing" && resolvedZomboidServerPath.trim().length > 0 && !isZomboidServerPathValid) ? (
                    <p className="rounded-[8px] border border-yellow-400/20 bg-yellow-500/10 px-4 py-3 text-sm text-yellow-100">
                      {t("remoteSetup.linuxPathRequired")}
                    </p>
                  ) : null}
                </div>

                {zomboidMode === "download" && (
                  <div className="space-y-3">
                    <div>
                      <p className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                        {t("remoteSetup.step3BranchLabel")}
                      </p>
                      <p className="mt-1 ml-1 font-mono text-xs text-gray-500">{selectedZomboidBranchCommand}</p>
                    </div>
                    <div className="grid gap-3 sm:grid-cols-2">
                      <ModeButton
                        active={zomboidBranch === "default"}
                        title={t("remoteSetup.step3BranchDefaultTitle")}
                        description={t("remoteSetup.step3BranchDefaultDesc")}
                        onClick={() => setZomboidBranch("default")}
                      />
                      <ModeButton
                        active={zomboidBranch === "unstable"}
                        title={t("remoteSetup.step3BranchUnstableTitle")}
                        description={t("remoteSetup.step3BranchUnstableDesc")}
                        onClick={() => setZomboidBranch("unstable")}
                      />
                    </div>
                  </div>
                )}

                {zomboidMode === "download" && (
                  <ResultPanel
                    title={t("remoteSetup.step3PanelTitle")}
                    status={zomboidStatus}
                    result={installResult}
                    liveLines={liveLogLines.filter((line) => line.phase === "zomboid-server")}
                    elapsedSeconds={zomboidElapsedSeconds}
                  />
                )}

                <div className="flex justify-between gap-3">
                  <button
                    type="button"
                    disabled={isRunning}
                    onClick={() => setActiveStep(2)}
                    className="rounded-[8px] border border-white/10 px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {t("remoteSetup.btnBack")}
                  </button>
                  {zomboidMode === "existing" ? (
                    <button
                      type="button"
                      disabled={!canSaveExistingZomboid}
                      onClick={() => void saveExistingZomboidStep()}
                      className="flex items-center justify-center gap-2 rounded-[8px] bg-cyan-500 px-5 py-2 text-sm font-black text-white transition-colors hover:bg-cyan-400 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
                    >
                      {zomboidStatus === "running" ? <Loader2 size={17} className="animate-spin" /> : <Save size={17} />}
                      {zomboidStatus === "running" ? t("remoteSetup.step3Saving") : t("remoteSetup.step3SaveBtn")}
                    </button>
                  ) : (
                    <button
                      type="button"
                      disabled={!canDownloadZomboid}
                      onClick={() => void downloadZomboidStep()}
                      className="flex items-center justify-center gap-2 rounded-[8px] bg-cyan-500 px-5 py-2 text-sm font-black text-white transition-colors hover:bg-cyan-400 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
                    >
                      {zomboidStatus === "running" ? <Loader2 size={17} className="animate-spin" /> : <HardDriveDownload size={17} />}
                      {zomboidStatus === "running" ? t("remoteSetup.step3Downloading") : t("remoteSetup.step3DownloadBtn")}
                    </button>
                  )}
                </div>
              </section>
            )}

            {activeStep === 4 && (
              <section className="space-y-5">
                <div>
                  <h3 className="text-xl font-black text-white">{t("remoteSetup.step4Header")}</h3>
                  <p className="mt-1 text-sm text-gray-400">
                    {t("remoteSetup.step4Subheader")}
                  </p>
                </div>
                <div className="grid gap-3 rounded-[8px] border border-green-400/20 bg-green-500/10 p-4 text-sm text-green-100">
                  <SavedPath label={t("remoteSetup.step1Title")} value={helperResult?.remotePath ? parentRemotePath(helperResult.remotePath) : defaultHelperDir} />
                  <SavedPath label={t("remoteSetup.step2Title")} value={resolvedSteamcmdPath} />
                  <SavedPath label={t("remoteSetup.step3Title")} value={zomboidServerDir} />
                  <SavedPath label={t("remoteSetup.step3PathLabel")} value={resolvedZomboidServerPath} />
                </div>
                <div className="flex justify-between gap-3">
                  <button
                    type="button"
                    onClick={() => setActiveStep(3)}
                    className="rounded-[8px] border border-white/10 px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
                  >
                    {t("remoteSetup.btnBack")}
                  </button>
                  <button
                    type="button"
                    onClick={onClose}
                    className="rounded-[8px] bg-cyan-500 px-5 py-2 text-sm font-black text-white transition-colors hover:bg-cyan-400"
                  >
                    {t("remoteSetup.btnDone")}
                  </button>
                </div>
              </section>
            )}

            {error && (
              <div className="mt-5 rounded-[8px] border border-red-400/20 bg-red-500/10 px-4 py-3 text-sm text-red-200">
                {error}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

function zomboidModeRequiresSteamcmd(mode: ZomboidSetupMode) {
  return mode === "download"
}

function SetupStep({
  index,
  title,
  description,
  status,
  active,
  onClick,
  disabled,
}: {
  index: number
  title: string
  description: string
  status: StepStatus
  active: boolean
  onClick?: () => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`w-full rounded-[8px] border p-4 text-left transition-all ${
        active
          ? "border-cyan-300/30 bg-cyan-500/10"
          : "border-white/5 bg-[#22272b] hover:border-white/10 hover:bg-[#272e34]"
      } disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-[#22272b] disabled:hover:border-white/5`}
    >
      <div className="flex items-start gap-3">
        <div className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] text-sm font-black ${
          status === "success"
            ? "bg-green-500 text-white"
            : status === "running"
              ? "bg-cyan-500 text-white"
              : status === "error"
                ? "bg-red-500 text-white"
                : "bg-white/5 text-gray-400"
        }`}>
          {status === "success" ? <CheckCircle2 size={17} /> : status === "running" ? <Loader2 size={17} className="animate-spin" /> : index}
        </div>
        <div>
          <h3 className="text-sm font-black text-white">{title}</h3>
          <p className="mt-1 text-xs leading-5 text-gray-500">{description}</p>
        </div>
      </div>
    </button>
  )
}

function ModeButton({
  active,
  title,
  description,
  onClick,
}: {
  active: boolean
  title: string
  description: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-[8px] border p-4 text-left transition-colors ${
        active
          ? "border-cyan-300/40 bg-cyan-500/10 text-cyan-100"
          : "border-white/5 bg-[#1e2327] text-gray-400 hover:border-white/10 hover:text-white"
      }`}
    >
      <p className="text-sm font-black">{title}</p>
      <p className="mt-1 text-xs leading-5 opacity-80">{description}</p>
    </button>
  )
}

function RemotePathInput({
  label,
  value,
  placeholder,
  disabled,
  onChange,
}: {
  label: string
  value: string
  placeholder: string
  disabled?: boolean
  onChange: (value: string) => void
}) {
  return (
    <label className="space-y-2">
      <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
        {label}
      </span>
      <input
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        className="w-full rounded-[8px] border border-white/5 bg-[#1e2327] px-4 py-3 text-sm font-semibold text-white transition-all placeholder:text-gray-600 focus:border-cyan-300/50 focus:outline-none focus:ring-1 focus:ring-cyan-300/20 disabled:cursor-not-allowed disabled:text-gray-500"
        placeholder={placeholder}
      />
    </label>
  )
}

function ResultPanel({
  title,
  status,
  result,
  liveLines = [],
  elapsedSeconds = 0,
}: {
  title: string
  status: StepStatus
  result: RemoteSetupResult | null
  liveLines?: RemoteSetupLogEvent[]
  elapsedSeconds?: number
}) {
  const isSuccess = status === "success"
  const isError = status === "error"
  const PanelIcon = title.includes("SteamCMD")
    ? UploadCloud
    : title.toLowerCase().includes("helper")
      ? FileArchive
      : HardDriveDownload
  const output = liveLines.length > 0
    ? formatLiveLogOutput(liveLines)
    : result
      ? formatResultOutput(result)
      : "(no output yet)"

  return (
    <div className={`overflow-hidden rounded-[8px] border ${
      isSuccess ? "border-green-400/20" : isError ? "border-red-400/20" : "border-white/5"
      }`}>
      <div className="flex items-center justify-between gap-3 border-b border-white/5 bg-[#1e2327] px-4 py-3">
        <div className="flex items-center gap-2">
          <PanelIcon size={17} className="text-cyan-200" />
          <span className="text-xs font-black uppercase tracking-widest text-gray-400">{title}</span>
        </div>
        <div className="flex items-center gap-3">
          {(status === "running" || elapsedSeconds > 0) && (
            <span className="rounded-[6px] border border-white/5 bg-black/20 px-2 py-1 font-mono text-[11px] font-bold text-gray-300">
              {formatElapsedTime(elapsedSeconds)}
            </span>
          )}
          <span className={isSuccess ? "text-xs font-bold text-green-300" : isError ? "text-xs font-bold text-red-300" : "text-xs font-bold text-gray-500"}>
            {status === "running" ? "Running" : status === "success" ? "Done" : status === "error" ? "Failed" : "Waiting"}
          </span>
        </div>
      </div>

      <pre className="max-h-56 overflow-auto whitespace-pre-wrap bg-[#15191d] p-4 font-mono text-xs leading-5 text-gray-300 custom-scrollbar">
        {output}
      </pre>
    </div>
  )
}

function SavedPath({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-[10px] font-black uppercase tracking-widest text-green-300/70">{label}</p>
      <p className="mt-1 break-all font-mono text-xs text-green-50">{value}</p>
    </div>
  )
}

function formatSetupFailure(result: RemoteSetupResult, fallback: string) {
  const details = [result.stderr, result.stdout]
    .map((value) => value.trim())
    .filter(Boolean)
    .join("\n")

  return details ? `${fallback}\n\n${details}` : fallback
}
function formatResultOutput(result: RemoteSetupResult) {
  const output = [
    `exit ${result.exitCode ?? "-"}`,
    result.command.trim() ? `$ command\n${result.command.trim()}` : "",
    result.stdout.trim() ? `$ stdout\n${result.stdout.trim()}` : "",
    result.stderr.trim() ? `$ stderr\n${result.stderr.trim()}` : "",
  ].filter(Boolean).join("\n\n")

  return output || "(no output)"
}

function formatLiveLogOutput(lines: RemoteSetupLogEvent[]) {
  return lines
    .map((event) => {
      const prefix = event.stream === "stderr" ? "[ERR]" : event.stream === "info" ? "[..]" : "[OUT]"
      return `${prefix} ${event.line}`
    })
    .join("\n")
}

function formatElapsedTime(totalSeconds: number) {
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const parts = hours > 0
    ? [hours, minutes, seconds]
    : [minutes, seconds]

  return parts.map((part) => String(part).padStart(2, "0")).join(":")
}
