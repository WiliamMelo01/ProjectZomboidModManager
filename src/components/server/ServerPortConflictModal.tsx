import { AlertTriangle, RefreshCw, XCircle } from "lucide-react"

export type ServerPortCheck = {
  ports: number[]
  usages: PortUsage[]
}

type PortUsage = {
  port: number
  protocol: string
  pid: number
  processName: string
}

type ServerPortConflictModalProps = {
  check: ServerPortCheck
  isKilling: boolean
  operation?: "test" | "start"
  onCancel: () => void
  onConfirm: () => void
}

export function ServerPortConflictModal({ check, isKilling, operation = "test", onCancel, onConfirm }: ServerPortConflictModalProps) {
  const operationLabel = operation === "start" ? "iniciar" : "testar"

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4 backdrop-blur-md animate-in fade-in duration-300">
      <div className="w-full max-w-lg overflow-hidden rounded-3xl border border-orange-500/20 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="border-b border-orange-500/10 bg-orange-500/10 p-6">
          <div className="flex items-start gap-3">
            <AlertTriangle size={28} className="mt-0.5 shrink-0 text-orange-400" />
            <div>
              <h3 className="text-xl font-black text-white">Portas em uso</h3>
              <p className="mt-1 text-sm text-gray-400">
                Para {operationLabel} o servidor, as portas {check.ports.join(", ")} precisam estar livres. Encerre os processos abaixo antes de continuar.
              </p>
            </div>
          </div>
        </div>

        <div className="max-h-72 overflow-y-auto p-6 custom-scrollbar">
          <div className="space-y-3">
            {check.usages.map((usage) => (
              <div key={`${usage.protocol}:${usage.port}:${usage.pid}`} className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <p className="font-bold text-white">{usage.processName}</p>
                    <p className="mt-1 font-mono text-xs text-gray-500">PID {usage.pid}</p>
                  </div>
                  <div className="rounded-xl border border-orange-500/20 bg-orange-500/10 px-3 py-2 text-right">
                    <p className="text-[10px] font-black text-orange-300">{usage.protocol}</p>
                    <p className="font-mono text-sm font-bold text-white">{usage.port}</p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-3 border-t border-white/5 p-6 sm:flex-row sm:justify-end">
          <button
            onClick={onCancel}
            disabled={isKilling}
            className="rounded-xl border border-white/10 px-5 py-3 text-sm font-bold text-gray-400 transition-colors hover:bg-white/5 hover:text-white disabled:opacity-50"
          >
            Cancelar
          </button>
          <button
            onClick={onConfirm}
            disabled={isKilling}
            className="flex items-center justify-center gap-2 rounded-xl bg-red-500 px-5 py-3 text-sm font-black text-white transition-colors hover:bg-red-600 disabled:opacity-60"
          >
            {isKilling ? <RefreshCw size={18} className="animate-spin" /> : <XCircle size={18} />}
            Encerrar processos e {operationLabel}
          </button>
        </div>
      </div>
    </div>
  )
}
