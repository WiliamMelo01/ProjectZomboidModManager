import { useState } from "react"
import { Box, Home, Settings } from "lucide-react"

import { AppHeader } from "@/components/AppHeader"
import { AppSidebar } from "@/components/AppSidebar"
import { Dashboard } from "@/components/Dashboard"

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: Home },
  { id: "mods", label: "Mods", icon: Box, badge: "189" },
  { id: "settings", label: "Settings", icon: Settings },
]

function App() {
  const [activeTab, setActiveTab] = useState("dashboard")
 
  return (
    <main className="flex h-screen w-screen overflow-hidden bg-[#22272b] text-white">
      <AppSidebar activeTab={activeTab} items={navItems} onTabChange={setActiveTab} />

      <div className="flex-1 flex flex-col min-w-0">
        <AppHeader />

        <div className="flex-1 overflow-hidden relative">
          {activeTab === "dashboard" && <Dashboard />}
          {activeTab === "mods" && (
            <div className="flex items-center justify-center h-full text-gray-500">
              Mods view coming soon...
            </div>
          )}
          {activeTab === "settings" && (
            <div className="flex items-center justify-center h-full text-gray-500">
              Settings view coming soon...
            </div>
          )}
        </div>
      </div>
    </main>
  );
}

export default App
