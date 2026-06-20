import { Play, SquareTerminal, X } from "lucide-react"
import { useMemo, useState } from "react"

import {
  createCommandRunner,
  type RemoteConnectionDraft,
  type TerminalCommandResult,
} from "@/lib/commandRunner"
import { getErrorMessage } from "@/lib/errors"

type RemoteTerminalModalProps = {
  connection: RemoteConnectionDraft
  isOpen: boolean
  onClose: () => void
}

export function RemoteTerminalModal({ connection, isOpen, onClose }: RemoteTerminalModalProps) {
  const [command, setCommand] = useState("hostname")
  const [result, setResult] = useState<TerminalCommandResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isRunning, setIsRunning] = useState(false)
  const runner = useMemo(
    () =>
      createCommandRunner({
        target: "remote",
        localWorkingDirectory: "",
        remoteConnection: connection,
        isRemoteConnected: true,
      }),
    [connection],
  )
  const canRun = command.trim().length > 0 && !isRunning && !runner.unavailableReason

  if (!isOpen) return null

  async function runCommand() {
    if (!canRun) return

    setIsRunning(true)
    setError(null)
    setResult(null)

    try {
      setResult(await runner.run(command))
    } catch (runError) {
      setError(getErrorMessage(runError))
    } finally {
      setIsRunning(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md">
      <div className="flex max-h-[86vh] w-full max-w-3xl flex-col overflow-hidden rounded-[8px] border border-white/10 bg-[#22272b] shadow-2xl">
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1e2327] px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="rounded-[8px] bg-cyan-500/10 p-2 text-cyan-200">
              <SquareTerminal size={22} />
            </div>
            <div>
              <h2 className="text-lg font-black text-white">Remote VM terminal</h2>
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

        <div className="overflow-y-auto p-6 custom-scrollbar">
          {runner.unavailableReason && (
            <div className="mb-4 rounded-[8px] border border-yellow-400/20 bg-yellow-500/10 px-4 py-3 text-sm text-yellow-100">
              {runner.unavailableReason}
            </div>
          )}

          <label className="space-y-2">
            <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
              SSH command
            </span>
            <textarea
              value={command}
              onChange={(event) => setCommand(event.target.value)}
              rows={4}
              spellCheck={false}
              className="w-full resize-y rounded-[8px] border border-white/5 bg-[#1e2327] px-4 py-3 font-mono text-sm text-white transition-all placeholder:text-gray-600 focus:border-cyan-300/50 focus:outline-none focus:ring-1 focus:ring-cyan-300/20"
              placeholder="hostname"
            />
          </label>

          {error && (
            <div className="mt-4 rounded-[8px] border border-red-400/20 bg-red-500/10 px-4 py-3 text-sm text-red-200">
              {error}
            </div>
          )}

          {result && (
            <div className={`mt-4 overflow-hidden rounded-[8px] border ${
              result.success ? "border-green-400/20" : "border-red-400/20"
            }`}>
              <div className="flex items-center justify-between gap-3 border-b border-white/5 bg-[#1e2327] px-4 py-2">
                <span className="text-xs font-black uppercase tracking-widest text-gray-400">
                  exit {result.exitCode ?? "-"}
                </span>
                <span className={result.success ? "text-xs font-bold text-green-300" : "text-xs font-bold text-red-300"}>
                  {result.success ? "Success" : "Failed"}
                </span>
              </div>
              <pre className="max-h-80 overflow-auto whitespace-pre-wrap bg-[#15191d] p-4 font-mono text-xs leading-5 text-gray-200 custom-scrollbar">
                {formatTerminalOutput(result)}
              </pre>
            </div>
          )}
        </div>

        <div className="flex justify-end gap-3 border-t border-white/5 bg-[#1e2327] px-6 py-4">
          <button
            type="button"
            onClick={onClose}
            className="rounded-[8px] border border-white/10 px-4 py-2 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
          >
            Close
          </button>
          <button
            type="button"
            disabled={!canRun}
            onClick={() => void runCommand()}
            className="flex items-center justify-center gap-2 rounded-[8px] bg-cyan-500 px-5 py-2 text-sm font-black text-white transition-colors hover:bg-cyan-400 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
          >
            <Play size={17} />
            {isRunning ? "Running..." : "Run"}
          </button>
        </div>
      </div>
    </div>
  )
}

function formatTerminalOutput(result: TerminalCommandResult) {
  const output = [
    result.stdout.trim() ? `$ stdout\n${result.stdout.trim()}` : "",
    result.stderr.trim() ? `$ stderr\n${result.stderr.trim()}` : "",
  ].filter(Boolean).join("\n\n")

  return output || "(no output)"
}
