import { Download, FolderOpen, Hash, MapPinned, PackageCheck, User, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { isLocalMod } from "@/lib/modDependencies"
import type { ZomboidMod } from "@/types/mod"

type ServerModDetailsModalProps = {
  mod: ZomboidMod
  onClose: () => void
}

export function ServerModDetailsModal({ mod, onClose }: ServerModDetailsModalProps) {
  const { t } = useTranslation()
  const dependencies = mod.dependencies ?? []
  const mapNames = mod.mapNames ?? []

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
          {mod.imageUrl ? (
            <>
              <img
                src={mod.imageUrl}
                alt=""
                aria-hidden="true"
                className="absolute inset-0 h-full w-full scale-110 object-cover opacity-25 blur-2xl"
              />
              <div className="absolute inset-x-0 top-0 flex h-[72%] items-center justify-center p-4 sm:h-[76%] sm:p-6">
                <img
                  src={mod.imageUrl}
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
              <Badge>{isLocalMod(mod) ? "LOCAL" : "STEAM"}</Badge>
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
            <Detail label={t("mods.size")} value={mod.size || "-"} />
            <Detail label={t("mods.source")} value={mod.source || "-"} />
            <Detail label="Workshop ID" value={mod.workshopId || "-"} icon={<Hash size={14} />} />
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

function Badge({ children, tone = "orange" }: { children: React.ReactNode; tone?: "orange" | "red" }) {
  return (
    <span className={`rounded-full border px-2.5 py-1 text-[10px] font-black uppercase tracking-wide ${
      tone === "red"
        ? "border-red-500/30 bg-red-500/15 text-red-200"
        : "border-orange-400/30 bg-orange-400/15 text-orange-200"
    }`}>
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
