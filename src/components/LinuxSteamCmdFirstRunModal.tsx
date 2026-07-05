import { CheckCircle2, Copy, Settings, Terminal, XCircle } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

type LinuxSteamCmdFirstRunModalProps = {
  isOpen: boolean
  resolvedPath: string | null
  isChecking: boolean
  isInstalling: boolean
  installError: string | null
  onCheckAgain: () => void
  onInstall: () => void
  onClose: () => void
  onOpenSettings: () => void
}

const INSTALL_COMMAND = "mkdir -p \"$HOME/.local/share/ZomboidServerModManager\" && curl -fsSL https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz -o /tmp/pzmm-steamcmd-linux.tar.gz && tar -xzf /tmp/pzmm-steamcmd-linux.tar.gz -C \"$HOME/.local/share/ZomboidServerModManager\" && chmod 0755 \"$HOME/.local/share/ZomboidServerModManager/steamcmd.sh\""

export function LinuxSteamCmdFirstRunModal({
  isOpen,
  resolvedPath,
  isChecking,
  isInstalling,
  installError,
  onCheckAgain,
  onInstall,
  onClose,
  onOpenSettings,
}: LinuxSteamCmdFirstRunModalProps) {
  const { t } = useTranslation()
  const [didCopy, setDidCopy] = useState(false)

  if (!isOpen) {
    return null
  }

  async function copyInstallCommand() {
    await navigator.clipboard?.writeText(INSTALL_COMMAND)
    setDidCopy(true)
    window.setTimeout(() => setDidCopy(false), 1600)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 px-4 backdrop-blur-sm">
      <div className="w-full max-w-2xl overflow-hidden rounded-3xl border border-white/10 bg-[#20262b] shadow-2xl">
        <div className="border-b border-white/10 bg-[#272e34] px-6 py-5">
          <div className="flex items-start gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-orange-500/25 bg-orange-500/10 text-orange-300">
              <Terminal size={22} />
            </div>
            <div className="min-w-0 flex-1">
              <h2 className="text-xl font-black text-white">{t("linuxSteamcmd.title")}</h2>
              <p className="mt-1 text-sm text-gray-400">
                {t("linuxSteamcmd.description")}
              </p>
            </div>
          </div>
        </div>

        <div className="space-y-5 px-6 py-5">
          <div className="rounded-2xl border border-white/10 bg-[#171c20] p-4">
            <div className="flex items-start gap-3">
              {resolvedPath ? (
                <CheckCircle2 className="mt-0.5 shrink-0 text-green-400" size={21} />
              ) : (
                <XCircle className="mt-0.5 shrink-0 text-orange-300" size={21} />
              )}
              <div className="min-w-0">
                <p className="text-sm font-bold text-white">
                  {resolvedPath ? t("linuxSteamcmd.found") : t("linuxSteamcmd.missing")}
                </p>
                <p className="mt-1 break-all text-xs text-gray-400">
                  {resolvedPath ?? t("linuxSteamcmd.missingDescription")}
                </p>
              </div>
            </div>
          </div>

          {!resolvedPath && (
            <div className="rounded-2xl border border-orange-500/20 bg-orange-500/10 p-4">
              <p className="text-sm font-bold text-orange-100">{t("linuxSteamcmd.installCommand")}</p>
              <div className="mt-3 flex gap-3 rounded-xl border border-black/30 bg-[#101417] p-3">
                <code className="min-w-0 flex-1 break-all text-xs leading-relaxed text-gray-200">{INSTALL_COMMAND}</code>
                <button
                  type="button"
                  onClick={() => void copyInstallCommand()}
                  className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-white/10 bg-white/5 text-gray-200 transition hover:bg-white/10"
                  title={t("linuxSteamcmd.copyCommand")}
                >
                  <Copy size={16} />
                </button>
              </div>
              {didCopy && <p className="mt-2 text-xs font-bold text-green-300">{t("linuxSteamcmd.copied")}</p>}
              {installError && <p className="mt-3 rounded-xl border border-red-500/20 bg-red-500/10 p-3 text-xs text-red-200">{installError}</p>}
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center justify-end gap-3 border-t border-white/10 bg-[#1b2025] px-6 py-4">
          <button
            type="button"
            onClick={onOpenSettings}
            className="inline-flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2 text-sm font-bold text-gray-200 transition hover:bg-white/10"
          >
            <Settings size={16} />
            {t("linuxSteamcmd.openSettings")}
          </button>
          {!resolvedPath && (
            <button
              type="button"
              onClick={onInstall}
              disabled={isInstalling}
              className="rounded-xl bg-orange-500 px-4 py-2 text-sm font-black text-[#171c20] transition hover:bg-orange-400 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {isInstalling ? t("linuxSteamcmd.installing") : t("linuxSteamcmd.runInstall")}
            </button>
          )}
          <button
            type="button"
            onClick={onCheckAgain}
            disabled={isChecking || isInstalling}
            className="rounded-xl border border-orange-500/30 bg-orange-500/10 px-4 py-2 text-sm font-bold text-orange-100 transition hover:bg-orange-500/20 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {isChecking ? t("linuxSteamcmd.checking") : t("linuxSteamcmd.checkAgain")}
          </button>
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl border border-white/10 px-4 py-2 text-sm font-bold text-gray-200 transition hover:bg-white/10"
          >
            {resolvedPath ? t("linuxSteamcmd.continue") : t("linuxSteamcmd.close")}
          </button>
        </div>
      </div>
    </div>
  )
}
