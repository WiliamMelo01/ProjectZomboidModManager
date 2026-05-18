import { Plus, Server, Users, Activity, Wifi } from "lucide-react"

interface ServerInfo {
  id: string
  name: string
  players: number
  maxPlayers: number
  status: "online" | "offline"
  version: string
  ip: string
}

const mockServers: ServerInfo[] = [
  {
    id: "1",
    name: "Meu Servidor Local",
    players: 0,
    maxPlayers: 32,
    status: "offline",
    version: "41.78.16",
    ip: "127.0.0.1",
  },
  {
    id: "2",
    name: "Teste de Mods - Local",
    players: 0,
    maxPlayers: 10,
    status: "offline",
    version: "41.78.16",
    ip: "localhost",
  },
]

export function Dashboard() {
  return (
    <div className="p-8 h-full overflow-y-auto custom-scrollbar">
      <div className="flex justify-between items-center mb-8">
        <div>
          <h2 className="text-3xl font-bold tracking-tight">Servidores</h2>
          <p className="text-gray-400 mt-1">Gerencie e monitore seus servidores de Project Zomboid.</p>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {mockServers.map((server) => (
          <ServerCard key={server.id} server={server} />
        ))}
        
        <AddServerCard />
      </div>
    </div>
  )
}

function ServerCard({ server }: { server: ServerInfo }) {
  const isOnline = server.status === "online"

  return (
    <div className="group relative bg-[#2b3238] border border-white/5 rounded-2xl p-6 transition-all duration-300 hover:border-orange-400/40 hover:bg-[#353c42] hover:shadow-[0_0_20px_rgba(251,146,60,0.1)] overflow-hidden cursor-pointer">
      {/* Decorative background glow on hover */}
      <div className="absolute -right-8 -top-8 w-24 h-24 bg-orange-400/5 rounded-full blur-3xl group-hover:bg-orange-400/10 transition-colors duration-300" />
      
      <div className="flex justify-between items-start mb-4">
        <div className={`flex items-center gap-2 px-2.5 py-0.5 rounded-full text-xs font-medium ${
          isOnline ? "bg-green-500/10 text-green-400" : "bg-red-500/10 text-red-400"
        }`}>
          <div className={`w-1.5 h-1.5 rounded-full ${isOnline ? "bg-green-400 animate-pulse" : "bg-red-400"}`} />
          {server.status.toUpperCase()}
        </div>
        <span className="text-xs text-gray-500 font-mono">{server.version}</span>
      </div>

      <div className="flex items-center gap-3 mb-6">
        <div className="p-2.5 bg-[#22272b] rounded-xl group-hover:text-orange-400 transition-colors">
          <Server size={24} />
        </div>
        <h3 className="text-xl font-semibold truncate">{server.name}</h3>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2 text-gray-400">
            <Users size={16} />
            <span>Jogadores</span>
          </div>
          <span className="font-medium">{server.players} / {server.maxPlayers}</span>
        </div>
        
        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2 text-gray-400">
            <Wifi size={16} />
            <span>Endereço</span>
          </div>
          <span className="font-mono text-xs text-gray-300">{server.ip}</span>
        </div>

        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2 text-gray-400">
            <Activity size={16} />
            <span>Performance</span>
          </div>
          <div className="h-1.5 w-24 bg-[#22272b] rounded-full overflow-hidden">
            <div 
              className={`h-full rounded-full ${isOnline ? "bg-orange-400" : "bg-gray-600"}`} 
              style={{ width: isOnline ? '65%' : '0%' }} 
            />
          </div>
        </div>
      </div>
    </div>
  )
}

function AddServerCard() {
  return (
    <button className="group h-full min-h-[220px] flex flex-col items-center justify-center gap-4 bg-transparent border-2 border-dashed border-white/10 rounded-2xl transition-all duration-300 hover:border-orange-400/50 hover:bg-orange-400/5 hover:shadow-[0_0_20px_rgba(251,146,60,0.05)]">
      <div className="p-4 bg-[#2b3238] rounded-full group-hover:scale-110 group-hover:text-orange-400 transition-all duration-300">
        <Plus size={32} />
      </div>
      <div className="text-center">
        <p className="text-lg font-semibold">Criar Servidor</p>
        <p className="text-sm text-gray-500">Adicione um novo servidor</p>
      </div>
    </button>
  )
}
