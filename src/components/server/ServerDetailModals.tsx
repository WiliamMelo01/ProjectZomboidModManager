import { AlertCircle, AlertTriangle, Check, CheckCircle2, Info, MapPinned, PlusCircle, Trash2, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { getModImageSrc } from "@/lib/modImages"
import { normalizeModId } from "@/lib/modDependencies"
import type { ZomboidMod } from "@/types/mod"

export type PendingActivation = {
  mod: ZomboidMod
  dependenciesToInstall: ZomboidMod[]
  dependenciesToActivate: ZomboidMod[]
  modNeedsInstall: boolean
}

export type MoveModRequest = {
  mod: ZomboidMod
  position: "start" | "end"
}

type IncompatibleModsModalProps = {
  gameBuild: "b41" | "b42"
  mods: {
    id: string
    name: string
    compatibleBuilds: string[]
    isInLibrary: boolean
  }[]
  onClose: () => void
}

export function IncompatibleModsModal({ gameBuild, mods, onClose }: IncompatibleModsModalProps) {
  const { t } = useTranslation()
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md animate-in fade-in duration-300">
      <div className="w-full max-w-lg overflow-hidden rounded-3xl border border-red-500/20 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="flex items-center gap-3 border-b border-red-500/10 bg-red-500/10 p-6">
          <AlertTriangle className="text-red-400" size={28} />
          <div>
            <h3 className="text-xl font-bold text-white">{t("modals.incompatibleTitle")}</h3>
            <p className="mt-1 text-xs text-gray-400">{t("modals.incompatibleDescription", { build: gameBuild.toUpperCase() })}</p>
          </div>
        </div>
        <div className="p-6">
          <div className="mb-6 max-h-72 space-y-2 overflow-y-auto pr-2 custom-scrollbar">
            {mods.map((mod) => (
              <div key={mod.id} className="rounded-xl border border-red-500/10 bg-red-500/5 p-3">
                <p className="text-sm font-bold text-white">{mod.name}</p>
                {normalizeModId(mod.name) !== normalizeModId(mod.id) && (
                  <p className="mt-1 break-all font-mono text-[10px] text-red-300">ID: {mod.id}</p>
                )}
                <p className="mt-2 text-xs text-gray-400">
                  {mod.isInLibrary
                    ? t("modals.compatibleOnly", { builds: mod.compatibleBuilds.map((build) => build.toUpperCase()).join(", ") })
                    : t("modals.missingLibrary")}
                </p>
              </div>
            ))}
          </div>
          <button onClick={onClose} className="w-full rounded-xl bg-red-500 py-3 font-bold text-white transition-all hover:bg-red-600">
            {t("modals.understood")}
          </button>
        </div>
      </div>
    </div>
  )
}

type ChangeServerBuildModalProps = {
  currentBuild: "b41" | "b42"
  nextBuild: "b41" | "b42"
  activeModsCount: number
  isSaving: boolean
  onCancel: () => void
  onConfirm: () => void
}

