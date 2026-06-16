import { Box, Check, ChevronLeft, ChevronRight, Copy, Plus, RotateCcw, Save, Server, Users, X } from "lucide-react"
import { useMemo, useState } from "react"
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
  const compatibleAvailableMods = useMemo(
    () => availableMods.filter((mod) => supportsBuild(mod, gameBuild)),
    [availableMods, gameBuild],
  )

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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="flex h-full max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#161a1d] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1c2126] p-6">
          <div className="flex min-w-0 items-center gap-3">
            <div className="rounded-xl bg-orange-500/20 p-2.5 text-orange-500 ring-1 ring-orange-500/20">
              <Plus size={24} />
            </div>
            <div className="min-w-0">
              <h3 className="text-xl font-black uppercase italic tracking-tight text-white">{t("createServer.title")}</h3>
              <p className="text-xs font-medium text-gray-500">{t("createServer.step", { current: step })}</p>
            </div>
          </div>
          <button onClick={onClose} className="rounded-full bg-white/5 p-2 text-gray-400 transition-all hover:bg-white/10 hover:text-white">
            <X size={20} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
          {step === 1 ? (
            <div className="space-y-8 animate-in slide-in-from-right-4 duration-300">
              <div className="space-y-2">
                <label className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                  {t("createServer.name")}
                </label>
                <div className="relative group">
                  <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within:text-orange-500">
                    <Server size={18} />
                  </div>
                  <input
                    autoFocus
                    type="text"
                    value={serverName}
                    onChange={(e) => setServerName(e.target.value)}
                    placeholder={t("createServer.placeholder")}
                    className="w-full rounded-2xl border border-white/5 bg-[#1c2126] py-4.5 pl-12 pr-4 text-lg font-bold text-white transition-all placeholder:text-gray-700 focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20"
                  />
                </div>
              </div>

              <div className="rounded-2xl border border-orange-500/10 bg-orange-500/5 p-4 ring-1 ring-orange-500/5">
                <p className="text-xs font-medium leading-relaxed text-gray-400">
                  {t("createServer.tip")}
                </p>
              </div>

              <div className="grid grid-cols-2 gap-6">
                <div className="space-y-2">
                  <label className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                    {t("createServer.maxPlayers")}
                  </label>
                  <div className="relative group">
                    <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within:text-orange-500">
                      <Users size={18} />
                    </div>
                    <input
                      type="number"
                      min={1}
                      max={100}
                      value={maxPlayers}
                      onChange={(event) => setMaxPlayers(clampNumber(event.target.valueAsNumber, 1, 100, 16))}
                      className="w-full rounded-2xl border border-white/5 bg-[#1c2126] py-4 pl-12 pr-4 text-lg font-bold text-white transition-all focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20"
                    />
                  </div>
                </div>

                <div className="space-y-2">
                  <label className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                    {t("createServer.build")}
                  </label>
                  <div className="grid grid-cols-2 gap-2 h-[60px]">
                    {(["b41", "b42"] as GameBuild[]).map((build) => (
                      <button
                        key={build}
                        type="button"
                        onClick={() => {
                          setGameBuild(build)
                          setSelectedModIds(new Set())
                          setCloneSourceId("")
                        }}
                        className={`rounded-xl border text-xs font-black uppercase italic transition-all ${
                          gameBuild === build
                            ? "border-orange-500/40 bg-orange-500/10 text-orange-400 ring-1 ring-orange-500/20 shadow-[0_0_15px_rgba(249,115,22,0.1)]"
                            : "border-white/5 bg-[#1c2126] text-gray-500 hover:bg-white/5"
                        }`}
                      >
                        {build}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="space-y-8 animate-in slide-in-from-right-4 duration-300">
              <div className="flex gap-1 rounded-2xl border border-white/5 bg-[#1c2126] p-1.5">
                <button
                  type="button"
                  onClick={() => selectModSelectionMode("checklist")}
                  className={`flex flex-1 items-center justify-center gap-2 rounded-xl py-3.5 text-xs font-black uppercase italic transition-all ${
                    modSelectionMode === "checklist"
                      ? "bg-orange-500/10 text-orange-400 ring-1 ring-orange-500/20 shadow-lg"
                      : "text-gray-500 hover:text-gray-300 hover:bg-white/5"
                  }`}
                >
                  <Box size={16} />
                  {t("createServer.modList")}
                </button>
                <button
                  type="button"
                  onClick={() => selectModSelectionMode("clone")}
                  className={`flex flex-1 items-center justify-center gap-2 rounded-xl py-3.5 text-xs font-black uppercase italic transition-all ${
                    modSelectionMode === "clone"
                      ? "bg-orange-500/10 text-orange-400 ring-1 ring-orange-500/20 shadow-lg"
                      : "text-gray-500 hover:text-gray-300 hover:bg-white/5"
                  }`}
                >
                  <Copy size={16} />
                  {t("createServer.clone")}
                </button>
              </div>

              {modSelectionMode === "checklist" ? (
                <div key="checklist" className="space-y-4">
                  <div className="flex items-center justify-between px-1">
                    <label className="text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                      {t("createServer.selectMods", { count: selectedModIds.size })}
                    </label>
                  </div>
                  <div className="grid max-h-[320px] grid-cols-1 gap-2 overflow-y-auto pr-2 custom-scrollbar">
                    {compatibleAvailableMods.map((mod) => (
                      <label
                        key={mod.id}
                        className={`group flex cursor-pointer items-center gap-4 rounded-2xl border p-4 transition-all ${
                          selectedModIds.has(mod.id)
                            ? "border-orange-500/30 bg-orange-500/10 ring-1 ring-orange-500/10"
                            : "border-white/5 bg-[#1c2126] hover:border-white/10 hover:bg-[#1f252a]"
                        }`}
                      >
                        <input
                          type="checkbox"
                          className="hidden"
                          checked={selectedModIds.has(mod.id)}
                          onChange={() => toggleMod(mod.id)}
                        />
                        <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-lg border transition-all ${
                          selectedModIds.has(mod.id)
                            ? "border-orange-500 bg-orange-500 shadow-[0_0_10px_rgba(249,115,22,0.4)]"
                            : "border-white/10 bg-[#161a1d] group-hover:border-white/20"
                        }`}>
                          {selectedModIds.has(mod.id) && <Check size={14} className="text-white" strokeWidth={3} />}
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className={`truncate text-sm font-bold ${selectedModIds.has(mod.id) ? "text-white" : "text-gray-300"}`}>{mod.name}</p>
                          <p className="truncate font-mono text-[10px] text-gray-600">{mod.id}</p>
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
              ) : (
                <div key="clone" className="space-y-4">
                  <label className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                    {t("createServer.source")}
                  </label>
                  <div className="grid max-h-[320px] gap-2 overflow-y-auto pr-2 custom-scrollbar">
                    {existingServers.map((server) => (
                      <button
                        key={server.id}
                        disabled={server.gameBuild !== gameBuild}
                        onClick={() => setCloneSourceId(server.id)}
                        className={`flex items-center gap-4 rounded-2xl border p-4 transition-all text-left ${
                          cloneSourceId === server.id
                            ? "bg-orange-500/10 border-orange-500/30 ring-1 ring-orange-500/20"
                            : "bg-[#1c2126] border-white/5 hover:border-white/10 hover:bg-[#1f252a] disabled:cursor-not-allowed disabled:opacity-30"
                        }`}
                      >
                        <div className={`rounded-xl p-2.5 transition-colors ${cloneSourceId === server.id ? "bg-orange-500 text-white shadow-[0_0_15px_rgba(249,115,22,0.4)]" : "bg-[#161a1d] text-gray-600"}`}>
                          <Server size={20} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <p className={`font-black uppercase italic text-sm ${cloneSourceId === server.id ? "text-orange-400" : "text-gray-300"}`}>{server.name}</p>
                          <p className="text-[11px] font-medium text-gray-500">{t("createServer.activeMods", { count: server.modsCount, build: server.gameBuild.toUpperCase() })}</p>
                        </div>
                        {cloneSourceId === server.id && (
                          <div className="rounded-full bg-orange-500/20 p-1 text-orange-500">
                            <Check size={16} strokeWidth={3} />
                          </div>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {error && (
            <div className="mt-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-6 py-4 text-sm font-medium text-red-400 ring-1 ring-red-500/10">
              {error}
            </div>
          )}
        </div>

        <div className="flex items-center justify-between border-t border-white/5 bg-[#1c2126] p-6">
          <button
            onClick={step === 1 ? onClose : handleBack}
            className="flex items-center gap-2 px-6 py-3 text-xs font-black uppercase italic tracking-wider text-gray-500 transition-colors hover:text-white"
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
              className="flex items-center gap-2 rounded-2xl bg-orange-500 px-10 py-3.5 font-black uppercase italic tracking-widest text-white shadow-[0_0_20px_rgba(249,115,22,0.3)] transition-all hover:bg-orange-600 hover:shadow-[0_0_25px_rgba(249,115,22,0.4)] disabled:bg-gray-800 disabled:text-gray-600 disabled:shadow-none active:scale-95"
            >
              <span>{t("createServer.next")}</span>
              <ChevronRight size={18} />
            </button>
          ) : (
            <button
              disabled={isCreating}
              onClick={() => void handleCreate()}
              className="group relative flex items-center gap-2 overflow-hidden rounded-2xl bg-orange-500 px-10 py-3.5 font-black uppercase italic tracking-widest text-white shadow-[0_0_20px_rgba(249,115,22,0.3)] transition-all hover:bg-orange-600 hover:shadow-[0_0_25px_rgba(249,115,22,0.4)] disabled:bg-gray-800 disabled:text-gray-600 disabled:shadow-none active:scale-95"
            >
              {isCreating ? <RotateCcw size={18} className="animate-spin" /> : <Save size={18} className="transition-transform group-hover:scale-110" />}
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
