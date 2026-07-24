import { Download, FolderOpen, Hash, MapPinned, PackageCheck, User, X } from "lucide-react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { getModImageSrc } from "@/lib/modImages"
import { isLocalMod } from "@/lib/modDependencies"
import { isModInCloud } from "@/lib/serverMods"
import { invokeTauri } from "@/lib/tauri"
import type { ZomboidMod } from "@/types/mod"

type ServerModDetailsModalProps = {
  mod: ZomboidMod
  onClose: () => void
  workshopMappings?: Record<string, string>
  onSaveWorkshopMapping?: (modId: string, workshopId: string) => Promise<void>
}

export function ServerModDetailsModal({
  mod,
  onClose,
  workshopMappings = {},
  onSaveWorkshopMapping,
}: ServerModDetailsModalProps) {
  const { t } = useTranslation()
  const dependencies = mod.dependencies ?? []
  const mapNames = mod.mapNames ?? []
  const imageSrc = getModImageSrc(mod.imageUrl)
  const [resolvedSize, setResolvedSize] = useState(mod.size || "-")

  const [isEditing, setIsEditing] = useState(false)
  const resolvedWorkshopId = mod.workshopId?.trim() || workshopMappings[mod.id] || workshopMappings[mod.id.toLowerCase()] || ""
  const [tempWorkshopId, setTempWorkshopId] = useState(resolvedWorkshopId)
  const [isSaving, setIsSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

  // Sincroniza o ID se ele for carregado após a montagem
  useEffect(() => {
    setTempWorkshopId(resolvedWorkshopId)
  }, [resolvedWorkshopId])

  useEffect(() => {
    let isCancelled = false
    const hasKnownSize = Boolean(mod.size && mod.size !== "-")

    setResolvedSize(hasKnownSize ? mod.size : "...")

    if (hasKnownSize || !mod.packagePath) {
      return () => {
        isCancelled = true
      }
    }

    void invokeTauri<string>("get_zomboid_mod_package_size", { packagePath: mod.packagePath })
      .then((size) => {
        if (!isCancelled) {
          setResolvedSize(size || "-")
        }
      })
      .catch(() => {
        if (!isCancelled) {
          setResolvedSize("-")
        }
      })

    return () => {
      isCancelled = true
    }
  }, [mod.packagePath, mod.size])

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md animate-in fade-in duration-300"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="server-mod-details-title"
        className="max-h-[90vh] w-full max-w-3xl overflow-y-auto rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl custom-scrollbar animate-in zoom-in-95 duration-300"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="relative min-h-80 overflow-hidden bg-[#1e2327] sm:h-96">
          {imageSrc ? (
            <>
              <img
                src={imageSrc}
                alt=""
                aria-hidden="true"
                className="absolute inset-0 h-full w-full scale-110 object-cover opacity-25 blur-2xl"
              />
              <div className="absolute inset-x-0 top-0 flex h-[72%] items-center justify-center p-4 sm:h-[76%] sm:p-6">
                <img
                  src={imageSrc}
                  alt={mod.name}
                  className="max-h-full max-w-full rounded-2xl border border-white/10 bg-[#15191c]/60 object-contain shadow-2xl"
                />
              </div>
            </>
          ) : (
            <div className="flex h-full w-full items-center justify-center bg-gradient-to-br from-[#2b3238] to-[#1e2327]">
              <Download size={64} className="text-white/5" />
            </div>
          )}
          <div className="absolute inset-0 bg-gradient-to-t from-[#22272b] via-[#22272b]/30 to-transparent" />
          <button
            type="button"
            aria-label={t("common.close")}
            onClick={onClose}
            className="absolute right-4 top-4 rounded-full border border-white/10 bg-black/50 p-2 text-gray-300 backdrop-blur-md transition-colors hover:bg-black/70 hover:text-white"
          >
            <X size={20} />
          </button>
          <div className="absolute bottom-5 left-6 right-6">
            <div className="mb-2 flex flex-wrap gap-2">
              <SourceBadge mod={mod} />
              {mod.compatibleBuilds.map((build) => <Badge key={build}>{build}</Badge>)}
              {mod.source === "missing" && <Badge tone="red">{t("mods.missing")}</Badge>}
            </div>
            <h3 id="server-mod-details-title" className="text-3xl font-black tracking-tight text-white">{mod.name}</h3>
            <p className="mt-1 flex items-center gap-1.5 text-sm text-gray-300">
              <User size={14} className="text-orange-400" />
              {t("mods.by")} {mod.author}
            </p>
          </div>
        </div>

        <div className="p-6">
          <p className="whitespace-pre-wrap text-sm leading-relaxed text-gray-300">
            {mod.description || t("backend.noDescription")}
          </p>

          <div className="mt-6 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            <Detail label={t("mods.version")} value={mod.version || "-"} />
            <Detail label={t("mods.size")} value={resolvedSize} />
            <Detail label={t("mods.source")} value={mod.source || "-"} />
            
            {isEditing ? (
              <div className="min-w-0 rounded-xl border border-orange-400/25 bg-[#1e2327] p-3 col-span-1 sm:col-span-2">
                <p className="text-[10px] font-bold uppercase tracking-widest text-orange-400">{t("modDetails.workshopEditing")}</p>
                <div className="mt-1 flex items-center gap-2">
                  <input
                    type="text"
                    value={tempWorkshopId}
                    onChange={(e) => setTempWorkshopId(e.target.value.replace(/\D/g, ""))}
                    placeholder={t("modDetails.workshopPlaceholder")}
                    disabled={isSaving}
                    className="flex-1 bg-black/35 border border-white/10 rounded-lg px-2.5 py-1 text-xs text-white font-mono focus:outline-none focus:border-orange-400/50"
                  />
                  <button
                    onClick={async () => {
                      setSaveError(null)
                      const cleanId = tempWorkshopId.trim()
                      if (cleanId && !/^\d+$/.test(cleanId)) {
                        setSaveError(t("modDetails.numbersOnly"))
                        return
                      }
                      setIsSaving(true)
                      try {
                        if (onSaveWorkshopMapping) {
                          await onSaveWorkshopMapping(mod.id, cleanId)
                        }
                        setIsEditing(false)
                      } catch (err) {
                        setSaveError(t("modDetails.saveError"))
                      } finally {
                        setIsSaving(false)
                      }
                    }}
                    disabled={isSaving}
                    className="rounded-lg bg-orange-400 px-3 py-1 text-xs font-bold text-black hover:bg-orange-300 disabled:opacity-50 transition-colors"
                  >
                    {isSaving ? t("fixWorkshopIds.savingShort") : t("common.save")}
                  </button>
                  <button
                    onClick={() => {
                      setTempWorkshopId(resolvedWorkshopId)
                      setIsEditing(false)
                      setSaveError(null)
                    }}
                    disabled={isSaving}
                    className="rounded-lg bg-[#2b3238] border border-white/5 px-2.5 py-1 text-xs font-bold text-gray-300 hover:bg-[#353c42] transition-colors"
                  >
                    {t("common.cancel")}
                  </button>
                </div>
                {saveError && <p className="mt-1 text-[10px] text-red-400">{saveError}</p>}
              </div>
            ) : (
              <div className="min-w-0 rounded-xl border border-white/5 bg-[#1e2327] p-3 flex flex-col justify-between">
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-widest text-gray-500">Workshop ID</p>
                  <p className="mt-1 flex items-center gap-1.5 break-all font-mono text-xs text-gray-300">
                    <Hash size={14} className="text-orange-400 shrink-0" />
                    {resolvedWorkshopId || <span className="text-gray-500 italic">{t("modDetails.notAssociated")}</span>}
                  </p>
                  <div className="mt-2">
                    {isModInCloud(mod, workshopMappings) ? (
                      <span className="inline-flex items-center gap-1 text-[10px] font-bold text-sky-300 bg-sky-500/10 border border-sky-500/20 px-2 py-0.5 rounded-full">
                        {t("modDetails.foundCloud")}
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-[10px] font-bold text-amber-300 bg-amber-500/10 border border-amber-500/20 px-2 py-0.5 rounded-full">
                        {t("modDetails.notFoundCloud")}
                      </span>
                    )}
                  </div>
                </div>
                {onSaveWorkshopMapping && (
                  <button
                    onClick={() => setIsEditing(true)}
                    className="mt-2 text-left text-[10px] font-bold text-orange-400/80 hover:text-orange-400 hover:underline transition-colors"
                  >
                    {resolvedWorkshopId ? t("modDetails.editId") : t("modDetails.linkId")}
                  </button>
                )}
              </div>
            )}

            <Detail label="Mod ID" value={mod.id} icon={<PackageCheck size={14} />} />
            <Detail label={t("mods.path")} value={mod.path || "-"} icon={<FolderOpen size={14} />} />
          </div>

          {dependencies.length > 0 && (
            <DetailList title={t("mods.dependencies")} values={dependencies} />
          )}
          {mapNames.length > 0 && (
            <DetailList title={t("mods.maps")} values={mapNames} icon={<MapPinned size={15} />} />
          )}
        </div>
      </div>
    </div>
  )
}