export function ChangeServerBuildModal({
  currentBuild,
  nextBuild,
  activeModsCount,
  isSaving,
  onCancel,
  onConfirm,
}: ChangeServerBuildModalProps) {
  const { t } = useTranslation()
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md animate-in fade-in duration-300">
      <div className="w-full max-w-md overflow-hidden rounded-3xl border border-orange-500/20 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="flex items-center gap-3 border-b border-orange-500/10 bg-orange-500/10 p-6">
          <AlertTriangle className="text-orange-400" size={28} />
          <div>
            <h3 className="text-xl font-bold text-white">{t("modals.changeBuildTitle")}</h3>
            <p className="mt-1 text-xs text-gray-400">{t("modals.reviewImpact")}</p>
          </div>
        </div>
        <div className="p-6">
          <div className="mb-5 flex items-center justify-center gap-3">
            <BuildBadge build={currentBuild} muted />
            <span className="text-gray-600">→</span>
            <BuildBadge build={nextBuild} />
          </div>
          <p className="mb-4 text-sm leading-relaxed text-gray-300">
            {t("modals.changeBuildBody", { build: nextBuild.toUpperCase() })}
          </p>
          {activeModsCount > 0 && (
            <div className="mb-6 rounded-2xl border border-orange-500/20 bg-orange-500/10 p-4 text-sm text-orange-200">
              {t("modals.activeModsReview", { count: activeModsCount })}
            </div>
          )}
          <div className="flex gap-3">
            <button
              onClick={onConfirm}
              disabled={isSaving}
              className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-orange-500 py-3 font-bold text-white transition-all hover:bg-orange-600 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {isSaving ? t("modals.changing") : t("modals.useBuild", { build: nextBuild.toUpperCase() })}
            </button>
            <button
              onClick={onCancel}
              disabled={isSaving}
              className="flex-1 rounded-xl border border-white/10 bg-transparent py-3 font-bold text-gray-400 transition-all hover:bg-white/5 hover:text-white disabled:opacity-60"
            >
              {t("modals.cancel")}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

function BuildBadge({ build, muted = false }: { build: "b41" | "b42"; muted?: boolean }) {
  return (
    <span className={`rounded-full border px-4 py-2 text-sm font-black uppercase ${
      muted ? "border-white/10 bg-white/5 text-gray-500" : "border-orange-400/30 bg-orange-400/10 text-orange-300"
    }`}>
      {build}
    </span>
  )
}

type MapInstallConfirmationModalProps = {
  mod: ZomboidMod
  onCancel: () => void
  onConfirm: () => void
}

export function MapInstallConfirmationModal({ mod, onCancel, onConfirm }: MapInstallConfirmationModalProps) {
  const { t } = useTranslation()
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-md animate-in fade-in duration-300">
      <div className="w-full max-w-md overflow-hidden rounded-3xl border border-orange-500/20 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="flex items-center gap-3 border-b border-orange-500/10 bg-orange-500/10 p-6">
          <MapPinned className="text-orange-400" size={28} />
          <h3 className="text-xl font-bold text-white">{t("modals.mapTitle")}</h3>
        </div>
        <div className="p-6">
          <p className="mb-4 text-sm leading-relaxed text-gray-300">
            {t("modals.mapBody", { name: mod.name })}
          </p>
          <div className="mb-6 rounded-2xl border border-white/5 bg-[#1e2327] p-4">
            <p className="mb-2 text-[10px] font-bold uppercase tracking-widest text-gray-500">{t("modals.mapsFound")}</p>
            <p className="text-sm text-white">{mod.mapNames?.join(", ")}</p>
          </div>
          <div className="flex gap-3">
            <button onClick={onConfirm} className="flex-1 rounded-xl bg-orange-500 py-3 font-bold text-white transition-all hover:bg-orange-600">
              {t("modals.addMap")}
            </button>
            <button onClick={onCancel} className="flex-1 rounded-xl border border-white/10 bg-transparent py-3 font-bold text-gray-400 transition-all hover:bg-white/5 hover:text-white">
              {t("common.cancel")}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

type DeactivateModModalProps = {
  mod: ZomboidMod
  onCancel: () => void
  onConfirm: () => void
}

export function DeactivateModModal({ mod, onCancel, onConfirm }: DeactivateModModalProps) {
  const { t } = useTranslation()
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-sm overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300 p-6 text-center">
        <div className="w-16 h-16 bg-red-500/10 text-red-500 rounded-full flex items-center justify-center mx-auto mb-4">
          <Trash2 size={32} />
        </div>
        <h3 className="text-xl font-bold text-white mb-2">{t("modals.deactivateTitle")}</h3>
        <p className="text-gray-400 text-sm mb-6">
          {t("modals.deactivateBody", { name: mod.name })}
        </p>
        <div className="flex gap-3">
          <button onClick={onConfirm} className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl transition-all">
            {t("modals.yesDeactivate")}
          </button>
          <button onClick={onCancel} className="flex-1 py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all">
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </div>
  )
}

type DependencyWarningModalProps = {
  mod: ZomboidMod
  dependents: ZomboidMod[]
  onClose: () => void
}

export function DependencyWarningModal({ mod, dependents, onClose }: DependencyWarningModalProps) {
  const { t } = useTranslation()
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-orange-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 bg-orange-500/10 border-b border-orange-500/10 flex items-center gap-3">
          <AlertTriangle className="text-orange-500" size={28} />
          <h3 className="text-xl font-bold text-white">{t("modals.dependencyWarning")}</h3>
        </div>
        <div className="p-6">
          <p className="text-gray-300 text-sm mb-4 leading-relaxed">
            {t("modals.dependencyBody", { name: mod.name })}
          </p>
          <div className="space-y-2 mb-6">
            {dependents.map((dependent) => (
              <div key={dependent.id} className="flex items-center gap-2 p-3 bg-[#1e2327] rounded-xl border border-white/5">
                <div className="w-2 h-2 rounded-full bg-orange-500" />
                <span className="text-sm font-medium text-white">{dependent.name}</span>
              </div>
            ))}
          </div>
          <div className="p-4 bg-orange-500/5 rounded-2xl border border-orange-500/10 flex gap-3 mb-6">
            <Info size={20} className="text-orange-400 shrink-0 mt-0.5" />
            <p className="text-[11px] text-gray-400 italic">
              {t("modals.dependencyHint")}
            </p>
          </div>
          <button onClick={onClose} className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20">
            {t("modals.understood")}
          </button>
        </div>
      </div>
    </div>
  )
}

