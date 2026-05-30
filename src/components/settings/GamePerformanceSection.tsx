import { CheckCircle2, Folder, FolderOpen, Monitor, RefreshCw, Search, XCircle } from "lucide-react"

import { RamDropdown } from "@/components/settings/RamDropdown"
import type { ZomboidInstallationStatus } from "@/types/settings"

type GamePerformanceSectionProps = {
  path: string
  clientRam: string
  serverRam: string
  ramOptions: string[]
  status: ZomboidInstallationStatus | null
  isScanning: boolean
  onPathChange: (path: string) => void
  onClientRamChange: (ram: string) => void
  onServerRamChange: (ram: string) => void
  onBrowse: () => void
  onOpenFolder: () => void
  onScan: () => void
}

export function GamePerformanceSection({
  path,
  clientRam,
  serverRam,
  ramOptions,
  status,
  isScanning,
  onPathChange,
  onClientRamChange,
  onServerRamChange,
  onBrowse,
  onOpenFolder,
  onScan,
}: GamePerformanceSectionProps) {
  const isConfigured = status?.isExecutableFound && status?.isClientConfigFound

  return (
    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
      <section className="bg-[#2b3238] rounded-3xl border border-white/5 p-8 shadow-xl relative group">
        <div className="absolute top-0 right-0 w-32 h-32 bg-orange-500/5 blur-3xl rounded-full -mr-16 -mt-16 transition-all group-hover:bg-orange-500/10" />
        <div className="flex items-center gap-3 mb-6 relative z-10">
          <div className="w-10 h-10 rounded-2xl bg-orange-500/10 flex items-center justify-center text-orange-400 border border-orange-500/20">
            <Monitor size={20} />
          </div>
          <div>
            <h3 className="text-xl font-bold text-white">Configuracao do Jogo</h3>
            <p className="text-xs text-gray-500">Defina o executavel e a memoria alocada para o client.</p>
          </div>
        </div>

        <div className="space-y-4 relative z-10">
          <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
            <div className="flex items-start gap-3">
              {isScanning ? (
                <RefreshCw size={20} className="text-orange-400 shrink-0 mt-0.5 animate-spin" />
              ) : isConfigured ? (
                <CheckCircle2 size={20} className="text-green-400 shrink-0 mt-0.5" />
              ) : (
                <XCircle size={20} className="text-red-400 shrink-0 mt-0.5" />
              )}
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <p className="text-sm font-bold text-white">
                    {isConfigured ? "Project Zomboid configurado" : "Project Zomboid nao configurado"}
                  </p>
                  <button type="button" onClick={onScan} className="flex items-center gap-2 rounded-xl border border-orange-500/20 bg-orange-500/10 px-3 py-1.5 text-xs font-bold text-orange-400 transition-all hover:bg-orange-500 hover:text-white">
                    <RefreshCw size={14} className={isScanning ? "animate-spin" : ""} />
                    Escanear
                  </button>
                </div>
                <p className="mt-1 text-xs text-gray-500 break-all">
                  {status?.detectedExecutablePath || "O app tenta usar a pasta padrao da Steam e localizar ProjectZomboid64.exe automaticamente."}
                </p>
                <div className="mt-3 grid gap-2 text-[11px] text-gray-500 md:grid-cols-3">
                  <span className={status?.isGameDirFound ? "text-green-300" : "text-red-300"}>
                    Pasta Steam: {status?.isGameDirFound ? "encontrada" : "nao encontrada"}
                  </span>
                  <span className={status?.isClientConfigFound ? "text-green-300" : "text-red-300"}>
                    Launcher: {status?.isClientConfigFound ? "ok" : "pendente"}
                  </span>
                  <span className={status?.isServerConfigFound ? "text-green-300" : "text-yellow-300"}>
                    Servidor: {status?.isServerConfigFound ? "ok" : "nao encontrado"}
                  </span>
                </div>
                <p className="mt-2 text-[11px] text-gray-600 break-all">
                  Pasta padrao: {status?.defaultGameDir || "C:\\Program Files (x86)\\Steam\\steamapps\\common\\ProjectZomboid"}
                </p>
              </div>
            </div>
          </div>

          <div className="space-y-3">
            <label htmlFor="game-path" className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
              Executavel do Jogo (.exe)
            </label>
            <div className="flex flex-col gap-3 md:flex-row">
              <div className="relative flex-1 group/input">
                <Folder size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within/input:text-orange-400 transition-colors" />
                <input
                  id="game-path"
                  type="text"
                  value={path}
                  onChange={(event) => onPathChange(event.target.value)}
                  placeholder="C:\\SteamLibrary\\steamapps\\common\\ProjectZomboid\\ProjectZomboid64.exe"
                  className="w-full bg-[#1e2327] border border-white/5 rounded-2xl py-3.5 pl-12 pr-4 text-sm focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-700"
                />
              </div>
              <button onClick={onBrowse} className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95">
                <Folder size={18} />
                Procurar
              </button>
              <button onClick={onOpenFolder} className="flex items-center justify-center gap-2 bg-[#2b3238] hover:bg-[#323a41] border border-white/10 px-5 py-3.5 rounded-2xl text-sm font-bold transition-all hover:border-orange-500/30 active:scale-95">
                <FolderOpen size={18} />
                Abrir pasta
              </button>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6 pt-2">
            <RamInput label="RAM do Client (Jogo)" value={clientRam} options={ramOptions} onChange={onClientRamChange} />
            <RamInput label="RAM do Servidor" value={serverRam} options={ramOptions} onChange={onServerRamChange} />
          </div>
        </div>
      </section>

      <div className="p-4 bg-orange-400/5 border border-orange-400/10 rounded-2xl flex gap-3">
        <Search size={20} className="text-orange-400 shrink-0 mt-0.5" />
        <p className="text-[11px] text-gray-400 leading-relaxed italic">
          O app usará o executável selecionado para localizar o arquivo de configuração e ajustar as flags de memória (-Xms e -Xmx). Certifique-se de selecionar o executável correto da versão que você utiliza (geralmente 64 bits).
        </p>
      </div>
    </div>
  )
}

function RamInput({ label, value, options, onChange }: { label: string; value: string; options: string[]; onChange: (ram: string) => void }) {
  return (
    <div className="space-y-3">
      <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">{label}</label>
      <RamDropdown value={value} onChange={onChange} options={options} />
    </div>
  )
}