function SourceBadge({ mod }: { mod: ZomboidMod }) {
  if (isLocalMod(mod)) {
    return <Badge tone="green">LOCAL</Badge>
  }

  if (mod.source === "steamcmd") {
    return <Badge tone="blue">STEAMCMD</Badge>
  }

  return <Badge>STEAM</Badge>
}

function Badge({ children, tone = "orange" }: { children: React.ReactNode; tone?: "orange" | "red" | "green" | "blue" }) {
  const toneClass = {
    blue: "border-sky-400/30 bg-sky-400/15 text-sky-200",
    green: "border-green-400/30 bg-green-400/15 text-green-200",
    orange: "border-orange-400/30 bg-orange-400/15 text-orange-200",
    red: "border-red-500/30 bg-red-500/15 text-red-200",
  }[tone]

  return (
    <span className={`rounded-full border px-2.5 py-1 text-[10px] font-black uppercase tracking-wide ${toneClass}`}>
      {children}
    </span>
  )
}

function Detail({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="min-w-0 rounded-xl border border-white/5 bg-[#1e2327] p-3">
      <p className="text-[10px] font-bold uppercase tracking-widest text-gray-500">{label}</p>
      <p className="mt-1 flex items-center gap-1.5 break-all font-mono text-xs text-gray-300">
        {icon && <span className="shrink-0 text-orange-400">{icon}</span>}
        {value}
      </p>
    </div>
  )
}

function DetailList({ title, values, icon }: { title: string; values: string[]; icon?: React.ReactNode }) {
  return (
    <div className="mt-6">
      <p className="mb-2 flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-widest text-gray-500">
        {icon && <span className="text-orange-400">{icon}</span>}
        {title}
      </p>
      <div className="flex flex-wrap gap-2">
        {values.map((value) => (
          <span key={value} className="rounded-lg border border-white/5 bg-[#1e2327] px-3 py-2 font-mono text-xs text-gray-300">
            {value}
          </span>
        ))}
      </div>
    </div>
  )
}
