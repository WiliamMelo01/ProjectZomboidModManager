import { useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"
import { AlertCircle, Box, Check, CheckCircle2, Loader2, Search, Upload, X, Minus, Maximize2 } from "lucide-react"

import type { ZomboidMod } from "@/types/mod"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import { invokeTauri } from "@/lib/tauri"
import { getErrorMessage } from "@/lib/errors"

type UploadLocalModsModalProps = {
  isOpen: boolean
  connection: RemoteConnectionDraft
  onClose: () => void
  onSuccess: () => void
}

type FailedModUpload = {
  name: string
  error: string
}

export function UploadLocalModsModal({
  isOpen,
  connection,
  onClose,
  onSuccess,
}: UploadLocalModsModalProps) {
  const { t } = useTranslation()
  const [searchQuery, setSearchQuery] = useState("")
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [isUploading, setIsUploading] = useState(false)
  const [currentUploadingMod, setCurrentUploadingMod] = useState<ZomboidMod | null>(null)
  const [completedCount, setCompletedCount] = useState(0)
  const [failedMods, setFailedMods] = useState<FailedModUpload[]>([])
  const [uploadStarted, setUploadStarted] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [isMinimized, setIsMinimized] = useState(false)
  const [modStatuses, setModStatuses] = useState<Record<string, { status: 'pending' | 'uploading' | 'success' | 'failed', error?: string }>>({})

  const [localMods, setLocalMods] = useState<ZomboidMod[]>([])
  const [isLoadingLocalMods, setIsLoadingLocalMods] = useState(false)

  // Reset state and load local mods on open
  useEffect(() => {
    if (isOpen) {
      setSearchQuery("")
      setSelectedIds(new Set())
      setIsUploading(false)
      setCurrentUploadingMod(null)
      setCompletedCount(0)
      setFailedMods([])
      setUploadStarted(false)
      setError(null)
      setLocalMods([])
      setIsMinimized(false)
      setModStatuses({})

      setIsLoadingLocalMods(true)
      invokeTauri<ZomboidMod[]>("list_zomboid_mods")
        .then((fetchedMods) => {
          setLocalMods(fetchedMods)
        })
        .catch((err) => {
          setError(getErrorMessage(err))
        })
        .finally(() => {
          setIsLoadingLocalMods(false)
        })
    }
  }, [isOpen])

  const filteredMods = useMemo(() => {
    const query = searchQuery.trim().toLowerCase()
    if (!query) return localMods
    return localMods.filter(
      (mod) =>
        mod.name.toLowerCase().includes(query) ||
        mod.id.toLowerCase().includes(query) ||
        (mod.author && mod.author.toLowerCase().includes(query))
    )
  }, [localMods, searchQuery])

  const handleToggleSelect = (modId: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(modId)) {
        next.delete(modId)
      } else {
        next.add(modId)
      }
      return next
    })
  }

  const handleSelectAll = () => {
    setSelectedIds(new Set(filteredMods.map((mod) => mod.id)))
  }

  const handleDeselectAll = () => {
    setSelectedIds(new Set())
  }

  const handleStartUpload = async () => {
    const selectedMods = localMods.filter((mod) => selectedIds.has(mod.id))
    if (selectedMods.length === 0) return

    setIsUploading(true)
    setUploadStarted(true)
    setCompletedCount(0)
    setFailedMods([])
    setError(null)

    const initialStatuses: Record<string, { status: 'pending' | 'uploading' | 'success' | 'failed' }> = {}
    selectedMods.forEach((mod) => {
      initialStatuses[mod.id] = { status: 'pending' }
    })
    setModStatuses(initialStatuses)

    for (let i = 0; i < selectedMods.length; i++) {
      const mod = selectedMods[i]
      setCurrentUploadingMod(mod)
      setModStatuses((prev) => ({
        ...prev,
        [mod.id]: { status: 'uploading' },
      }))

      try {
        await invokeTauri("upload_local_mod_to_remote", {
          connection,
          modId: mod.id,
          workshopId: mod.workshopId,
          localModPath: mod.packagePath,
        })
        setCompletedCount((prev) => prev + 1)
        setModStatuses((prev) => ({
          ...prev,
          [mod.id]: { status: 'success' },
        }))
      } catch (err) {
        const errorMsg = getErrorMessage(err)
        setFailedMods((prev) => [...prev, { name: mod.name, error: errorMsg }])
        setModStatuses((prev) => ({
          ...prev,
          [mod.id]: { status: 'failed', error: errorMsg },
        }))
      }
    }

    setIsUploading(false)
    setCurrentUploadingMod(null)
    onSuccess()
  }

  if (!isOpen) return null

  const selectedCount = selectedIds.size
  const totalToUpload = selectedIds.size
  const progressPercent = totalToUpload > 0 ? Math.round(((completedCount + failedMods.length) / totalToUpload) * 100) : 0

  if (isMinimized) {
    return (
      <div className="fixed bottom-6 right-6 z-50 w-80 overflow-hidden rounded-2xl border border-white/10 bg-[#22272b] shadow-2xl animate-in slide-in-from-bottom duration-300">
        <div className="flex items-center justify-between bg-[#1e2327] px-4 py-3 border-b border-white/5">
          <div className="flex items-center gap-2 min-w-0">
            <Loader2 size={14} className="animate-spin text-orange-500 shrink-0" />
            <span className="text-xs font-bold text-white truncate">
              {t("mods.uploading", "Enviando mods...")} ({progressPercent}% ({completedCount + failedMods.length}/{totalToUpload}))
            </span>
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => setIsMinimized(false)}
              className="rounded-lg bg-white/5 p-1.5 text-gray-400 transition-colors hover:bg-white/10 hover:text-white"
              title={t("common.expand")}
            >
              <Maximize2 size={12} />
            </button>
          </div>
        </div>
        <div className="p-4 space-y-3">
          {currentUploadingMod && (
            <p className="text-[10px] text-gray-400 font-mono truncate">
              {t("common.current", "Atual")}: {currentUploadingMod.name}
            </p>
          )}
          <div className="space-y-1">
            <div className="h-1.5 w-full rounded-full bg-white/5 overflow-hidden">
              <div
                className="h-full bg-orange-500 transition-all duration-500"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md animate-in fade-in duration-300">
      <div className="flex max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden rounded-3xl border border-white/5 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300">
        
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1e2327] px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="rounded-xl bg-orange-500/10 p-2 text-orange-400">
              <Upload size={22} />
            </div>
            <div>
              <h2 className="text-lg font-black text-white">
                {t("serverDetail.uploadLocalMods", "Enviar mods locais")}
              </h2>
              <p className="text-xs text-gray-500">
                {connection.username}@{connection.host}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {isUploading && (
              <button
                type="button"
                onClick={() => setIsMinimized(true)}
                className="rounded-full bg-white/5 p-2 text-gray-400 transition-colors hover:bg-white/10 hover:text-white"
                title={t("common.minimize")}
              >
                <Minus size={18} />
              </button>
            )}
            <button
              type="button"
              onClick={onClose}
              disabled={isUploading}
              className="rounded-full bg-white/5 p-2 text-gray-400 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        {/* Content */}
        {!uploadStarted ? (
          // Selection view
          <div className="flex flex-col min-h-0 flex-1 p-6">
            <div className="flex flex-col sm:flex-row gap-3 items-stretch sm:items-center justify-between mb-4">
              {/* Search */}
              <div className="relative flex-1 group">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500 group-focus-within:text-orange-400 transition-colors" size={16} />
                <input
                  type="text"
                  placeholder={t("mods.search")}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full bg-[#1c2126] border border-white/5 rounded-xl py-2 pl-9 pr-4 text-xs focus:outline-none focus:border-orange-400/50 transition-all placeholder:text-gray-600"
                />
              </div>

              {/* Bulk select buttons */}
              <div className="flex items-center gap-2 text-xs">
                <button
                  type="button"
                  onClick={handleSelectAll}
                  className="rounded-lg border border-white/5 bg-[#2b3238] px-3 py-1.5 font-bold text-gray-300 hover:text-white hover:border-orange-400/30 transition-colors"
                >
                  {t("common.selectAll", "Selecionar Todos")}
                </button>
                <button
                  type="button"
                  onClick={handleDeselectAll}
                  className="rounded-lg border border-white/5 bg-[#2b3238] px-3 py-1.5 font-bold text-gray-300 hover:text-white hover:border-orange-400/30 transition-colors"
                >
                  {t("common.deselectAll", "Limpar")}
                </button>
              </div>
            </div>

            {/* List */}
            <div className="flex-1 overflow-y-auto custom-scrollbar border border-white/5 rounded-2xl bg-[#1c2126]/40 p-2 space-y-1.5">
              {isLoadingLocalMods ? (
                <div className="flex flex-col items-center justify-center py-16 text-gray-500">
                  <Loader2 size={36} className="mb-2 animate-spin text-orange-500" />
                  <p className="text-sm font-bold">{t("library.loading", "Carregando mods...")}</p>
                </div>
              ) : filteredMods.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-16 text-gray-500">
                  <Box size={36} className="mb-2 opacity-25" />
                  <p className="text-sm font-bold">{t("mods.noResults", "Nenhum mod encontrado.")}</p>
                </div>
              ) : (
                filteredMods.map((mod) => {
                  const isSelected = selectedIds.has(mod.id)
                  return (
                    <button
                      key={mod.id}
                      type="button"
                      onClick={() => handleToggleSelect(mod.id)}
                      className={`flex w-full items-center justify-between rounded-xl border p-3 text-left transition-all ${
                        isSelected
                          ? "border-orange-500/30 bg-orange-500/5 hover:bg-orange-500/10"
                          : "border-transparent bg-white/5 hover:bg-white/10"
                      }`}
                    >
                      <div className="min-w-0 pr-4">
                        <h4 className="font-bold text-xs text-white truncate">{mod.name}</h4>
                        <p className="text-[10px] text-gray-500 font-mono truncate mt-0.5">ID: {mod.id}</p>
                      </div>
                      <div className="flex items-center gap-3 shrink-0">
                        <span className="text-[10px] text-gray-500 font-mono bg-black/40 px-2 py-0.5 rounded border border-white/5">
                          {mod.size}
                        </span>
                        <div
                          className={`flex h-5 w-5 items-center justify-center rounded-lg border transition-all ${
                            isSelected
                              ? "border-orange-500 bg-orange-500 text-white"
                              : "border-white/10 bg-[#161a1d]"
                          }`}
                        >
                          {isSelected && <Check size={14} />}
                        </div>
                      </div>
                    </button>
                  )
                })
              )}
            </div>

            {/* Footer actions */}
            <div className="flex items-center justify-between border-t border-white/5 mt-6 pt-4">
              <span className="text-xs text-gray-400 font-bold">
                {selectedCount} {t("serverDetail.selected", "selecionado(s)")}
              </span>
              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={onClose}
                  className="rounded-xl border border-white/10 px-5 py-2.5 text-xs font-bold text-gray-300 hover:text-white hover:bg-white/5 transition-colors"
                >
                  {t("common.cancel", "Cancelar")}
                </button>
                <button
                  type="button"
                  onClick={handleStartUpload}
                  disabled={selectedCount === 0}
                  className="flex items-center gap-2 rounded-xl bg-orange-500 px-6 py-2.5 text-xs font-black text-white hover:bg-orange-600 transition-colors disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
                >
                  <Upload size={14} />
                  <span>{t("common.upload", "Upload")}</span>
                </button>
              </div>
            </div>
          </div>
        ) : (
          // Uploading progress view
          <div className="flex flex-col min-h-0 flex-1 p-6 space-y-6">
            <div className="flex items-center gap-4 bg-[#1c2126]/50 rounded-2xl border border-white/5 p-5 shrink-0">
              <div className="relative flex h-10 w-10 items-center justify-center rounded-full bg-orange-500/10 text-orange-400">
                {isUploading ? (
                  <Loader2 size={20} className="animate-spin text-orange-500" />
                ) : (
                  <CheckCircle2 size={20} className="text-green-400" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <h4 className="font-bold text-sm text-white">
                  {isUploading
                    ? t("mods.uploading", "Enviando mods para o servidor...")
                    : t("mods.uploadComplete", "Upload Concluído!")}
                </h4>
                {currentUploadingMod && (
                  <p className="text-xs text-gray-500 font-mono truncate mt-0.5">
                    {t("common.current", "Atual")}: {currentUploadingMod.name} ({currentUploadingMod.size})
                  </p>
                )}
              </div>
            </div>

            {/* Progress bar */}
            <div className="space-y-2 shrink-0">
              <div className="flex items-center justify-between text-xs font-bold font-mono">
                <span className="text-gray-400">
                  {completedCount + failedMods.length} / {totalToUpload} {t("mods.completed", "concluídos")}
                </span>
                <span className="text-orange-400">{progressPercent}%</span>
              </div>
              <div className="h-2 w-full rounded-full bg-white/5 overflow-hidden">
                <div
                  className="h-full bg-orange-500 transition-all duration-500"
                  style={{ width: `${progressPercent}%` }}
                />
              </div>
            </div>

            {/* Detailed upload status list */}
            <div className="flex-1 overflow-y-auto max-h-60 custom-scrollbar border border-white/5 rounded-2xl bg-[#1c2126]/40 p-3 space-y-2">
              {localMods
                .filter((mod) => selectedIds.has(mod.id))
                .map((mod) => {
                  const state = modStatuses[mod.id] || { status: 'pending' }
                  return (
                    <div key={mod.id} className="flex items-center justify-between text-xs p-2.5 rounded-xl bg-[#2b3238]">
                      <div className="min-w-0 flex-1 pr-4">
                        <span className="font-bold text-white block truncate">{mod.name}</span>
                        {state.status === 'failed' && state.error && (
                          <span className="text-[10px] text-red-400 block mt-0.5 truncate">{state.error}</span>
                        )}
                      </div>
                      <div className="flex items-center gap-2 shrink-0">
                        {state.status === 'pending' && <span className="text-gray-500 font-bold uppercase tracking-wider text-[10px]">{t("common.pending")}</span>}
                        {state.status === 'uploading' && (
                          <span className="text-orange-400 flex items-center gap-1 font-bold uppercase tracking-wider text-[10px]">
                            <Loader2 size={12} className="animate-spin" />
                            Enviando
                          </span>
                        )}
                        {state.status === 'success' && (
                          <span className="text-green-400 flex items-center gap-1 font-bold uppercase tracking-wider text-[10px]">
                            <Check size={12} />
                            Sucesso
                          </span>
                        )}
                        {state.status === 'failed' && (
                          <span className="text-red-400 flex items-center gap-1 font-bold uppercase tracking-wider text-[10px]">
                            <X size={12} />
                            {t("common.error")}
                          </span>
                        )}
                      </div>
                    </div>
                  )
                })}
            </div>

            {/* Error alerts summary */}
            {failedMods.length > 0 && (
              <div className="space-y-2 shrink-0">
                <h5 className="text-[10px] font-black uppercase tracking-wider text-red-400 flex items-center gap-1.5">
                  <AlertCircle size={12} />
                  <span>{t("common.failures", "Falhas no envio")}</span>
                </h5>
                <div className="space-y-1.5">
                  {failedMods.map((item, idx) => (
                    <div key={idx} className="rounded-xl border border-red-500/10 bg-red-500/5 p-3 text-xs leading-relaxed text-red-300">
                      <span className="font-bold">{item.name}: </span>
                      <span>{item.error}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Result summary */}
            {completedCount > 0 && !isUploading && (
              <div className="rounded-xl border border-green-500/10 bg-green-500/5 p-4 text-xs text-green-300 flex items-center gap-2.5 shrink-0">
                <CheckCircle2 size={16} />
                <span>
                  {completedCount} {t("mods.uploadedSuccessfully", "mods enviados com sucesso!")}
                </span>
              </div>
            )}

            {/* Footer close */}
            <div className="flex justify-end pt-2 shrink-0">
              <button
                type="button"
                onClick={onClose}
                disabled={isUploading}
                className="rounded-xl bg-[#2b3238] border border-white/10 px-6 py-2.5 text-xs font-bold text-gray-300 hover:text-white transition-colors disabled:cursor-not-allowed disabled:opacity-40"
              >
                {t("common.close", "Fechar")}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
