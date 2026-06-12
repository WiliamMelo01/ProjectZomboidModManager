import { Hash, Save, Server, Users, X } from "lucide-react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { i18n } from "@/i18n"
import type { ZomboidServer } from "@/types/server"

type ServerConfigurationModalProps = {
  isOpen: boolean
  server: ZomboidServer | null
  onClose: () => void
  onSave: (data: { publicName: string; maxPlayers: number; defaultPort: string }) => Promise<void> | void
}

export function ServerConfigurationModal({ isOpen, server, onClose, onSave }: ServerConfigurationModalProps) {
  const { t } = useTranslation()
  const [publicName, setPublicName] = useState("")
  const [maxPlayers, setMaxPlayers] = useState(16)
  const [defaultPort, setDefaultPort] = useState("16261")
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!server || !isOpen) return

    setPublicName(server.name)
    setMaxPlayers(server.maxPlayers || 16)
    setDefaultPort(server.port || "16261")
    setError(null)
  }, [isOpen, server])

  if (!isOpen || !server) return null

  const canSave = publicName.trim().length > 0 && maxPlayers >= 1 && maxPlayers <= 100 && isValidPort(defaultPort)

  const handleSave = async () => {
    if (!canSave) return

    setIsSaving(true)
    setError(null)

    try {
      await onSave({ publicName, maxPlayers, defaultPort })
      onClose()
    } catch (saveError) {
      setError(getErrorMessage(saveError))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="flex max-h-[90vh] w-full max-w-xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="flex items-center justify-between border-b border-white/5 bg-[#2b3238] p-6">
          <div className="flex items-center gap-3">
            <div className="rounded-xl bg-orange-500/10 p-2 text-orange-400">
              <Server size={24} />
            </div>
            <div>
              <h3 className="text-xl font-bold uppercase italic text-white">{t("serverConfig.title")}</h3>
              <p className="text-xs text-gray-400">{t("serverConfig.description")}</p>
            </div>
          </div>
          <button onClick={onClose} className="rounded-full p-2 text-gray-400 transition-colors hover:bg-white/5">
            <X size={20} />
          </button>
        </div>

        <div className="flex-1 space-y-5 overflow-y-auto p-8 custom-scrollbar">
          <div className="space-y-3">
            <label className="ml-1 text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
              {t("serverConfig.publicName")}
            </label>
            <div className="relative group/input">
              <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within/input:text-orange-400">
                <Server size={18} />
              </div>
              <input
                type="text"
                value={publicName}
                onChange={(event) => setPublicName(event.target.value)}
                className="w-full rounded-2xl border border-white/5 bg-[#1e2327] py-4 pl-12 pr-4 text-base transition-all focus:border-orange-400/50 focus:outline-none focus:ring-1 focus:ring-orange-400/20"
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-3">
              <label className="ml-1 text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
                {t("serverConfig.maxPlayers")}
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
                  className="w-full rounded-2xl border border-white/5 bg-[#1e2327] py-4 pl-12 pr-4 text-base transition-all focus:border-orange-400/50 focus:outline-none focus:ring-1 focus:ring-orange-400/20"
                />
              </div>
            </div>

            <div className="space-y-3">
              <label className="ml-1 text-[10px] font-black uppercase tracking-[0.2em] text-gray-500">
                {t("serverConfig.defaultPort")}
              </label>
              <div className="relative group/input">
                <div className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within/input:text-orange-400">
                  <Hash size={18} />
                </div>
                <input
                  type="number"
                  min={1}
                  max={65535}
                  value={defaultPort}
                  onChange={(event) => setDefaultPort(event.target.value)}
                  className="w-full rounded-2xl border border-white/5 bg-[#1e2327] py-4 pl-12 pr-4 text-base transition-all focus:border-orange-400/50 focus:outline-none focus:ring-1 focus:ring-orange-400/20"
                />
              </div>
            </div>
          </div>

          <div className="rounded-2xl border border-orange-400/10 bg-orange-400/5 p-4">
            <p className="text-xs leading-relaxed text-gray-400">{t("serverConfig.hint")}</p>
          </div>

          {error && (
            <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-5 py-4 text-sm text-red-300">
              {error}
            </div>
          )}
        </div>

        <div className="flex items-center justify-between border-t border-white/5 bg-[#2b3238]/50 p-6">
          <button onClick={onClose} className="px-6 py-3 text-sm font-bold text-gray-400 transition-colors hover:text-white">
            {t("serverConfig.skip")}
          </button>
          <button
            disabled={!canSave || isSaving}
            onClick={() => void handleSave()}
            className="flex items-center gap-2 rounded-2xl bg-orange-500 px-8 py-3 font-black uppercase italic tracking-wider text-white shadow-lg shadow-orange-500/20 transition-all hover:bg-orange-600 disabled:bg-gray-700 disabled:text-gray-500"
          >
            <Save size={18} />
            <span>{isSaving ? t("serverConfig.saving") : t("serverConfig.save")}</span>
          </button>
        </div>
      </div>
    </div>
  )
}

function isValidPort(value: string) {
  const port = Number(value)
  return Number.isInteger(port) && port >= 1 && port <= 65535
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

  return i18n.t("serverConfig.saveError")
}
