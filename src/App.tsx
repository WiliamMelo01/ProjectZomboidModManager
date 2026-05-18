import { useState } from 'react'

function App() {
  const [search, setSearch] = useState('')

  return (
    <div className="flex flex-col h-screen w-full bg-background text-foreground">
      <header className="border-b px-6 py-4">
        <h1 className="text-2xl font-bold">Zomboid Server Mod Manager</h1>
      </header>
      <main className="flex flex-1 overflow-hidden">
        <aside className="w-80 border-r flex flex-col p-4 gap-4">
          <input
            type="text"
            placeholder="Search mods..."
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <div className="flex-1 overflow-auto border rounded-md p-2">
            {search ? (
              <div className="text-muted-foreground italic text-sm">Filter: {search}</div>
            ) : (
              <div className="text-muted-foreground italic text-sm">No mods yet — implement scanner</div>
            )}
          </div>
        </aside>
        <section className="flex-1 p-6">
          <div className="flex items-center justify-center h-full border-2 border-dashed rounded-lg">
            <p className="text-muted-foreground">Select a mod to see details.</p>
          </div>
        </section>
      </main>
    </div>
  )
}

export default App
