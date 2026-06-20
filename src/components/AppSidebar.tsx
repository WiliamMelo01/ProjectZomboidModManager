import { Network } from "lucide-react"
import { AppSidebarItem, type SidebarItem } from "@/components/AppSidebarItem"
import { Logo } from "@/components/Logo"
import { useTranslation } from "react-i18next"
import packageMetadata from "../../package.json"

type AppSidebarProps = {
  activeTab: string
  items: SidebarItem[]
  onTabChange: (tabId: string) => void
  onChangeWorkspace: () => void
}

export function AppSidebar({ activeTab, items, onTabChange, onChangeWorkspace }: AppSidebarProps) {
  const { t } = useTranslation()
  const appChannel = formatAppChannel(packageMetadata.appChannel ?? "beta")

  return (
    <nav className="flex h-full w-[18vw] min-w-[240px] flex-col gap-8 py-8 px-4 bg-[#1e2327] border-r border-white/5 shadow-2xl z-20">
      <div className="px-4 mb-4">
        <Logo />
      </div>

      <div className="flex-1 flex flex-col gap-1">
        <p className="px-4 text-[10px] font-bold text-gray-500 uppercase tracking-widest mb-2">{t("nav.mainMenu")}</p>
        <ul className="flex flex-col gap-1">
          {items.map((item) => (
            <AppSidebarItem
              key={item.id}
              item={item}
              isActive={activeTab === item.id}
              onClick={() => onTabChange(item.id)}
            />
          ))}
        </ul>
      </div>

      <div className="mt-auto px-4 pb-8 pt-6 border-t border-white/5">
        <button
          type="button"
          onClick={onChangeWorkspace}
          className="mb-3 flex w-full items-center gap-3 rounded-xl border border-white/5 bg-[#171b1f]/70 px-3 py-2.5 text-left text-gray-300 transition-all hover:border-cyan-300/30 hover:bg-cyan-500/10 hover:text-cyan-100"
          title="Trocar workspace"
        >
          <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-[#22272b] text-cyan-200">
            <Network size={17} />
          </span>
          <span className="min-w-0">
            <span className="block text-[9px] font-black uppercase tracking-widest text-gray-500">Workspace</span>
            <span className="block truncate text-xs font-bold">Trocar local/remoto</span>
          </span>
        </button>

        <div className="rounded-xl border border-white/5 bg-[#171b1f]/70 px-3 py-2.5">
          <div className="mb-1.5 flex items-center justify-between gap-3">
            <span className="text-[9px] font-bold uppercase tracking-widest text-gray-500">PZ Manager</span>
            <span className="rounded-full border border-orange-400/20 bg-orange-500/10 px-2 py-0.5 text-[9px] font-black uppercase text-orange-300">
              {appChannel}
            </span>
          </div>
          <div className="flex items-baseline gap-1.5">
            <span className="text-[10px] font-semibold text-gray-500">{t("app.version")}</span>
            <span className="font-mono text-xs font-bold text-gray-300">v{packageMetadata.version}</span>
          </div>
        </div>
      </div>
    </nav>
  )
}

function formatAppChannel(channel: string) {
  return channel.charAt(0).toUpperCase() + channel.slice(1).toLowerCase()
}
