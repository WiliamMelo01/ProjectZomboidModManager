import { AlertCircle, CheckCircle2, ChevronRight, Hash, PackageCheck, SkipForward, Wand2, X } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import { findModForServerId } from "@/lib/modBuilds"
import type { ZomboidMod } from "@/types/mod"
import type { ZomboidServer } from "@/types/server"

type FixWorkshopIdsModalProps = {
  server: ZomboidServer
  allMods?: ZomboidMod[]
  workshopMappings?: Record<string, string>
  onClose: () => void
  onSaveWorkshopMapping?: (modId: string, workshopId: string) => Promise<void>
  onApplyServerMods?: (server: ZomboidServer, activeModIds: string[]) => Promise<void>
}

export function FixWorkshopIdsModal({
  server,
  allMods = [],
  workshopMappings = {},
  onClose,
  onSaveWorkshopMapping,
  onApplyServerMods,
}: FixWorkshopIdsModalProps) {
  const { t } = useTranslation()

  const activeModIds = server?.activeModIds ?? []
  const gameBuild = server?.gameBuild ?? "b41"

  // 1. Mapeia a lista de mods ativos do servidor e identifica quais não têm Workshop ID
  const activeModsList = activeModIds.map((modId) => {
    const mod = findModForServerId(allMods, modId, gameBuild)
    const resolvedId =
      mod?.workshopId?.trim() ||
      workshopMappings[modId] ||
      workshopMappings[modId.toLowerCase()] ||
      ""
    const isLocalMod = mod?.source === "local"

    return {
      modId,
      modName: mod?.name || modId,
      workshopId: resolvedId,
      isLocalMod,
    }
  })

  // Mods ativos que não têm ID de Workshop e que NÃO são exclusivamente locais
  const missingModsQueue = activeModsList.filter(
    (item) => !item.workshopId && !item.isLocalMod
  )

  const [currentIndex, setCurrentIndex] = useState(0)
  const [inputWorkshopId, setInputWorkshopId] = useState("")
  const [isSaving, setIsSaving] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [isFinished, setIsFinished] = useState(missingModsQueue.length === 0)
  const [skippedModIds, setSkippedModIds] = useState<Set<string>>(new Set())

  const currentMissingMod = missingModsQueue[currentIndex]

  // Salvar ID para o mod atual na fila
  const handleSaveCurrentMod = async () => {
    if (!currentMissingMod) return
    setErrorMessage(null)

    const cleanId = inputWorkshopId.trim()
    if (!cleanId || !/^\d+$/.test(cleanId)) {
      setErrorMessage("Por favor, digite um ID numérico válido.")
      return
    }

    setIsSaving(true)
    try {
      if (onSaveWorkshopMapping) {
        await onSaveWorkshopMapping(currentMissingMod.modId, cleanId)
      }
      setInputWorkshopId("")

      // Avança na fila
      if (currentIndex + 1 < missingModsQueue.length) {
        setCurrentIndex(currentIndex + 1)
      } else {
        setIsFinished(true)
      }
    } catch (err) {
      setErrorMessage("Erro ao salvar o ID. Tente novamente.")
    } finally {
      setIsSaving(false)
    }
  }

  // Pular apenas o mod atual
  const handleSkipCurrentMod = () => {
    if (!currentMissingMod) return
    setErrorMessage(null)
    setInputWorkshopId("")
    setSkippedModIds((prev) => new Set(prev).add(currentMissingMod.modId))

    if (currentIndex + 1 < missingModsQueue.length) {
      setCurrentIndex(currentIndex + 1)
    } else {
      setIsFinished(true)
    }
  }

  // Pular todos os mods restantes sem ID
  const handleSkipAllRemaining = () => {
    setErrorMessage(null)
    setInputWorkshopId("")

    const newSkipped = new Set(skippedModIds)
    for (let i = currentIndex; i < missingModsQueue.length; i++) {
      newSkipped.add(missingModsQueue[i].modId)
    }
    setSkippedModIds(newSkipped)
    setIsFinished(true)
  }

  // Aplicar as alterações finais e regravar o arquivo .ini do servidor
  const handleFinalApply = async () => {
    setIsSaving(true)
    try {
      if (onApplyServerMods && server) {
        await onApplyServerMods(server, server.activeModIds ?? [])
      }
      onClose()
    } catch (err) {
      setErrorMessage("Erro ao salvar a configuração no servidor.")
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-md animate-in fade-in duration-300"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        className="w-full max-w-xl overflow-hidden rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Modal Header */}
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1e2327] px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-orange-500/20 text-orange-400 border border-orange-500/30">
              <Wand2 size={20} />
            </div>
            <div>
              <h3 className="text-lg font-bold text-white">Organizar Workshop IDs</h3>
              <p className="text-xs text-gray-400">
                Alinha os IDs da Workshop na ordem exata dos Mod IDs no servidor
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full border border-white/5 p-2 text-gray-400 transition-colors hover:bg-white/5 hover:text-white"
          >
            <X size={18} />
          </button>
        </div>

        {/* Modal Content */}
        <div className="p-6">
          {isFinished ? (
            /* Tela Concluída */
            <div className="flex flex-col items-center justify-center py-6 text-center">
              <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-green-500/20 text-green-400 border border-green-500/30">
                <CheckCircle2 size={36} />
              </div>
              <h4 className="text-xl font-bold text-white">Tudo Pronto!</h4>
              <p className="mt-2 max-w-md text-xs text-gray-300 leading-relaxed">
                {missingModsQueue.length === 0
                  ? "Todos os mods ativos no servidor já possuem ID da Workshop associado. A ordem no arquivo .ini será alinhada perfeitamente!"
                  : "Todos os mods da fila foram processados. Clique no botão abaixo para regravar o arquivo servertest.ini na ordem exata!"}
              </p>

              <div className="mt-6 w-full rounded-2xl border border-white/5 bg-[#1e2327] p-4 text-left">
                <div className="flex justify-between items-center text-xs text-gray-400 mb-2">
                  <span>Total de Mods Ativos:</span>
                  <span className="font-bold text-white">{activeModsList.length}</span>
                </div>
                <div className="flex justify-between items-center text-xs text-gray-400">
                  <span>Mods Pulados sem Workshop ID:</span>
                  <span className="font-bold text-amber-400">{skippedModIds.size}</span>
                </div>
              </div>

              <div className="mt-6 flex w-full gap-3">
                <button
                  type="button"
                  onClick={handleFinalApply}
                  disabled={isSaving}
                  className="flex-1 rounded-xl bg-orange-500 py-3 text-xs font-bold text-white shadow-lg shadow-orange-500/20 transition-all hover:bg-orange-600 disabled:opacity-50"
                >
                  {isSaving ? "Gravando no Servidor..." : "Aplicar e Salvar no Servidor"}
                </button>
              </div>
            </div>
          ) : (
            /* Tela de Preenchimento da Fila */
            <div>
              {/* Indicador de Progresso */}
              <div className="mb-6 flex items-center justify-between">
                <span className="text-xs font-bold uppercase tracking-wider text-orange-400">
                  Mod {currentIndex + 1} de {missingModsQueue.length} sem ID
                </span>
                <span className="text-xs text-gray-500">
                  {Math.round(((currentIndex) / missingModsQueue.length) * 100)}% concluído
                </span>
              </div>

              {/* Card do Mod Atual */}
              <div className="mb-6 rounded-2xl border border-white/10 bg-[#1e2327] p-4">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[#2b3238] text-orange-400 border border-white/5">
                    <PackageCheck size={20} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <h4 className="truncate text-base font-bold text-white">
                      {currentMissingMod?.modName}
                    </h4>
                    <p className="truncate font-mono text-xs text-gray-400">
                      ID: {currentMissingMod?.modId}
                    </p>
                  </div>
                </div>

                {/* Campo de Input para o Workshop ID */}
                <div className="mt-4">
                  <label className="mb-1.5 block text-[10px] font-bold uppercase tracking-wider text-gray-400">
                    Digite o Workshop ID deste mod:
                  </label>
                  <div className="flex gap-2">
                    <div className="relative flex-1">
                      <Hash size={14} className="absolute left-3 top-3 text-orange-400" />
                      <input
                        type="text"
                        value={inputWorkshopId}
                        onChange={(e) => setInputWorkshopId(e.target.value.replace(/\D/g, ""))}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") void handleSaveCurrentMod()
                        }}
                        placeholder="Ex: 2460154815"
                        disabled={isSaving}
                        autoFocus
                        className="w-full rounded-xl border border-white/10 bg-black/40 py-2.5 pl-9 pr-3 text-xs font-mono text-white placeholder-gray-600 focus:border-orange-400/50 focus:outline-none"
                      />
                    </div>
                    <button
                      type="button"
                      onClick={handleSaveCurrentMod}
                      disabled={isSaving}
                      className="flex items-center gap-1 rounded-xl bg-orange-500 px-4 py-2.5 text-xs font-bold text-white transition-all hover:bg-orange-600 shadow-md shadow-orange-500/20 disabled:opacity-50"
                    >
                      <span>{isSaving ? "Salvando..." : "Salvar"}</span>
                      <ChevronRight size={14} />
                    </button>
                  </div>
                  {errorMessage && (
                    <p className="mt-2 text-[10px] font-bold text-red-400">{errorMessage}</p>
                  )}
                </div>
              </div>

              {/* Botões de Ação para Pular */}
              <div className="flex flex-col gap-2 border-t border-white/5 pt-4 sm:flex-row sm:justify-between">
                <button
                  type="button"
                  onClick={handleSkipCurrentMod}
                  disabled={isSaving}
                  className="flex items-center justify-center gap-1.5 rounded-xl border border-white/10 bg-[#2b3238] px-4 py-2.5 text-xs font-bold text-gray-300 transition-colors hover:bg-[#353c42] hover:text-white"
                >
                  <SkipForward size={14} />
                  <span>Pular este mod</span>
                </button>

                <button
                  type="button"
                  onClick={handleSkipAllRemaining}
                  disabled={isSaving}
                  className="flex items-center justify-center gap-1.5 rounded-xl border border-amber-500/20 bg-amber-500/10 px-4 py-2.5 text-xs font-bold text-amber-300 transition-colors hover:bg-amber-500/20"
                >
                  <SkipForward size={14} />
                  <span>Pular todos os mods sem ID</span>
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
