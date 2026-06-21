import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Box, Check, Loader2, Server, ShieldAlert, X } from "lucide-react"
import { listen } from "@tauri-apps/api/event"

import type { ZomboidServer } from "@/types/server"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import { invokeTauri } from "@/lib/tauri"
import { getErrorMessage } from "@/lib/errors"

type DeployLocalServerModalProps = {
  isOpen: boolean
  connection: RemoteConnectionDraft
  onClose: () => void
  onSuccess: (deployedServerName: string, deployedServerId: string) => void
}

type RemoteServerDeployResult = {
  success: boolean
  serverId: string
  deployedServerFiles: number
  deployedMods: number
  skippedMods: string[]
  localBundlePath: string
  remoteBundlePath: string
  command: string
  stdout: string
  stderr: string
}

type DeployProgressPayload = {
  status: string
  detail: string | null
}

export function DeployLocalServerModal({
  isOpen,
  connection,
  onClose,
  onSuccess,
}: DeployLocalServerModalProps) {
  const { t } = useTranslation()
  const [localServers, setLocalServers] = useState<ZomboidServer[]>([])
  const [selectedServer, setSelectedServer] = useState<ZomboidServer | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [isDeploying, setIsDeploying] = useState(false)
  const [includeMods, setIncludeMods] = useState(true)
  const [overwriteExistingMods, setOverwriteExistingMods] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [deployResult, setDeployResult] = useState<RemoteServerDeployResult | null>(null)
  const [deployedServerName, setDeployedServerName] = useState<string>("")

  const [progressStatus, setProgressStatus] = useState<string>("locating_configs")
  const [progressDetail, setProgressDetail] = useState<string | null>(null)

  useEffect(() => {
    if (!isOpen) return

    async function fetchLocalServers() {
      setIsLoading(true)
      setError(null)
      setDeployResult(null)
      setDeployedServerName("")
      try {
        const serversList = await invokeTauri<ZomboidServer[]>("list_zomboid_servers")
        setLocalServers(serversList)
      } catch (err) {
        setError(getErrorMessage(err))
      } finally {
        setIsLoading(false)
      }
    }

    void fetchLocalServers()
  }, [isOpen])

  useEffect(() => {
    if (!isDeploying) {
      setProgressStatus("locating_configs")
      setProgressDetail(null)
      return
    }

    let unsubscribe: (() => void) | null = null

    listen<DeployProgressPayload>("deploy-progress", (event) => {
      setProgressStatus(event.payload.status)
      setProgressDetail(event.payload.detail || null)
    }).then((unsub) => {
      unsubscribe = unsub
    })

    return () => {
      if (unsubscribe) unsubscribe()
    }
  }, [isDeploying])

  if (!isOpen) return null

  const handleDeploy = async () => {
    if (!selectedServer) return

    setIsDeploying(true)
    setError(null)
    setDeployResult(null)
    setDeployedServerName("")

    try {
      const result = await invokeTauri<RemoteServerDeployResult>("deploy_local_zomboid_server_to_remote", {
        request: {
          connection,
          serverId: selectedServer.id,
          includeMods,
          overwriteExistingMods,
        },
      })

      if (result.success) {
        setProgressStatus("success")
        setProgressDetail(null)
        setDeployResult(result)
        setDeployedServerName(selectedServer.name)
        onSuccess(selectedServer.name, result.serverId)
      } else {
        setError(t("deployLocalServer.failed", "Falha na implantação do servidor."))
      }
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsDeploying(false)
    }
  }

  const getStatusIndex = (status: string) => {
    switch (status) {
      case "locating_configs": return 1
      case "scanning_mods": return 2
      case "staging": return 3
      case "copying_configs": return 4
      case "copying_mods": return 5
      case "compressing": return 6
      case "uploading": return 7
      case "extracting": return 8
      case "success": return 9
      default: return 0
    }
  }

  const stepsList = [
    {
      id: "prep",
      label: t("deployLocalServer.stepPrep", "Preparando arquivos"),
      description: (currentStatus: string) => {
        if (currentStatus === "locating_configs") return t("deployLocalServer.progress.locatingConfigs", "Localizando arquivos de configuração...")
        if (currentStatus === "scanning_mods") return t("deployLocalServer.progress.scanningMods", "Identificando mods locais ativos...")
        if (currentStatus === "staging") return t("deployLocalServer.progress.staging", "Preparando pasta temporária...")
        if (currentStatus === "copying_configs") return t("deployLocalServer.progress.copyingConfigs", "Copiando arquivos de configuração...")
        if (currentStatus === "copying_mods") return `${t("deployLocalServer.progress.copyingMods", "Copiando mods locais...")} ${progressDetail || ""}`
        return null
      },
      minIdx: 1,
      maxIdx: 5,
    },
    {
      id: "compress",
      label: t("deployLocalServer.stepCompress", "Compactando arquivos (Zip)"),
      description: (currentStatus: string) => {
        if (currentStatus === "compressing") {
          return `${t("deployLocalServer.progress.compressing", "Compactando arquivos...")} ${progressDetail || ""}`
        }
        return null
      },
      minIdx: 6,
      maxIdx: 6,
    },
    {
      id: "upload",
      label: t("deployLocalServer.stepUpload", "Transferindo para VM (SCP)"),
      description: (currentStatus: string) => {
        if (currentStatus === "uploading") {
          return `${t("deployLocalServer.progress.uploading", "Enviando arquivo via SCP...")} ${progressDetail ? `(${progressDetail})` : ""}`
        }
        return null
      },
      minIdx: 7,
      maxIdx: 7,
    },
    {
      id: "extract",
      label: t("deployLocalServer.stepExtract", "Extraindo e instalando na VM"),
      description: (currentStatus: string) => {
        if (currentStatus === "extracting") return progressDetail || t("deployLocalServer.progress.extracting", "Executando script remoto de extração...")
        if (currentStatus === "success") return t("deployLocalServer.progress.success", "Implantação concluída com sucesso!")
        return null
      },
      minIdx: 8,
      maxIdx: 9,
    },
  ]

  const currentIdx = getStatusIndex(progressStatus)

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="flex h-full max-h-[80vh] w-full max-w-xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#161a1d] shadow-2xl animate-in zoom-in-95 duration-300">
        
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1c2126] p-6">
          <div className="flex min-w-0 items-center gap-3">
            <div className="rounded-xl bg-orange-500/20 p-2.5 text-orange-500 ring-1 ring-orange-500/20">
              <Server size={24} />
            </div>
            <div className="min-w-0">
              <h3 className="text-xl font-black uppercase italic tracking-tight text-white">
                {t("deployLocalServer.title", "Deploy de Servidor Local")}
              </h3>
              <p className="text-xs font-medium text-gray-500">
                {t("deployLocalServer.subtitle", "Envie um servidor local com seus mods ativos para a VM remota")}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            disabled={isDeploying}
            className="rounded-full bg-white/5 p-2 text-gray-400 transition-all hover:bg-white/10 hover:text-white disabled:opacity-50"
          >
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
          {deployResult ? (
            <div className="flex h-full flex-col justify-center space-y-6 py-4 text-center animate-in fade-in duration-300">
              <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl border border-green-400/30 bg-green-500/15 text-green-300">
                <Check size={34} />
              </div>
              <div>
                <h4 className="text-xl font-black uppercase tracking-tight text-white">
                  {t("deployLocalServer.successTitle", "Deploy concluído")}
                </h4>
                <p className="mt-2 text-sm leading-relaxed text-gray-400">
                  {t("deployLocalServer.successMessage", {
                    name: deployedServerName,
                    defaultValue: `Servidor ${deployedServerName} implantado com sucesso na VM.`,
                  })}
                </p>
              </div>
              <div className="grid gap-3 rounded-2xl border border-white/5 bg-[#1c2126] p-5 text-left">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-gray-400">{t("deployLocalServer.summaryServerFiles", "Arquivos de servidor")}</span>
                  <span className="font-mono font-bold text-white">{deployResult.deployedServerFiles}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-gray-400">{t("deployLocalServer.summaryMods", "Mods enviados")}</span>
                  <span className="font-mono font-bold text-white">{deployResult.deployedMods}</span>
                </div>
                {deployResult.skippedMods.length > 0 && (
                  <div className="rounded-xl border border-yellow-400/20 bg-yellow-500/10 p-3 text-xs text-yellow-100">
                    {t("deployLocalServer.summarySkipped", "Alguns mods não foram encontrados localmente:")} {deployResult.skippedMods.join(", ")}
                  </div>
                )}
              </div>
            </div>
          ) : isDeploying ? (
            <div className="flex h-full flex-col justify-center space-y-8 py-4 animate-in fade-in duration-300">
              <div className="flex flex-col items-center text-center space-y-3">
                <div className="relative flex items-center justify-center">
                  <Loader2 size={40} className="animate-spin text-orange-500" />
                  <Server size={18} className="absolute text-white" />
                </div>
                <div>
                  <h4 className="text-lg font-bold text-white uppercase tracking-tight">
                    {t("deployLocalServer.deployingTitle", "Implantando Servidor...")}
                  </h4>
                  <p className="max-w-md text-xs text-gray-500 leading-relaxed pt-1">
                    {t(
                      "deployLocalServer.deployingDesc",
                      "Compactando os arquivos locais do servidor e transferindo via SCP. Isso pode levar alguns minutos se houver muitos mods."
                    )}
                  </p>
                </div>
              </div>

              {/* Vertical steps progress */}
              <div className="mx-auto max-w-sm w-full space-y-5 bg-[#1c2126]/50 rounded-2xl border border-white/5 p-6">
                {stepsList.map((step, idx) => {
                  const isCompleted = currentIdx > step.maxIdx
                  const isRunning = currentIdx >= step.minIdx && currentIdx <= step.maxIdx
                  const isPending = currentIdx < step.minIdx

                  return (
                    <div key={step.id} className="relative flex gap-4">
                      {/* Left Timeline Line */}
                      {idx < stepsList.length - 1 && (
                        <div
                          className={`absolute left-[15px] top-8 bottom-0 w-0.5 -mb-5 transition-colors duration-500 ${
                            isCompleted ? "bg-green-500" : isRunning ? "bg-orange-500/30" : "bg-white/5"
                          }`}
                        />
                      )}

                      {/* Step Indicator Dot */}
                      <div className="relative z-10 flex h-8 w-8 shrink-0 items-center justify-center rounded-full transition-all duration-300">
                        {isCompleted ? (
                          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-green-500/20 text-green-500 border border-green-500/30">
                            <Check size={16} />
                          </div>
                        ) : isRunning ? (
                          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-orange-500/20 text-orange-500 border border-orange-500 ring-4 ring-orange-500/10 animate-pulse">
                            <Loader2 size={16} className="animate-spin" />
                          </div>
                        ) : (
                          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-white/5 text-gray-600 border border-white/5">
                            <div className="h-2.5 w-2.5 rounded-full bg-gray-600" />
                          </div>
                        )}
                      </div>

                      {/* Step Text */}
                      <div className="flex flex-col pt-1">
                        <span
                          className={`text-sm font-bold transition-colors ${
                            isCompleted ? "text-gray-300" : isRunning ? "text-white" : "text-gray-500"
                          }`}
                        >
                          {step.label}
                        </span>
                        {isRunning && (
                          <span className="mt-1 text-xs font-mono font-semibold text-orange-400 leading-relaxed animate-in fade-in duration-300 max-w-[280px] break-all">
                            {step.description(progressStatus)}
                          </span>
                        )}
                        {isCompleted && (
                          <span className="mt-0.5 text-xs text-gray-500 font-medium">
                            {t("deployLocalServer.stepCompleted", "Concluído")}
                          </span>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          ) : (
            <div className="space-y-6">
              {error && (
                <div className="flex items-start gap-3 rounded-2xl border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-300 animate-in fade-in duration-300">
                  <ShieldAlert size={20} className="shrink-0 text-red-400" />
                  <div className="space-y-1">
                    <span className="font-bold">{t("common.error", "Erro")}</span>
                    <p className="break-words font-mono text-xs">{error}</p>
                  </div>
                </div>
              )}

              {/* Server List */}
              <div className="space-y-2">
                <label className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">
                  {t("deployLocalServer.selectServer", "Selecione o servidor local")}
                </label>
                
                {isLoading ? (
                  <div className="flex items-center justify-center py-12">
                    <Loader2 className="animate-spin text-gray-500" size={32} />
                  </div>
                ) : localServers.length === 0 ? (
                  <div className="rounded-2xl border border-dashed border-white/10 p-8 text-center text-sm text-gray-400">
                    {t("deployLocalServer.noLocalServers", "Nenhum servidor local encontrado.")}
                  </div>
                ) : (
                  <div className="grid grid-cols-1 gap-3 max-h-[30vh] overflow-y-auto pr-2 custom-scrollbar">
                    {localServers.map((server) => {
                      const isSelected = selectedServer?.id === server.id
                      return (
                        <button
                          key={server.id}
                          type="button"
                          onClick={() => setSelectedServer(server)}
                          className={`flex items-center justify-between rounded-2xl border p-4 text-left transition-all ${
                            isSelected
                              ? "border-orange-500 bg-orange-500/5 text-white ring-1 ring-orange-500/30"
                              : "border-white/5 bg-[#1c2126] text-gray-300 hover:border-white/10 hover:text-white"
                          }`}
                        >
                          <div className="flex min-w-0 items-center gap-3">
                            <div className={`rounded-xl p-2.5 transition-colors ${
                              isSelected ? "bg-orange-500/20 text-orange-400" : "bg-white/5 text-gray-400"
                            }`}>
                              <Server size={18} />
                            </div>
                            <div className="min-w-0">
                              <h4 className="font-bold text-sm truncate">{server.name}</h4>
                              <p className="text-xs text-gray-500 font-mono truncate">{server.fileName}</p>
                            </div>
                          </div>

                          <div className="flex items-center gap-3">
                            <div className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2 py-1 text-xs text-gray-400">
                              <Box size={12} />
                              <span>{server.modsCount} mods</span>
                            </div>
                            <span className="text-xs font-bold uppercase text-gray-500 px-2 py-1 bg-white/5 rounded-lg">
                              {server.gameBuild}
                            </span>
                            {isSelected && (
                              <div className="rounded-full bg-orange-500 p-1 text-white">
                                <Check size={14} />
                              </div>
                            )}
                          </div>
                        </button>
                      )
                    })}
                  </div>
                )}
              </div>

              {/* Options */}
              {selectedServer && (
                <div className="rounded-2xl border border-white/5 bg-[#1c2126] p-5 space-y-4 animate-in slide-in-from-bottom-2 duration-300">
                  <h4 className="text-[10px] font-black uppercase tracking-[0.2em] text-gray-400 mb-2">
                    {t("deployLocalServer.options", "Opções de implantação")}
                  </h4>

                  {/* Include Mods */}
                  <label className="flex items-start gap-3 cursor-pointer select-none group">
                    <div className="relative flex items-center pt-0.5">
                      <input
                        type="checkbox"
                        checked={includeMods}
                        onChange={(e) => setIncludeMods(e.target.checked)}
                        className="peer sr-only"
                      />
                      <div className="h-5 w-5 rounded-lg border border-white/10 bg-[#161a1d] transition-all group-hover:border-orange-500/50 peer-checked:border-orange-500 peer-checked:bg-orange-500" />
                      <Check
                        size={14}
                        className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 text-white opacity-0 transition-opacity peer-checked:opacity-100"
                      />
                    </div>
                    <div className="space-y-0.5">
                      <span className="text-sm font-bold text-gray-200 group-hover:text-white">
                        {t("deployLocalServer.optIncludeMods", "Copiar mods ativos instalados localmente")}
                      </span>
                      <p className="text-xs text-gray-500">
                        {t("deployLocalServer.optIncludeModsDesc", "Compacta e envia as pastas dos mods locais ativos para a pasta 'Zomboid/mods' da VM remota.")}
                      </p>
                    </div>
                  </label>

                  {/* Overwrite Existing Mods */}
                  {includeMods && (
                    <label className="flex items-start gap-3 cursor-pointer select-none group animate-in slide-in-from-top-2 duration-200">
                      <div className="relative flex items-center pt-0.5">
                        <input
                          type="checkbox"
                          checked={overwriteExistingMods}
                          onChange={(e) => setOverwriteExistingMods(e.target.checked)}
                          className="peer sr-only"
                        />
                        <div className="h-5 w-5 rounded-lg border border-white/10 bg-[#161a1d] transition-all group-hover:border-orange-500/50 peer-checked:border-orange-500 peer-checked:bg-orange-500" />
                        <Check
                          size={14}
                          className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 text-white opacity-0 transition-opacity peer-checked:opacity-100"
                        />
                      </div>
                      <div className="space-y-0.5">
                        <span className="text-sm font-bold text-gray-200 group-hover:text-white">
                          {t("deployLocalServer.optOverwrite", "Sobrescrever arquivos existentes")}
                        </span>
                        <p className="text-xs text-gray-500">
                          {t("deployLocalServer.optOverwriteDesc", "Substitui arquivos de configurações e mods remotos existentes se houver conflito.")}
                        </p>
                      </div>
                    </label>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 border-t border-white/5 bg-[#1c2126] p-6">
          <button
            type="button"
            onClick={onClose}
            disabled={isDeploying}
            className="rounded-xl bg-[#2b3238] border border-white/5 text-sm font-bold text-gray-300 hover:text-white hover:bg-[#343b42] px-5 py-2.5 transition-all disabled:opacity-50"
          >
            {deployResult ? t("common.close", "Fechar") : t("common.cancel", "Cancelar")}
          </button>
          
          {deployResult ? (
            <button
              type="button"
              onClick={() => {
                setDeployResult(null)
                setDeployedServerName("")
                setSelectedServer(null)
              }}
              className="rounded-xl bg-orange-600 hover:bg-orange-500 text-sm font-bold text-white px-5 py-2.5 transition-all shadow-lg shadow-orange-950/20"
            >
              {t("deployLocalServer.btnDeployAnother", "Fazer outro deploy")}
            </button>
          ) : !isDeploying && (
            <button
              type="button"
              onClick={handleDeploy}
              disabled={!selectedServer}
              className="rounded-xl bg-orange-600 hover:bg-orange-500 text-sm font-bold text-white px-5 py-2.5 transition-all shadow-lg shadow-orange-950/20 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {t("deployLocalServer.btnConfirm", "Confirmar Deploy")}
            </button>
          )}
        </div>

      </div>
    </div>
  )
}
