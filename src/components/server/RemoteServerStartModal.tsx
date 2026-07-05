import { CheckCircle2, Play, RefreshCw, Send, ShieldAlert, ShieldCheck, Square, Terminal, X, XCircle } from "lucide-react"
import { useEffect, useRef, useState, type ReactNode } from "react"

import type { ZomboidServer } from "@/types/server"

export type RemoteFirewallRuleStatus = {
  protocol: string
  port: number
  allowed: boolean
}

export type RemoteServerFirewallCheck = {
  serverId: string
  ports: number[]
  rules: RemoteFirewallRuleStatus[]
  missingRules: RemoteFirewallRuleStatus[]
  isConfigured: boolean
  logs: string[]
}

export type RemoteServerActionResult = {
  success: boolean
  message: string
  command: string
  logs: string[]
}

type RemoteServerStartModalProps = {
  isOpen: boolean
  server: ZomboidServer
  firewallCheck: RemoteServerFirewallCheck | null
  startResult: RemoteServerActionResult | null
  logs: string[]
  error: string | null
  isChecking: boolean
  isConfiguring: boolean
  isStarting: boolean
  onClose: () => void
  onRecheck: () => void
  onConfigureFirewall: () => void
  onStartServer: () => void
  onSendCommand: (command: string) => Promise<void>
  onStopServer: () => Promise<void>
}

