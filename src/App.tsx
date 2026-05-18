import { useState } from 'react'
import { Search, Package, Server, Settings, Info, AlertTriangle } from 'lucide-react'
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { Button } from "@/components/ui/button"

const DUMMY_MODS = [
  { id: '1', name: 'Autotsar Trailers', author: 'iBrRus', version: '1.39', workshopId: '2282429356', status: 'installed' },
  { id: '2', name: 'Brita\'s Armor Pack', author: 'BRITA', version: '2.0.4', workshopId: '2460154032', status: 'update-available' },
  { id: '3', name: 'Common Sense', author: 'Braven', version: '1.0.5', workshopId: '2875848298', status: 'installed' },
  { id: '4', name: 'Filibuster Rhymes\' Used Cars!', author: 'Filibuster Rhymes', version: '4.1', workshopId: '1896907770', status: 'installed' },
  { id: '5', name: 'Minimal Display Bars', author: 'bitneko', version: '0.5.0', workshopId: '2004998206', status: 'missing' },
]

function App() {
  const [search, setSearch] = useState('')
  const [selectedMod, setSelectedMod] = useState<typeof DUMMY_MODS[0] | null>(null)

  const filteredMods = DUMMY_MODS.filter(mod => 
    mod.name.toLowerCase().includes(search.toLowerCase()) || 
    mod.workshopId.includes(search)
  )

  return (
    <div className="flex h-screen w-full bg-background text-foreground overflow-hidden">
      {/* Sidebar navigation */}
      <aside className="w-16 border-r flex flex-col items-center py-4 gap-4 bg-muted/30">
        <div className="w-10 h-10 rounded-lg bg-primary flex items-center justify-center text-primary-foreground mb-4">
          <Server size={24} />
        </div>
        <Button variant="ghost" size="icon" className="rounded-lg"><Package size={20} /></Button>
        <Button variant="ghost" size="icon" className="rounded-lg"><Settings size={20} /></Button>
        <div className="mt-auto">
          <Button variant="ghost" size="icon" className="rounded-lg"><Info size={20} /></Button>
        </div>
      </aside>

      {/* Mod List Sidebar */}
      <aside className="w-80 border-r flex flex-col bg-card">
        <div className="p-4 border-b space-y-4">
          <h2 className="text-xl font-semibold">Mod Manager</h2>
          <div className="relative">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search mods or ID..."
              className="pl-8"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
        </div>
        <ScrollArea className="flex-1">
          <div className="p-2 space-y-1">
            {filteredMods.map(mod => (
              <button
                key={mod.id}
                onClick={() => setSelectedMod(mod)}
                className={`w-full text-left px-3 py-2 rounded-md transition-colors ${
                  selectedMod?.id === mod.id ? 'bg-accent text-accent-foreground' : 'hover:bg-muted'
                }`}
              >
                <div className="font-medium text-sm truncate">{mod.name}</div>
                <div className="flex items-center justify-between mt-1">
                  <span className="text-xs text-muted-foreground">ID: {mod.workshopId}</span>
                  {mod.status === 'update-available' && (
                    <Badge variant="secondary" className="text-[10px] h-4 px-1 bg-yellow-500/10 text-yellow-600 border-yellow-600/20">
                      Update
                    </Badge>
                  )}
                  {mod.status === 'missing' && (
                    <Badge variant="destructive" className="text-[10px] h-4 px-1">
                      Missing
                    </Badge>
                  )}
                </div>
              </button>
            ))}
            {filteredMods.length === 0 && (
              <div className="p-4 text-center text-sm text-muted-foreground italic">
                No mods found
              </div>
            )}
          </div>
        </ScrollArea>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 overflow-auto bg-muted/10">
        {selectedMod ? (
          <div className="p-8 max-w-4xl mx-auto space-y-6">
            <div className="flex justify-between items-start">
              <div>
                <h1 className="text-3xl font-bold tracking-tight">{selectedMod.name}</h1>
                <p className="text-muted-foreground mt-1">by {selectedMod.author}</p>
              </div>
              <div className="flex gap-2">
                <Button variant="outline">Workshop Page</Button>
                <Button>Update Mod</Button>
              </div>
            </div>

            <div className="grid grid-cols-3 gap-4">
              <Card>
                <CardHeader className="pb-2">
                  <CardDescription>Workshop ID</CardDescription>
                  <CardTitle className="text-lg font-mono">{selectedMod.workshopId}</CardTitle>
                </CardHeader>
              </Card>
              <Card>
                <CardHeader className="pb-2">
                  <CardDescription>Version</CardDescription>
                  <CardTitle className="text-lg">{selectedMod.version}</CardTitle>
                </CardHeader>
              </Card>
              <Card>
                <CardHeader className="pb-2">
                  <CardDescription>Status</CardDescription>
                  <CardTitle className="text-lg capitalize">
                    {selectedMod.status.replace('-', ' ')}
                  </CardTitle>
                </CardHeader>
              </Card>
            </div>

            <Separator />

            <div className="space-y-4">
              <h3 className="text-lg font-semibold">Mod Details</h3>
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <div className="text-sm font-medium">Mod Folder</div>
                  <div className="text-sm text-muted-foreground bg-muted p-2 rounded border font-mono">
                    C:\ZomboidServer\mods\{selectedMod.name.replace(/\s+/g, '_')}
                  </div>
                </div>
                <div className="space-y-2">
                  <div className="text-sm font-medium">Last Sync</div>
                  <div className="text-sm text-muted-foreground bg-muted p-2 rounded border">
                    2 hours ago
                  </div>
                </div>
              </div>
            </div>

            {selectedMod.status === 'missing' && (
              <div className="flex items-center gap-3 p-4 bg-destructive/10 border border-destructive/20 rounded-lg text-destructive">
                <AlertTriangle size={20} />
                <div className="text-sm font-medium">
                  This mod is listed in the server configuration but not found in the local workshop directory.
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-full space-y-4 text-center">
            <div className="w-16 h-16 rounded-full bg-muted flex items-center justify-center text-muted-foreground">
              <Package size={32} />
            </div>
            <div>
              <h2 className="text-xl font-semibold">No Mod Selected</h2>
              <p className="text-muted-foreground">Select a mod from the sidebar to view its details and management options.</p>
            </div>
          </div>
        )}
      </main>
    </div>
  )
}

export default App
