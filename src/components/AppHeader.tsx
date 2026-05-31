import { Download, FolderSync, Settings, Search, Bell, RefreshCw } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import type { WorkshopDownloadResult } from "@/types/download"

export type AppNotificationAction =
  | { type: "server-test"; serverId: string }
  | { type: "download-result"; result: WorkshopDownloadResult }

export type AppNotification = {
  id: string
  title: string
  message: string
  tone: "success" | "warning" | "error"
  createdAt: string
  action?: AppNotificationAction
  isRead?: boolean
}

type AppHeaderProps = {
  onScanMods?: () => void
  onInstallAllMods?: () => void
  isInstallingAllMods?: boolean
  showSearch?: boolean
  onOpenSettings?: () => void
  notifications?: AppNotification[]
  onNotificationClick?: (notification: AppNotification) => void
  onMarkAllNotificationsRead?: () => void
  searchQuery: string
  onSearchChange: (value: string) => void
}

export function AppHeader({
  onScanMods,
  onInstallAllMods,
  isInstallingAllMods = false,
  showSearch = true,
  onOpenSettings,
  notifications = [],
  onNotificationClick,
  onMarkAllNotificationsRead,
  searchQuery,
  onSearchChange,
}: AppHeaderProps) {
  const { t, i18n } = useTranslation()
  const [isNotificationsOpen, setIsNotificationsOpen] = useState(false)
  const latestNotification = notifications.find((notification) => !notification.isRead) ?? null
  const unreadCount = notifications.filter((notification) => !notification.isRead).length

  return (
    <header className="sticky top-0 z-10 w-full px-8 py-4 flex items-center justify-between bg-[#22272b]/80 backdrop-blur-md border-b border-white/5">
      {showSearch ? (
        <div className="flex items-center flex-1 max-w-md relative group">
          <Search className="absolute left-3 text-gray-500 group-focus-within:text-orange-400 transition-colors" size={18} />
          <input
            type="text"
            placeholder={t("header.search")}
            value={searchQuery}
            onChange={(event) => onSearchChange(event.target.value)}
            className="w-full bg-[#2b3238] border border-white/5 rounded-xl py-2 pl-10 pr-4 text-sm focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-600"
          />
        </div>
      ) : (
        <div className="flex-1" />
      )}

      <div className="flex items-center gap-3">
        <button
          className="flex items-center gap-2 bg-orange-400/10 text-orange-400 hover:bg-orange-400 hover:text-white px-4 py-2 rounded-xl transition-all duration-300 font-medium text-sm group"
          onClick={onScanMods}
        >
          <FolderSync size={20} className="group-hover:rotate-180 transition-transform duration-500" />
          <span>{t("header.scanMods")}</span>
        </button>

        <button
          disabled={isInstallingAllMods}
          className="flex items-center gap-2 bg-[#2b3238] border border-white/5 text-gray-300 hover:text-white hover:border-orange-400/30 disabled:text-gray-600 disabled:cursor-not-allowed px-4 py-2 rounded-xl transition-all duration-300 font-medium text-sm"
          onClick={onInstallAllMods}
        >
          {isInstallingAllMods ? <RefreshCw size={20} className="animate-spin" /> : <Download size={20} />}
          <span>{t("header.bringSteam")}</span>
        </button>

        <div className="w-[1px] h-8 bg-white/5 mx-2" />

        <div className="relative group/notifications">
          <button
            onClick={() => setIsNotificationsOpen((value) => !value)}
            className="p-2.5 bg-[#2b3238] border border-white/5 text-gray-400 hover:text-white hover:border-white/10 rounded-xl transition-all relative"
          >
            <Bell size={20} />
            {unreadCount > 0 && (
              <span className="absolute -top-1 -right-1 min-w-5 rounded-full border-2 border-[#22272b] bg-orange-500 px-1 text-center text-[10px] font-black text-white">
                {unreadCount}
              </span>
            )}
          </button>

          {latestNotification && (
            <button
              onClick={() => onNotificationClick?.(latestNotification)}
              className={`absolute right-0 top-12 z-30 w-80 rounded-2xl border p-4 text-left shadow-2xl shadow-black/40 transition-all ${
                isNotificationsOpen ? "hidden" : "block"
              } ${
                latestNotification.tone === "success"
                  ? "border-green-500/20 bg-[#1e2a24]"
                  : latestNotification.tone === "error"
                    ? "border-red-500/20 bg-[#2a1e20]"
                    : "border-yellow-500/20 bg-[#2b291e]"
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <p className="text-sm font-black text-white">{latestNotification.title}</p>
                <span className="shrink-0 text-[10px] font-bold text-gray-400">{formatNotificationTime(latestNotification.createdAt, i18n.language)}</span>
              </div>
              <p className="mt-1 line-clamp-2 text-xs text-gray-300">{latestNotification.message}</p>
            </button>
          )}

          {isNotificationsOpen && (
          <div className="absolute right-0 top-12 z-40 w-96 overflow-hidden rounded-2xl border border-white/10 bg-[#1e2327] shadow-2xl shadow-black/50">
            <div className="flex items-center justify-between gap-3 border-b border-white/5 px-4 py-3">
              <p className="text-xs font-black uppercase tracking-widest text-gray-400">{t("header.notifications")}</p>
              {unreadCount > 0 && (
                <button
                  onClick={onMarkAllNotificationsRead}
                  className="text-[11px] font-bold text-orange-400 transition-colors hover:text-orange-300"
                >
                  {t("header.markAllRead")}
                </button>
              )}
            </div>
            <div className="max-h-96 overflow-y-auto custom-scrollbar">
              {notifications.length === 0 ? (
                <p className="px-4 py-6 text-center text-sm text-gray-500">{t("header.noNotifications")}</p>
              ) : (
                notifications.map((notification) => (
                  <button
                    key={notification.id}
                    onClick={() => {
                      onNotificationClick?.(notification)
                      setIsNotificationsOpen(false)
                    }}
                    className={`w-full border-b px-4 py-3 text-left transition-colors ${
                      notification.tone === "success"
                        ? "border-green-500/10 bg-green-500/5 hover:bg-green-500/10"
                        : notification.tone === "error"
                          ? "border-red-500/10 bg-red-500/5 hover:bg-red-500/10"
                          : "border-yellow-500/10 bg-yellow-500/5 hover:bg-yellow-500/10"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <p className="truncate text-sm font-bold text-white">{notification.title}</p>
                          <span className="shrink-0 text-[10px] font-bold text-gray-500">{formatNotificationTime(notification.createdAt, i18n.language)}</span>
                        </div>
                        <p className="mt-1 line-clamp-2 text-xs text-gray-400">{notification.message}</p>
                      </div>
                      {!notification.isRead && (
                        <span className={`mt-1 h-2 w-2 shrink-0 rounded-full ${
                          notification.tone === "success"
                            ? "bg-green-500"
                            : notification.tone === "error"
                              ? "bg-red-500"
                              : "bg-yellow-500"
                        }`} />
                      )}
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>
          )}
        </div>

        <button
          className="p-2.5 bg-[#2b3238] border border-white/5 text-gray-400 hover:text-white hover:border-white/10 rounded-xl transition-all"
          onClick={onOpenSettings}
        >
          <Settings size={20} />
        </button>
      </div>
    </header>
  )
}

function formatNotificationTime(createdAt: string, locale = "en") {
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(createdAt))
}
