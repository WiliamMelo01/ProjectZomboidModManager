import { AppSidebarItem, type SidebarItem } from "@/components/AppSidebarItem"

type AppSidebarProps = {
  activeTab: string
  items: SidebarItem[]
  onTabChange: (tabId: string) => void
}

export function AppSidebar({ activeTab, items, onTabChange }: AppSidebarProps) {
  return (
    <nav className="flex h-full w-[18vw] min-w-[240px] flex-col gap-8 py-8 px-4 bg-[#1e2327] border-r border-white/5 shadow-2xl z-20">
      <div className="px-4 mb-4">
        <h1 className="text-2xl font-black tracking-tighter flex items-center gap-2">
          <div className="w-8 h-8 bg-orange-500 rounded-lg flex items-center justify-center shadow-[0_0_15px_rgba(249,115,22,0.3)]">
            <span className="text-white text-lg">P</span>
          </div>
          <span className="bg-gradient-to-r from-white to-gray-400 bg-clip-text text-transparent uppercase italic">
            Z Manager
          </span>
        </h1>
      </div>

      <div className="flex-1 flex flex-col gap-1">
        <p className="px-4 text-[10px] font-bold text-gray-500 uppercase tracking-widest mb-2">Menu Principal</p>
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

      <div className="mt-auto px-4 pt-6 border-t border-white/5">
        <div className="bg-[#2b3238] rounded-xl p-3 flex items-center gap-3 border border-white/5">
          <div className="w-10 h-10 rounded-full bg-orange-500/10 flex items-center justify-center text-orange-400 font-bold border border-orange-500/20">
            AD
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-semibold truncate">Admin User</p>
            <p className="text-[10px] text-gray-500 truncate">v1.0.4 - Alpha</p>
          </div>
        </div>
      </div>
    </nav>
  )
}
