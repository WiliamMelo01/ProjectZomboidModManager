import { Lightbulb } from "lucide-react"

export function RamTips() {
  return (
    <div className="hidden lg:block absolute top-24 right-0 w-72 animate-in fade-in slide-in-from-right-4 duration-500">
      <section className="bg-[#2b3238] rounded-3xl border border-orange-400/20 p-6 shadow-xl relative overflow-hidden group">
        <div className="absolute top-0 right-0 w-24 h-24 bg-orange-500/5 blur-3xl rounded-full -mr-12 -mt-12" />

        <div className="flex items-center gap-3 mb-4">
          <div className="p-2 bg-orange-500/10 text-orange-400 rounded-lg">
            <Lightbulb size={20} />
          </div>
          <h3 className="font-bold text-white tracking-tight text-sm uppercase italic">Dicas de Alocação</h3>
        </div>

        <div className="space-y-6">
          <RamTipsSection
            title="Client (Jogo)"
            items={[
              ["Vanilla:", "2GB a 4GB é o suficiente."],
              ["Alguns Mods:", "4GB a 6GB recomendado."],
              ["Muitos Mods:", "8GB+ para estabilidade."],
            ]}
          />

          <div className="h-px bg-white/5" />

          <RamTipsSection
            title="Servidor"
            items={[
              ["Pequeno:", "2GB a 4GB dão conta."],
              ["Médio + Mods:", "6GB a 8GB recomendados."],
              ["Grandes:", "12GB+ para mapas extensos."],
            ]}
          />

          <div className="bg-[#1e2327] rounded-2xl p-4 border border-white/5">
            <div className="flex items-center gap-2 mb-2">
              <Lightbulb size={12} className="text-orange-400" />
              <span className="text-[9px] font-bold text-white uppercase italic">Atenção</span>
            </div>
            <p className="text-[10px] text-gray-500 leading-relaxed italic">
              Deixe sempre 2-4GB livres para o seu Windows funcionar sem travar.
            </p>
          </div>
        </div>
      </section>
    </div>
  )
}

type RamTipsSectionProps = {
  title: string
  items: [string, string][]
}

function RamTipsSection({ title, items }: RamTipsSectionProps) {
  return (
    <div>
      <p className="text-[9px] font-black text-gray-500 uppercase tracking-widest mb-3">{title}</p>
      <ul className="space-y-3">
        {items.map(([label, description]) => (
          <li key={label} className="flex gap-2">
            <div className="w-1 h-1 rounded-full bg-orange-500 mt-1.5 shrink-0" />
            <p className="text-[11px] text-gray-400">
              <span className="text-white font-bold">{label}</span> {description}
            </p>
          </li>
        ))}
      </ul>
    </div>
  )
}
