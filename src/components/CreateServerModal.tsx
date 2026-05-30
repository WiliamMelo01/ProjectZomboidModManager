import { Box, ChevronLeft, ChevronRight, Copy, Plus, Save, Server, X } from "lucide-react"
import { useState } from "react"

import type { ZomboidMod } from "@/types/mod"
import type { ZomboidServer } from "@/types/server"

type CreateServerModalProps = {
  isOpen: boolean
  onClose: () => void
  existingServers: ZomboidServer[]
  availableMods: ZomboidMod[]
  onCreate?: (data: { name: string; modIds: string[] }) => Promise<void> | void
}

export function CreateServerModal({ isOpen, onClose, existingServers, availableMods, onCreate }: CreateServerModalProps) {
  const [step, setStep] = useState(1)
  const [serverName, setServerName] = useState("")
  const [modSelectionMode, setModSelectionMode] = useState<"checklist" | "clone">("checklist")
  const [selectedModIds, setSelectedModIds] = useState<Set<string>>(new Set())
  const [cloneSourceId, setCloneSourceId] = useState<string>("")
  const [isCreating, setIsCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  if (!isOpen) return null

  const handleNext = () => setStep(2)
  const handleBack = () => setStep(1)

  const toggleMod = (id: string) => {
    const next = new Set(selectedModIds)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    setSelectedModIds(next)
  }

  const handleCreate = async () => {
    let finalModIds: string[] = []

    if (modSelectionMode === "clone" && cloneSourceId) {
      const source = existingServers.find(s => s.id === cloneSourceId)
      finalModIds = source?.activeModIds || []
    } else {
      finalModIds = Array.from(selectedModIds)
    }

    setIsCreating(true)
    setError(null)

    try {
      await onCreate?.({ name: serverName, modIds: finalModIds })
      setStep(1)
      setServerName("")
      setSelectedModIds(new Set())
      setCloneSourceId("")
      onClose()
    } catch (createError) {
      setError(getErrorMessage(createError))
    } finally {
      setIsCreating(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-2xl overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300 flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="p-6 border-b border-white/5 flex justify-between items-center bg-[#2b3238]">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-orange-500/10 text-orange-400 rounded-xl">
              <Plus size={24} />
            </div>
            <div>
              <h3 className="text-xl font-bold text-white uppercase italic">Novo Servidor</h3>
              <p className="text-xs text-gray-400">Passo {step} de 2</p>
            </div>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
          {step === 1 ? (
            <div className="space-y-6 animate-in slide-in-from-right-4 duration-300">
              <div className="space-y-3">
                <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                  Nome do Servidor
                </label>
                <div className="relative group/input">
                  <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within/input:text-orange-400 transition-colors">
                    <Server size={18} />
                  </div>
                  <input
                    autoFocus
                    type="text"
                    value={serverName}
                    onChange={(e) => setServerName(e.target.value)}
                    placeholder="Ex: Meu Servidor de Sobrevivência"
                    className="w-full bg-[#1e2327] border border-white/5 rounded-2xl py-4 pl-12 pr-4 text-lg focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-700"
                  />
                </div>
              </div>

              <div className="p-4 bg-orange-400/5 border border-orange-400/10 rounded-2xl">
                <p className="text-xs text-gray-400 leading-relaxed">
                  Dica: Use nomes curtos e sem caracteres especiais para evitar problemas com os arquivos de configuração do Project Zomboid.
                </p>
              </div>
            </div>
          ) : (
            <div className="space-y-8 animate-in slide-in-from-right-4 duration-300">
              <div className="flex gap-4 p-1 bg-[#1e2327] rounded-2xl border border-white/5">
                <button
                  onClick={() => setModSelectionMode("checklist")}
                  className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                    modSelectionMode === "checklist" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                  }`}
                >
                  <Box size={18} />
                  Lista de Mods
                </button>
                <button
                  onClick={() => setModSelectionMode("clone")}
                  className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                    modSelectionMode === "clone" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                  }`}
                >
                  <Copy size={18} />
                  Clonar Existente
                </button>
              </div>

              {modSelectionMode === "checklist" ? (
                <div className="space-y-4">
                  <div className="flex justify-between items-center">
                    <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                      Selecionar Mods ({selectedModIds.size})
                    </label>
                  </div>
                  <div className="grid grid-cols-1 gap-2 max-h-[300px] overflow-y-auto pr-2 custom-scrollbar">
                    {availableMods.map((mod) => (
                      <label
                        key={mod.id}
                        className={`flex items-center gap-3 p-3 rounded-xl border transition-all cursor-pointer ${
                          selectedModIds.has(mod.id)
                            ? "bg-orange-400/10 border-orange-400/30 text-white"
                            : "bg-[#1e2327] border-white/5 text-gray-400 hover:border-white/10"
                        }`}
                      >
                        <input
                          type="checkbox"
                          className="hidden"
                          checked={selectedModIds.has(mod.id)}
                          onChange={() => toggleMod(mod.id)}
                        />
                        <div className={`w-5 h-5 rounded flex items-center justify-center border transition-all ${
                          selectedModIds.has(mod.id) ? "bg-orange-500 border-orange-500" : "border-white/20"
                        }`}>
                          {selectedModIds.has(mod.id) && <Plus size={14} className="text-white" />}
                        </div>
                        <span className="text-sm font-medium truncate">{mod.name}</span>
                        <span className="text-[10px] font-mono text-gray-600 ml-auto">{mod.id}</span>
                      </label>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="space-y-4">
                  <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                    Servidor de Origem
                  </label>
                  <div className="grid gap-2">
                    {existingServers.map((server) => (
                      <button
                        key={server.id}
                        onClick={() => setCloneSourceId(server.id)}
                        className={`flex items-center gap-3 p-4 rounded-2xl border transition-all text-left ${
                          cloneSourceId === server.id
                            ? "bg-orange-400/10 border-orange-400/30 ring-1 ring-orange-400/20"
                            : "bg-[#1e2327] border-white/5 hover:border-white/10"
                        }`}
                      >
                        <div className={`p-2 rounded-xl ${cloneSourceId === server.id ? "bg-orange-500 text-white" : "bg-[#2b3238] text-gray-500"}`}>
                          <Server size={18} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <p className={`font-bold text-sm ${cloneSourceId === server.id ? "text-white" : "text-gray-300"}`}>{server.name}</p>
                          <p className="text-xs text-gray-500">{server.modsCount} mods ativos</p>
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {error && (
            <div className="mt-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-white/5 bg-[#2b3238]/50 flex justify-between items-center">
          <button
            onClick={step === 1 ? onClose : handleBack}
            className="flex items-center gap-2 px-6 py-3 text-sm font-bold text-gray-400 hover:text-white transition-colors"
          >
            {step === 1 ? "Cancelar" : (
              <>
                <ChevronLeft size={18} />
                Voltar
              </>
            )}
          </button>

          {step === 1 ? (
            <button
              disabled={!serverName.trim()}
              onClick={handleNext}
              className="flex items-center gap-2 bg-orange-500 hover:bg-orange-600 disabled:bg-gray-700 disabled:text-gray-500 text-white px-8 py-3 rounded-2xl font-black uppercase italic tracking-wider transition-all shadow-lg shadow-orange-500/20 active:scale-95"
            >
              <span>Próximo</span>
              <ChevronRight size={18} />
            </button>
          ) : (
            <button
              disabled={isCreating}
              onClick={() => void handleCreate()}
              className="flex items-center gap-2 bg-gradient-to-r from-orange-500 to-orange-600 hover:from-orange-400 hover:to-orange-500 text-white px-8 py-3 rounded-2xl font-black uppercase italic tracking-wider transition-all shadow-lg shadow-orange-500/20 active:scale-95"
            >
              <Save size={18} />
              <span>{isCreating ? "Criando" : "Criar Servidor"}</span>
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  return "Nao foi possivel criar o servidor."
}
