import type { LucideIcon } from "lucide-react"

export type SidebarItem = {
  id: string
  label: string
  icon: LucideIcon
  badge?: string
}

type AppSidebarItemProps = {
  item: SidebarItem
  isActive: boolean
  onClick: () => void
}

export function AppSidebarItem({ item, isActive, onClick }: AppSidebarItemProps) {
  const Icon = item.icon

  return (
    <li
      className={`group flex items-center gap-3 px-4 py-3 rounded-xl cursor-pointer transition-all duration-300 relative overflow-hidden ${
        isActive 
          ? "bg-orange-500 text-white shadow-[0_4px_15px_rgba(249,115,22,0.2)]" 
          : "text-gray-400 hover:text-white hover:bg-white/5"
      }`}
      onClick={onClick}
    >
      {/* Active Indicator Bar */}
      {isActive && (
        <div className="absolute left-0 top-1/4 bottom-1/4 w-1 bg-white rounded-r-full" />
      )}

      <Icon size={20} className={`${isActive ? "text-white" : "group-hover:text-orange-400"} transition-colors`} />
      <span className="text-sm font-medium tracking-wide">{item.label}</span>
      
      {item.badge && (
        <span className={`ml-auto text-[10px] font-bold px-2 py-0.5 rounded-md transition-colors ${
          isActive 
            ? "bg-white/20 text-white" 
            : "bg-[#2b3238] text-orange-400 group-hover:bg-orange-500 group-hover:text-white"
        }`}>
          {item.badge}
        </span>
      )}
    </li>
  )
}
