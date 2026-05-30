import { RefreshCw } from "lucide-react"

type LoadingModsPanelProps = {
  error: string | null
  isLoading: boolean
  onRetry: () => Promise<void>
}

export function LoadingModsPanel({ error, isLoading, onRetry }: LoadingModsPanelProps) {
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
          <>
            <div className="mt-5 rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
              {error}
            </div>
            <button onClick={() => void onRetry()} className="mt-5 rounded-xl bg-orange-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-orange-600">
              Tentar novamente
            </button>
          </>
        )}
      </div>
    </div>
  )
}
