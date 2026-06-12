import { Box, ChevronLeft, ChevronRight, Copy, Plus, Save, Server, Users, X } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import type { ZomboidMod } from "@/types/mod"
import type { GameBuild, ZomboidServer } from "@/types/server"
import { supportsBuild } from "@/lib/modBuilds"
import { i18n } from "@/i18n"

type CreateServerModalProps = {
  isOpen: boolean
  onClose: () => void
  existingServers: ZomboidServer[]
  availableMods: ZomboidMod[]
  onCreate?: (data: { name: string; modIds: string[]; gameBuild: GameBuild; maxPlayers: number }) => Promise<void> | void
}

export function CreateServerModal({ isOpen, onClose, existingServers, availableMods, onCreate }: CreateServerModalProps) {
  const { t } = useTranslation()
  const [step, setStep] = useState(1)
  const [serverName, setServerName] = useState("")
  const [maxPlayers, setMaxPlayers] = useState(16)
  const [gameBuild, setGameBuild] = useState<GameBuild>("b41")
  const [modSelectionMode, setModSelectionMode] = useState<"checklist" | "clone">("checklist")
  const [selectedModIds, setSelectedModIds] = useState<Set<string>>(new Set())
  const [cloneSourceId, setCloneSourceId] = useState<string>("")
  const [isCreating, setIsCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  if (!isOpen) return null

  const handleNext = () => setStep(2)
  const handleBack = () => setStep(1)

  const selectModSelectionMode = (mode: "checklist" | "clone") => {
    setModSelectionMode(mode)
    setError(null)

    if (mode === "clone") {
      setSelectedModIds(new Set())
    } else {
      setCloneSourceId("")
    }
  }

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
      await onCreate?.({ name: serverName, modIds: finalModIds, gameBuild, maxPlayers })
      setStep(1)
      setServerName("")
      setMaxPlayers(16)
      setSelectedModIds(new Set())
      setCloneSourceId("")
      setGameBuild("b41")
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
              <h3 className="text-xl font-bold text-white uppercase italic">{t("createServer.title")}</h3>
              <p className="text-xs text-gray-400">{t("createServer.step", { current: step })}</p>
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
                  {t("createServer.name")}
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
                    placeholder={t("createServer.placeholder")}
                    className="w-full bg-[#1e2327] border border-white/5 rounded-2xl py-4 pl-12 pr-4 text-lg focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-700"
                  />
                </div>
              </div>

              <div className="p-4 bg-orange-400/5 border border-orange-400/10 rounded-2xl">
                <p className="text-xs text-gray-400 leading-relaxed">
                  {t("createServer.tip")}
                </p>
              </div>

              <div className="space-y-3">
                <label className="ml-1 text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
                  {t("createServer.maxPlayers")}
                </label>
                <div className="relative group/input">
                  <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within/input:text-orange-400">
                    <Users size={18} />
                  </div>
                  <input
                    type="number"
                    min={1}
                    max={100}
                    value={maxPlayers}
                    onChange={(event) => setMaxPlayers(clampNumber(event.target.valueAsNumber, 1, 100, 16))}
                    className="w-full rounded-2xl border border-white/5 bg-[#1e2327] py-4 pl-12 pr-4 text-lg transition-all focus:border-orange-400/50 focus:outline-none focus:ring-1 focus:ring-orange-400/20"
                  />
                </div>
                <p className="ml-1 text-xs text-gray-500">{t("createServer.maxPlayersHint")}</p>
              </div>
              <div className="space-y-3">
                <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                  {t("createServer.build")}
                </label>
                <div className="grid grid-cols-2 gap-3">
                  {(["b41", "b42"] as GameBuild[]).map((build) => (
                    <button
                      key={build}
                      type="button"
                      onClick={() => {
                        setGameBuild(build)
                        setSelectedModIds(new Set())
                        setCloneSourceId("")
                      }}
                      className={`rounded-xl border px-4 py-3 text-sm font-bold uppercase ${
                        gameBuild === build ? "border-orange-400/40 bg-orange-400/10 text-orange-300" : "border-white/5 bg-[#1e2327] text-gray-400"
                      }`}
                    >
                      {build}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          ) : (
            <div className="space-y-8 animate-in slide-in-from-right-4 duration-300">
              <div className="flex gap-4 p-1 bg-[#1e2327] rounded-2xl border border-white/5">
                <button
                  type="button"
                  onClick={() => selectModSelectionMode("checklist")}
                  className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                    modSelectionMode === "checklist" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                  }`}
                >
                  <Box size={18} />
                  {t("createServer.modList")}
                </button>
                <button
                  type="button"
                  onClick={() => selectModSelectionMode("clone")}
                  className={`flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold transition-all ${
                    modSelectionMode === "clone" ? "bg-[#2b3238] text-orange-400 shadow-lg" : "text-gray-500 hover:text-gray-300"
                  }`}
                >
                  <Copy size={18} />
                  {t("createServer.clone")}
                </button>
              </div>

              {modSelectionMode === "checklist" ? (
                <div key="checklist" className="space-y-4">
                  <div className="flex justify-between items-center">
                    <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                      {t("createServer.selectMods", { count: selectedModIds.size })}
                    </label>
                  </div>
                  <div className="grid grid-cols-1 gap-2 max-h-[300px] overflow-y-auto pr-2 custom-scrollbar">
                    {availableMods.filter((mod) => supportsBuild(mod, gameBuild)).map((mod) => (
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
                <div key="clone" className="space-y-4">
                  <label className="text-[10px] font-black text-gray-500 uppercase tracking-[0.2em] ml-1">
                    {t("createServer.source")}
                  </label>
                  <div className="grid max-h-[300px] gap-2 overflow-y-auto pr-2 custom-scrollbar">
                    {existingServers.map((server) => (
                      <button
                        key={server.id}
                        disabled={server.gameBuild !== gameBuild}
                        onClick={() => setCloneSourceId(server.id)}
                        className={`flex items-center gap-3 p-4 rounded-2xl border transition-all text-left ${
                          cloneSourceId === server.id
                            ? "bg-orange-400/10 border-orange-400/30 ring-1 ring-orange-400/20"
                            : "bg-[#1e2327] border-white/5 hover:border-white/10 disabled:cursor-not-allowed disabled:opacity-40"
                        }`}
                      >
                        <div className={`p-2 rounded-xl ${cloneSourceId === server.id ? "bg-orange-500 text-white" : "bg-[#2b3238] text-gray-500"}`}>
                          <Server size={18} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <p className={`font-bold text-sm ${cloneSourceId === server.id ? "text-white" : "text-gray-300"}`}>{server.name}</p>
                          <p className="text-xs text-gray-500">{t("createServer.activeMods", { count: server.modsCount, build: server.gameBuild.toUpperCase() })}</p>
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
            {step === 1 ? t("createServer.cancel") : (
              <>
                <ChevronLeft size={18} />
                {t("createServer.back")}
              </>
            )}
          </button>

          {step === 1 ? (
            <button
              disabled={!serverName.trim() || maxPlayers < 1 || maxPlayers > 100}
              onClick={handleNext}
              className="flex items-center gap-2 bg-orange-500 hover:bg-orange-600 disabled:bg-gray-700 disabled:text-gray-500 text-white px-8 py-3 rounded-2xl font-black uppercase italic tracking-wider transition-all shadow-lg shadow-orange-500/20 active:scale-95"
            >
              <span>{t("createServer.next")}</span>
              <ChevronRight size={18} />
            </button>
          ) : (
            <button
              disabled={isCreating}
              onClick={() => void handleCreate()}
              className="flex items-center gap-2 bg-gradient-to-r from-orange-500 to-orange-600 hover:from-orange-400 hover:to-orange-500 text-white px-8 py-3 rounded-2xl font-black uppercase italic tracking-wider transition-all shadow-lg shadow-orange-500/20 active:scale-95"
            >
              <Save size={18} />
              <span>{isCreating ? t("createServer.creating") : t("createServer.create")}</span>
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

function clampNumber(value: number, min: number, max: number, fallback: number) {
  if (!Number.isFinite(value)) {
    return fallback
  }

  return Math.min(max, Math.max(min, Math.trunc(value)))
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  return i18n.t("createServer.createError")
}