type PendingActivationModalProps = {
  activation: PendingActivation
  onCancel: () => void
  onConfirm: () => void
}

export function PendingActivationModal({ activation, onCancel, onConfirm }: PendingActivationModalProps) {
  const { t } = useTranslation()

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 border-b border-white/5 flex justify-between items-center">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-orange-500/20 text-orange-400 rounded-xl">
              <AlertCircle size={24} />
            </div>
            <h3 className="text-xl font-bold text-white">{t("modals.pendingDependencies")}</h3>
          </div>
          <button onClick={onCancel} className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors">
            <X size={20} />
          </button>
        </div>

        <div className="p-6">
          <p className="text-gray-400 text-sm mb-4">
            {t("modals.pendingBody", { name: activation.mod.name })}
          </p>
          <div className="space-y-3 mb-6 max-h-56 overflow-y-auto custom-scrollbar pr-2">
            {activation.modNeedsInstall && <ActivationItem mod={activation.mod} action={t("modals.bring")} />}
            {activation.dependenciesToActivate.map((dependency) => {
              const willInstall = activation.dependenciesToInstall.some(
                (installDependency) => normalizeModId(installDependency.id) === normalizeModId(dependency.id),
              )

              return <ActivationItem key={dependency.id} mod={dependency} action={willInstall ? t("modals.bring") : t("modals.activate")} />
            })}
          </div>
          <div className="flex flex-col gap-3">
            <button onClick={onConfirm} className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 flex items-center justify-center gap-2">
              <CheckCircle2 size={18} />
              {t("modals.prepareAndActivate")}
            </button>
            <button onClick={onCancel} className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all">
              {t("common.cancel")}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

function ActivationItem({ mod, action }: { mod: ZomboidMod; action: string }) {
  const imageSrc = getModImageSrc(mod.imageUrl)

  return (
    <div className="flex items-center gap-3 p-3 bg-[#2b3238] border border-white/5 rounded-xl">
      <div className="w-10 h-10 rounded-lg bg-[#1e2327] overflow-hidden shrink-0">
        {imageSrc ? (
          <img src={imageSrc} alt={mod.name} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-white/10">
            <PlusCircle size={16} />
          </div>
        )}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm font-bold text-white truncate">{mod.name}</p>
        <p className="text-[10px] text-gray-500 font-mono truncate">{mod.id}</p>
      </div>
      <span className="text-[10px] font-bold text-orange-300 bg-orange-500/10 border border-orange-500/10 rounded-full px-2 py-0.5 shrink-0">
        {action}
      </span>
    </div>
  )
}

type MoveModWarningModalProps = {
  request: MoveModRequest
  dontShowAgain: boolean
  onToggleDontShowAgain: () => void
  onCancel: () => void
  onConfirm: () => void
}

export function MoveModWarningModal({ request, dontShowAgain, onToggleDontShowAgain, onCancel, onConfirm }: MoveModWarningModalProps) {
  const { t } = useTranslation()

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-orange-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 bg-orange-500/10 border-b border-orange-500/10 flex items-center gap-3">
          <AlertTriangle className="text-orange-500" size={28} />
          <h3 className="text-xl font-bold text-white">{t("modals.securityWarning")}</h3>
        </div>
        <div className="p-6">
          <p className="text-gray-300 text-sm mb-6 leading-relaxed">
            {t("modals.moveBody", {
              name: request.mod.name,
              position: t(request.position === "start" ? "modals.atStart" : "modals.atEnd"),
            })}
          </p>
          <button onClick={onConfirm} className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 mb-4 flex items-center justify-center gap-2">
            <Check size={18} />
            {t("modals.confirmMove")}
          </button>
          <button onClick={onToggleDontShowAgain} className="mb-4 flex items-center gap-2 text-left group">
            <span className={`flex h-5 w-5 items-center justify-center rounded border transition-all ${
              dontShowAgain ? "border-orange-500 bg-orange-500" : "border-white/20 bg-transparent group-hover:border-white/40"
            }`}>
              {dontShowAgain && <Check size={12} className="text-white" />}
            </span>
            <span className="text-xs text-gray-400 transition-colors group-hover:text-gray-300">
              {t("modals.dontShowAgain")}
            </span>
          </button>
          <button onClick={onCancel} className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all">
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </div>
  )
}
