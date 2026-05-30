import { AlertCircle, AlertTriangle, Check, CheckCircle2, Info, PlusCircle, Trash2, X } from "lucide-react"

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

type DeactivateModModalProps = {
  mod: ZomboidMod
  onCancel: () => void
  onConfirm: () => void
}

export function DeactivateModModal({ mod, onCancel, onConfirm }: DeactivateModModalProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-sm overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300 p-6 text-center">
        <div className="w-16 h-16 bg-red-500/10 text-red-500 rounded-full flex items-center justify-center mx-auto mb-4">
          <Trash2 size={32} />
        </div>
        <h3 className="text-xl font-bold text-white mb-2">Desativar Mod?</h3>
        <p className="text-gray-400 text-sm mb-6">
          Tem certeza que deseja desativar o mod <span className="text-white font-bold">{mod.name}</span> deste servidor?
        </p>
        <div className="flex gap-3">
          <button onClick={onConfirm} className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl transition-all">
            Sim, Desativar
          </button>
          <button onClick={onCancel} className="flex-1 py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all">
            Cancelar
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
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-orange-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 bg-orange-500/10 border-b border-orange-500/10 flex items-center gap-3">
          <AlertTriangle className="text-orange-500" size={28} />
          <h3 className="text-xl font-bold text-white">Alerta de Dependência</h3>
        </div>
        <div className="p-6">
          <p className="text-gray-300 text-sm mb-4 leading-relaxed">
            O mod <span className="text-orange-400 font-bold">{mod.name}</span> não pode ser desativado sozinho pois é uma dependência direta de:
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
              Para remover este mod, você deve primeiro desativar os mods listados acima.
            </p>
          </div>
          <button onClick={onClose} className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20">
            Entendido
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
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-white/10 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 border-b border-white/5 flex justify-between items-center">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-orange-500/20 text-orange-400 rounded-xl">
              <AlertCircle size={24} />
            </div>
            <h3 className="text-xl font-bold text-white">Dependencias pendentes</h3>
          </div>
          <button onClick={onCancel} className="p-2 hover:bg-white/5 rounded-full text-gray-400 transition-colors">
            <X size={20} />
          </button>
        </div>

        <div className="p-6">
          <p className="text-gray-400 text-sm mb-4">
            O mod <span className="text-white font-bold">{activation.mod.name}</span> precisa ser preparado antes de ser ativado:
          </p>
          <div className="space-y-3 mb-6 max-h-56 overflow-y-auto custom-scrollbar pr-2">
            {activation.modNeedsInstall && <ActivationItem mod={activation.mod} action="Trazer" />}
            {activation.dependenciesToActivate.map((dependency) => {
              const willInstall = activation.dependenciesToInstall.some(
                (installDependency) => normalizeModId(installDependency.id) === normalizeModId(dependency.id),
              )

              return <ActivationItem key={dependency.id} mod={dependency} action={willInstall ? "Trazer" : "Ativar"} />
            })}
          </div>
          <div className="flex flex-col gap-3">
            <button onClick={onConfirm} className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 flex items-center justify-center gap-2">
              <CheckCircle2 size={18} />
              Trazer para local e ativar
            </button>
            <button onClick={onCancel} className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all">
              Cancelar
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

function ActivationItem({ mod, action }: { mod: ZomboidMod; action: string }) {
  return (
    <div className="flex items-center gap-3 p-3 bg-[#2b3238] border border-white/5 rounded-xl">
      <div className="w-10 h-10 rounded-lg bg-[#1e2327] overflow-hidden shrink-0">
        {mod.imageUrl ? (
          <img src={mod.imageUrl} alt={mod.name} className="w-full h-full object-cover" />
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
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-in fade-in duration-300">
      <div className="bg-[#22272b] border border-orange-500/20 rounded-3xl w-full max-w-md overflow-hidden shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="p-6 bg-orange-500/10 border-b border-orange-500/10 flex items-center gap-3">
          <AlertTriangle className="text-orange-500" size={28} />
          <h3 className="text-xl font-bold text-white">Aviso de Segurança</h3>
        </div>
        <div className="p-6">
          <p className="text-gray-300 text-sm mb-6 leading-relaxed">
            Alterar a ordem de carregamento pode quebrar o funcionamento de alguns mods. Mova{" "}
            <span className="text-orange-400 font-bold">{request.mod.name}</span> apenas se tiver certeza de que ele deve carregar
            {request.position === "start" ? " no início " : " no final "} da lista.
          </p>
          <button onClick={onConfirm} className="w-full py-3 bg-orange-500 hover:bg-orange-600 text-white font-bold rounded-xl transition-all shadow-lg shadow-orange-500/20 mb-4 flex items-center justify-center gap-2">
            <Check size={18} />
            Confirmar Movimentação
          </button>
          <button onClick={onToggleDontShowAgain} className="mb-4 flex items-center gap-2 text-left group">
            <span className={`flex h-5 w-5 items-center justify-center rounded border transition-all ${
              dontShowAgain ? "border-orange-500 bg-orange-500" : "border-white/20 bg-transparent group-hover:border-white/40"
            }`}>
              {dontShowAgain && <Check size={12} className="text-white" />}
            </span>
            <span className="text-xs text-gray-400 transition-colors group-hover:text-gray-300">
              Não mostrar este alerta novamente
            </span>
          </button>
          <button onClick={onCancel} className="w-full py-3 bg-transparent border border-white/10 text-gray-400 hover:text-white hover:bg-white/5 font-bold rounded-xl transition-all">
            Cancelar
          </button>
        </div>
      </div>
    </div>
  )
}