export function RemoteServerStartModal({
  isOpen,
  server,
  firewallCheck,
  startResult,
  logs,
  error,
  isChecking,
  isConfiguring,
  isStarting,
  onClose,
  onRecheck,
  onConfigureFirewall,
  onStartServer,
  onSendCommand,
  onStopServer,
}: RemoteServerStartModalProps) {
  const [commandText, setCommandText] = useState("")
  const [isSendingCommand, setIsSendingCommand] = useState(false)
  const [isStoppingServer, setIsStoppingServer] = useState(false)
  const [shouldStickToBottom, setShouldStickToBottom] = useState(true)
  const logsEndRef = useRef<HTMLDivElement | null>(null)

  const hasRunningSignal = /SERVER IS RUNNING|command channel is ready|appears to be running|is running on port/i.test(startResult?.message ?? "")
  const isServerOnline = server.status === "online" || Boolean(startResult?.success && hasRunningSignal)
  const isBusy = isChecking || isConfiguring || isStarting
  const canConfigureFirewall = Boolean(firewallCheck && !firewallCheck.isConfigured && !isBusy)
  const canStartServer = Boolean(firewallCheck?.isConfigured && !isBusy && !isServerOnline)
  const isStartupWatchPending = Boolean(startResult?.success && /not detected|keeps starting|still running/i.test(`${startResult.message} ${logs.join(" ")}`))
  const canSendCommand = Boolean(isServerOnline && commandText.trim() && !isSendingCommand && !isStoppingServer)
  const canStopServer = Boolean(isServerOnline && !isSendingCommand && !isStoppingServer)
  const visibleLogs = logs.length > 0 ? logs : ["Waiting for firewall check..."]

  useEffect(() => {
    if (!isOpen || !shouldStickToBottom) return

    logsEndRef.current?.scrollIntoView({ block: "end" })
  }, [isOpen, shouldStickToBottom, visibleLogs.length])

  if (!isOpen) {
    return null
  }

  async function submitCommand() {
    const command = commandText.trim()
    if (!command || isSendingCommand || isStoppingServer) return

    setIsSendingCommand(true)
    try {
      await onSendCommand(command)
      setCommandText("")
    } finally {
      setIsSendingCommand(false)
    }
  }

  async function stopServer() {
    if (!canStopServer) return

    setIsStoppingServer(true)
    try {
      await onStopServer()
    } finally {
      setIsStoppingServer(false)
    }
  }

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm">
      <div className="flex max-h-[min(92vh,760px)] w-[min(94vw,880px)] min-h-0 flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#171b1f] shadow-2xl shadow-black/40">
        <div className="shrink-0 flex items-start justify-between gap-4 border-b border-white/10 bg-[#20262b] px-5 py-4">
          <div className="flex items-start gap-3">
            <div className={`rounded-xl border p-2 ${firewallCheck?.isConfigured ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-300" : "border-orange-400/30 bg-orange-400/10 text-orange-300"}`}>
              {firewallCheck?.isConfigured ? <ShieldCheck size={20} /> : <ShieldAlert size={20} />}
            </div>
            <div>
              <p className="text-[10px] font-black uppercase tracking-[0.22em] text-cyan-300">Server startup</p>
              <h3 className="mt-1 text-lg font-black text-white">{server.name}</h3>
              <p className="mt-1 text-xs text-gray-400">Close only hides this window; it does not stop the server.</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl border border-white/10 p-2 text-gray-400 transition-colors hover:border-white/20 hover:text-white"
          >
            <X size={18} />
          </button>
        </div>

        <div className="grid min-h-0 flex-1 gap-4 overflow-hidden p-5 lg:grid-cols-[260px_minmax(0,1fr)]">
          <div className="min-h-0 space-y-3 overflow-auto pr-1 custom-scrollbar">
            <div className="rounded-xl border border-white/10 bg-[#20262b] p-4">
              <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Firewall</p>
              {isChecking ? (
                <StatusLine icon={<RefreshCw size={16} className="animate-spin" />} text="Checking rules" tone="muted" />
              ) : firewallCheck?.isConfigured ? (
                <StatusLine icon={<CheckCircle2 size={16} />} text="Ready" tone="success" />
              ) : firewallCheck ? (
                <StatusLine icon={<ShieldAlert size={16} />} text={`${firewallCheck.missingRules.length} missing setup item(s)`} tone="warning" />
              ) : (
                <StatusLine icon={<RefreshCw size={16} />} text="Pending" tone="muted" />
              )}
              <div className="mt-3 flex flex-wrap gap-2">
                {(firewallCheck?.ports.length ? firewallCheck.ports : [Number(server.port || 16261)]).map((port) => (
                  <span key={port} className="rounded-full border border-white/10 bg-black/20 px-2.5 py-1 font-mono text-xs text-gray-300">
                    {port}
                  </span>
                ))}
              </div>
            </div>

            <div className="rounded-xl border border-white/10 bg-[#20262b] p-4">
              <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Server</p>
              {isStarting ? (
                <StatusLine icon={<RefreshCw size={16} className="animate-spin" />} text="Starting" tone="muted" />
              ) : isServerOnline ? (
                <StatusLine icon={<CheckCircle2 size={16} />} text="Running" tone="success" />
              ) : startResult?.success ? (
                <StatusLine icon={<RefreshCw size={16} />} text="Waiting for running signal" tone="muted" />
              ) : error ? (
                <StatusLine icon={<XCircle size={16} />} text="Needs attention" tone="error" />
              ) : (
                <StatusLine icon={<Terminal size={16} />} text="Ready to start server" tone="muted" />
              )}
            </div>

            <div className="rounded-xl border border-white/10 bg-[#20262b] p-4">
              <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Terminal</p>
              {isServerOnline ? (
                <StatusLine icon={<Terminal size={16} />} text="Command channel ready" tone="success" />
              ) : isStarting || isStartupWatchPending ? (
                <StatusLine icon={<RefreshCw size={16} className="animate-spin" />} text="Waiting for server" tone="muted" />
              ) : (
                <StatusLine icon={<Terminal size={16} />} text="Available when online" tone="muted" />
              )}
            </div>

            {error && (
              <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-3 text-sm font-bold text-red-200">
                {error}
              </div>
            )}
          </div>

          <div className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-white/10 bg-[#101417]">
            <div className="shrink-0 flex items-start justify-between gap-3 border-b border-white/10 px-4 py-3">
              <div>
                <p className="text-[10px] font-black uppercase tracking-widest text-gray-500">Logs</p>
                <p className="max-w-[520px] break-words text-xs text-gray-400">{startResult?.message ?? "Server startup workflow"}</p>
              </div>
              <button
                type="button"
                onClick={onRecheck}
                disabled={isBusy}
                className="rounded-lg border border-white/10 px-3 py-2 text-xs font-black text-gray-300 transition-colors hover:border-cyan-300/30 hover:text-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Recheck
              </button>
            </div>

            <div
              className="custom-scrollbar min-h-[220px] flex-1 overflow-auto overscroll-contain p-4 font-mono text-xs leading-relaxed text-gray-300"
              onScroll={(event) => {
                const element = event.currentTarget
                setShouldStickToBottom(element.scrollHeight - element.scrollTop - element.clientHeight < 64)
              }}
            >
              {visibleLogs.map((line, index) => (
                <p key={`${index}-${line}`} className={`whitespace-pre-wrap break-words ${line.toLowerCase().includes("error") || line.toLowerCase().includes("missing") ? "text-orange-200" : line.toLowerCase().includes("ready") || line.toLowerCase().includes("started") ? "text-emerald-200" : "text-gray-300"}`}>
                  {line}
                </p>
              ))}
              <div ref={logsEndRef} />
            </div>

            {isServerOnline && (
              <div className="shrink-0 border-t border-white/10 bg-[#171b1f] px-4 py-3">
                <form
                  className="flex flex-col gap-2 sm:flex-row"
                  onSubmit={(event) => {
                    event.preventDefault()
                    void submitCommand()
                  }}
                >
                  <div className="flex min-w-0 flex-1 items-center gap-2 rounded-xl border border-white/10 bg-black/20 px-3 py-2 font-mono text-sm text-gray-200">
                    <Terminal size={16} className="shrink-0 text-cyan-300" />
                    <input
                      value={commandText}
                      onChange={(event) => setCommandText(event.target.value)}
                      placeholder="save, servermsg hello, quit..."
                      disabled={isSendingCommand || isStoppingServer}
                      className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-gray-600 disabled:opacity-60"
                    />
                  </div>
                  <button
                    type="submit"
                    disabled={!canSendCommand}
                    className="flex items-center justify-center gap-2 rounded-xl border border-cyan-400/30 bg-cyan-400/10 px-4 py-2 text-sm font-black text-cyan-200 transition-colors hover:bg-cyan-400 hover:text-[#071014] disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isSendingCommand ? <RefreshCw size={18} className="animate-spin" /> : <Send size={18} />}
                    <span>Send command</span>
                  </button>
                </form>
              </div>
            )}

            <div className="shrink-0 flex flex-wrap justify-end gap-3 border-t border-white/10 bg-[#171b1f] px-4 py-3">
              {canConfigureFirewall && (
                <button
                  type="button"
                  onClick={onConfigureFirewall}
                  className="flex items-center gap-2 rounded-xl border border-cyan-400/30 bg-cyan-400/10 px-4 py-2 text-sm font-black text-cyan-200 transition-colors hover:bg-cyan-400 hover:text-[#071014]"
                >
                  <ShieldCheck size={18} />
                  <span>Configure firewall</span>
                </button>
              )}
              {isServerOnline && (
                <button
                  type="button"
                  onClick={() => void stopServer()}
                  disabled={!canStopServer}
                  className="flex items-center gap-2 rounded-xl border border-red-400/30 bg-red-400/10 px-4 py-2 text-sm font-black text-red-200 transition-colors hover:bg-red-400 hover:text-[#160b0b] disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {isStoppingServer ? <RefreshCw size={18} className="animate-spin" /> : <Square size={18} />}
                  <span>Stop server</span>
                </button>
              )}
              <button
                type="button"
                onClick={onStartServer}
                disabled={!canStartServer || Boolean(startResult?.success)}
                className="flex items-center gap-2 rounded-xl border border-emerald-400/30 bg-emerald-400/10 px-4 py-2 text-sm font-black text-emerald-200 transition-colors hover:bg-emerald-400 hover:text-[#071014] disabled:cursor-not-allowed disabled:opacity-50"
              >
                {isStarting ? <RefreshCw size={18} className="animate-spin" /> : <Play size={18} />}
                <span>Start server</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function StatusLine({ icon, text, tone }: { icon: ReactNode; text: string; tone: "success" | "warning" | "error" | "muted" }) {
  const toneClass = {
    success: "text-emerald-300",
    warning: "text-orange-300",
    error: "text-red-300",
    muted: "text-gray-300",
  }[tone]

  return <div className={`mt-2 flex items-center gap-2 text-sm font-black ${toneClass}`}>{icon}<span>{text}</span></div>
}