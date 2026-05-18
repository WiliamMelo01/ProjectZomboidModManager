import { FolderSync, Settings, Search, Bell } from "lucide-react"

export function AppHeader() {
  return (
    <header className="sticky top-0 z-10 w-full px-8 py-4 flex items-center justify-between bg-[#22272b]/80 backdrop-blur-md border-b border-white/5">
      <div className="flex items-center flex-1 max-w-md relative group">
        <Search className="absolute left-3 text-gray-500 group-focus-within:text-orange-400 transition-colors" size={18} />
        <input 
          type="text" 
          placeholder="Buscar servidores ou mods..." 
          className="w-full bg-[#2b3238] border border-white/5 rounded-xl py-2 pl-10 pr-4 text-sm focus:outline-none focus:border-orange-400/50 focus:ring-1 focus:ring-orange-400/20 transition-all placeholder:text-gray-600"
        />
      </div>

      <div className="flex items-center gap-3">
        <button className="flex items-center gap-2 bg-orange-400/10 text-orange-400 hover:bg-orange-400 hover:text-white px-4 py-2 rounded-xl transition-all duration-300 font-medium text-sm group">
          <FolderSync size={20} className="group-hover:rotate-180 transition-transform duration-500" />
          <span>Escanear Mods</span>
        </button>

        <div className="w-[1px] h-8 bg-white/5 mx-2" />

        <button className="p-2.5 bg-[#2b3238] border border-white/5 text-gray-400 hover:text-white hover:border-white/10 rounded-xl transition-all relative">
          <Bell size={20} />
          <span className="absolute top-2.5 right-3 w-2 h-2 bg-orange-500 rounded-full border-2 border-[#2b3238]" />
        </button>

        <button className="p-2.5 bg-[#2b3238] border border-white/5 text-gray-400 hover:text-white hover:border-white/10 rounded-xl transition-all">
          <Settings size={20} />
        </button>
      </div>
    </header>
  )
}
