import { AlertTriangle, CheckCircle2, Clipboard, Maximize2, Minimize2, RefreshCw, Terminal, X, XCircle } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import type { AppNotification } from "@/components/AppHeader"

type ServerTestResult = {
  status: "passed" | "failed" | "setup_error"
  summary: string
  durationSeconds: number
  batPath: string
  command: string
  warningCount: number
  criticalCount: number
  logLines: string[]
}

type ServerTestEvent = {
  serverId: string
  event: "started" | "line" | "finished" | "error"
  line: string | null
  result: ServerTestResult | null
  error: string | null
}

type ServerTestPanelProps = {
  hasDownloadProgressCard?: boolean
  onNotification?: (notification: Omit<AppNotification, "id" | "createdAt" | "isRead">) => void
}

export function ServerTestPanel({ hasDownloadProgressCard = false, onNotification }: ServerTestPanelProps) {
  const [serverId, setServerId] = useState<string | null>(null)
  const [isOpen, setIsOpen] = useState(false)
  const [isMinimized, setIsMinimized] = useState(false)
  const [panelSize, setPanelSize] = useState<"compact" | "wide">("compact")
  const [isTesting, setIsTesting] = useState(false)
  const [result, setResult] = useState<ServerTestResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [logLines, setLogLines] = useState<string[]>([])
  const [isLogCopied, setIsLogCopied] = useState(false)
  const [startedAt, setStartedAt] = useState<number | null>(null)
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const onNotificationRef = useRef(onNotification)

  useEffect(() => {
    onNotificationRef.current = onNotification
  }, [onNotification])

  useEffect(() => {
    let unlisten: (() => void) | null = null
    let isDisposed = false

    void listen<ServerTestEvent>("server-test-event", (event) => {
      const payload = event.payload

      if (payload.event === "started") {
        setServerId(payload.serverId)
        setIsTesting(true)
        setResult(null)
        setError(null)
        setLogLines([])
        setIsOpen(true)
        setIsMinimized(false)
        setStartedAt(Date.now())
        setElapsedSeconds(0)
        setIsLogCopied(false)
        return
      }

      if (payload.serverId !== serverId && serverId !== null && payload.event !== "started") {
        return
      }

      if (payload.event === "line" && payload.line) {
        setLogLines((currentLines) => [...currentLines, payload.line as string].slice(-320))
        return
      }

      if (payload.event === "finished" && payload.result) {
        setServerId(payload.serverId)
        setResult(payload.result)
        setLogLines(payload.result.logLines)
        setIsTesting(false)
        setStartedAt(null)
        setElapsedSeconds(payload.result.durationSeconds)
        setIsOpen(true)

        onNotificationRef.current?.({
          title: "Teste do servidor finalizado",
          message: payload.result.summary,
          tone: payload.result.status === "passed" ? "success" : payload.result.status === "failed" ? "error" : "warning",
          action: { type: "server-test", serverId: payload.serverId },
        })
        return
      }

      if (payload.event === "error") {
        const message = payload.error ?? "Nao foi possivel testar o servidor."
        setServerId(payload.serverId)
        setError(message)
        setIsTesting(false)
        setStartedAt(null)
        setIsOpen(true)
        onNotificationRef.current?.({
          title: "Falha ao testar servidor",
          message,
          tone: "error",
          action: { type: "server-test", serverId: payload.serverId },
        })
      }
    }).then((unsubscribe) => {
      if (isDisposed) {
        unsubscribe()
      } else {
        unlisten = unsubscribe
      }
    })

    return () => {
      isDisposed = true
      unlisten?.()
    }
  }, [serverId])

  useEffect(() => {
    if (!isTesting || !startedAt) {
      return
    }

    const interval = window.setInterval(() => {
      setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000))
    }, 1000)

    return () => window.clearInterval(interval)
  }, [isTesting, startedAt])

  useEffect(() => {
    const handleOpenPanel = (event: Event) => {
      const customEvent = event as CustomEvent<{ serverId?: string; error?: string }>

      if (customEvent.detail?.serverId) {
        setServerId(customEvent.detail.serverId)
      }

      if (customEvent.detail?.error) {
        setError(customEvent.detail.error)
        setResult(null)
        setIsTesting(false)
      }

      setIsOpen(true)
      setIsMinimized(false)
    }

    window.addEventListener("pzmm-open-server-test-panel", handleOpenPanel)

    return () => window.removeEventListener("pzmm-open-server-test-panel", handleOpenPanel)
  }, [])

  if (!isOpen) {
    return null
  }

  const visibleLogs = result?.logLines ?? logLines
  const statusStyle = getServerTestStatusStyle(result?.status, isTesting)
  const widthClass = panelSize === "wide" ? "w-[min(94vw,1040px)]" : "w-[min(92vw,720px)]"

  const closePanel = () => {
    if (isTesting) {
      setIsMinimized(true)
      return
    }

    setResult(null)
    setError(null)
    setLogLines([])
    setIsOpen(false)
  }

  const copyLog = async () => {
    if (visibleLogs.length === 0) {
      return
    }

    await navigator.clipboard.writeText(visibleLogs.join("\n"))
    setIsLogCopied(true)
  }

  if (isMinimized) {
    return (
      <button
        onClick={() => setIsMinimized(false)}
        className={`fixed right-6 z-40 w-80 rounded-2xl border border-white/10 bg-[#22272b] p-4 text-left shadow-2xl shadow-black/50 transition-all hover:bg-[#293036] ${
          hasDownloadProgressCard ? "bottom-[144px]" : "bottom-6"
        }`}
      >
        <div className="flex items-center gap-3">
          <div className={`shrink-0 rounded-xl p-2 ${statusStyle.iconBg}`}>
            {isTesting ? (
              <RefreshCw size={18} className="animate-spin text-orange-400" />
            ) : result?.status === "passed" ? (
              <CheckCircle2 size={18} className="text-green-400" />
            ) : result?.status === "failed" ? (
              <XCircle size={18} className="text-red-400" />
            ) : (
              <AlertTriangle size={18} className="text-orange-400" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-black text-white">Teste do servidor</p>
            <p className="mt-1 truncate text-xs text-gray-400">
              {serverId ?? "perfil"} · {formatDuration(elapsedSeconds)} · {logLines.length} linhas
            </p>
          </div>
          <Maximize2 size={16} className="text-gray-500" />
        </div>
      </button>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm animate-in fade-in duration-200">
      <div className={`flex max-h-[78vh] ${widthClass} flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#22272b] shadow-2xl shadow-black/50`}>
        <div className="flex items-start justify-between gap-3 border-b border-white/5 p-5">
          <div className="flex min-w-0 items-start gap-3">
            <div className={`shrink-0 rounded-xl p-2 ${statusStyle.iconBg}`}>
              {isTesting ? (
                <RefreshCw size={isMinimized ? 18 : 24} className="animate-spin text-orange-400" />
              ) : result?.status === "passed" ? (
                <CheckCircle2 size={isMinimized ? 18 : 24} className="text-green-400" />
              ) : result?.status === "failed" ? (
                <XCircle size={isMinimized ? 18 : 24} className="text-red-400" />
              ) : (
                <AlertTriangle size={isMinimized ? 18 : 24} className="text-orange-400" />
              )}
            </div>
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <h3 className="truncate text-lg font-black text-white">
                  Teste do servidor
                </h3>
              </div>
              <p className="mt-0.5 truncate text-sm text-gray-400">
                {isTesting
                  ? `${serverId ?? "perfil"} - ${formatDuration(elapsedSeconds)} - ${logLines.length} linhas`
                  : error ?? result?.summary ?? "Aguardando teste"}
              </p>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <button
              onClick={() => setPanelSize((size) => (size === "compact" ? "wide" : "compact"))}
              className="rounded-lg p-2 text-gray-500 transition-colors hover:bg-white/5 hover:text-white"
              title="Alternar tamanho"
            >
              <Maximize2 size={16} />
            </button>
            <button
              onClick={() => setIsMinimized(true)}
              className="rounded-lg p-2 text-gray-500 transition-colors hover:bg-white/5 hover:text-white"
              title="Minimizar"
            >
              <Minimize2 size={16} />
            </button>
            <button
              onClick={closePanel}
              className="rounded-lg p-2 text-gray-500 transition-colors hover:bg-white/5 hover:text-white"
              title={isTesting ? "Minimizar enquanto roda" : "Fechar"}
            >
              <X size={18} />
            </button>
          </div>
        </div>

          <>
            <div className="min-h-0 flex-1 overflow-y-auto p-5 custom-scrollbar">
              {result && (
                <div className="mb-4 grid gap-3 md:grid-cols-4">
                  <div className={`rounded-2xl border p-4 ${getServerTestStatusStyle(result.status, false).panel}`}>
                    <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Status</p>
                    <p className="mt-1 text-sm font-bold text-white">{getServerTestStatusLabel(result.status)}</p>
                  </div>
                  <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
                    <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Duracao</p>
                    <p className="mt-1 text-sm font-bold text-white">{result.durationSeconds}s</p>
                  </div>
                  <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
                    <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Perfil</p>
                    <p className="mt-1 truncate text-sm font-bold text-white">{serverId ?? "-"}</p>
                  </div>
                  <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
                    <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Avisos</p>
                    <p className={`mt-1 text-sm font-bold ${result.criticalCount > 0 ? "text-red-300" : result.warningCount > 0 ? "text-yellow-300" : "text-white"}`}>
                      {result.criticalCount} criticos / {result.warningCount} avisos
                    </p>
                  </div>
                </div>
              )}

              {result && (
                <div className="mb-4 space-y-3 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
                  <div>
                    <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Arquivo</p>
                    <p className="mt-1 break-all font-mono text-xs text-gray-300">{result.batPath}</p>
                  </div>
                  <div>
                    <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Comando</p>
                    <p className="mt-1 break-all font-mono text-xs text-gray-300">{result.command}</p>
                  </div>
                </div>
              )}

              {error && (
                <div className="mb-4 rounded-2xl border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-200">
                  {error}
                </div>
              )}

              <div className="overflow-hidden rounded-2xl border border-white/5 bg-[#111417]">
                <div className="flex items-center justify-between border-b border-white/5 px-4 py-3">
                  <div className="flex items-center gap-2 text-gray-300">
                    <Terminal size={16} className="text-orange-400" />
                    <span className="text-xs font-black uppercase tracking-widest">Log do teste</span>
                  </div>
                  <button
                    onClick={() => void copyLog()}
                    disabled={visibleLogs.length === 0}
                    className="flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-bold text-gray-400 transition-colors hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    <Clipboard size={14} />
                    {isLogCopied ? "Copiado" : "Copiar"}
                  </button>
                </div>
                <div className="max-h-80 overflow-y-auto whitespace-pre-wrap p-4 font-mono text-xs leading-relaxed custom-scrollbar">
                  {visibleLogs.length ? (
                    visibleLogs.map((line, index) => (
                      <div key={`${index}:${line.slice(0, 24)}`} className={getLogLineClassName(line)}>
                        {line}
                      </div>
                    ))
                  ) : (
                    <p className="text-gray-500">Aguardando saida do servidor...</p>
                  )}
                </div>
              </div>
            </div>

            <div className="flex items-center justify-between gap-3 border-t border-white/5 p-5">
              <div className="font-mono text-xs text-gray-500">
                {isTesting ? `Tempo: ${formatDuration(elapsedSeconds)} / 03:00` : result ? `Finalizado em ${formatDuration(result.durationSeconds)}` : "Pronto"}
              </div>
              <button
                onClick={closePanel}
                className="rounded-xl border border-white/10 px-5 py-3 text-sm font-bold text-gray-400 transition-colors hover:bg-white/5 hover:text-white"
              >
                {isTesting ? "Minimizar" : "Fechar"}
              </button>
            </div>
          </>
      </div>
    </div>
  )
}

function getServerTestStatusLabel(status: ServerTestResult["status"]) {
  switch (status) {
    case "passed":
      return "Sem falhas criticas"
    case "failed":
      return "Falhas encontradas"
    case "setup_error":
      return "Configuracao incompleta"
  }
}

function getServerTestStatusStyle(status: ServerTestResult["status"] | undefined, isTesting: boolean) {
  if (isTesting) {
    return {
      iconBg: "bg-orange-500/10",
      panel: "border-orange-500/20 bg-orange-500/10",
    }
  }

  switch (status) {
    case "passed":
      return {
        iconBg: "bg-green-500/10",
        panel: "border-green-500/20 bg-green-500/10",
      }
    case "failed":
      return {
        iconBg: "bg-red-500/10",
        panel: "border-red-500/20 bg-red-500/10",
      }
    default:
      return {
        iconBg: "bg-orange-500/10",
        panel: "border-orange-500/20 bg-orange-500/10",
      }
  }
}

function formatDuration(totalSeconds: number) {
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60

  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
}

function getLogLineClassName(line: string) {
  const normalizedLine = line.toLowerCase()

  if (
    normalizedLine.includes("*** server started") ||
    normalizedLine.includes("server is listening on port") ||
    normalizedLine.includes("raknet.startup() return code: 0")
  ) {
    return "text-green-300"
  }

  if (
    normalizedLine.includes("error") ||
    normalizedLine.includes("exception") ||
    normalizedLine.includes("java.lang") ||
    normalizedLine.includes("failed") ||
    normalizedLine.includes("required mod") ||
    normalizedLine.includes("workshop item") ||
    normalizedLine.includes("missing mod") ||
    normalizedLine.includes("missing required")
  ) {
    return "text-red-300"
  }

  if (normalizedLine.includes("warn")) {
    return "text-yellow-300"
  }

  if (normalizedLine.includes("log")) {
    return "text-gray-300"
  }

  return "text-gray-400"
}
