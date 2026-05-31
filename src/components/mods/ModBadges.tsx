import { Car, Languages, MapPinned, Shirt, Terminal } from "lucide-react"

const BADGES = {
  map: { label: "Mapa", icon: MapPinned, className: "border-orange-400/20 bg-orange-400/10 text-orange-300" },
  translation: { label: "Traducao", icon: Languages, className: "border-cyan-400/20 bg-cyan-400/10 text-cyan-300" },
  vehicles: { label: "Veiculos", icon: Car, className: "border-blue-400/20 bg-blue-400/10 text-blue-300" },
  clothing: { label: "Roupas", icon: Shirt, className: "border-pink-400/20 bg-pink-400/10 text-pink-300" },
  lua: { label: "Lua", icon: Terminal, className: "border-purple-400/20 bg-purple-400/10 text-purple-300" },
} as const

const BADGE_PRIORITY = ["map", "vehicles", "clothing", "translation", "lua"] as const

type ModBadgesProps = {
  badges?: string[]
}

export function ModBadges({ badges = [] }: ModBadgesProps) {
  const primaryBadge = BADGE_PRIORITY.find((badge) => badges.includes(badge))
  const knownBadges = primaryBadge ? [BADGES[primaryBadge]] : []

  if (knownBadges.length === 0) {
    return null
  }

  return (
    <div className="flex flex-wrap gap-1.5">
      {knownBadges.map(({ label, icon: Icon, className }) => (
        <span key={label} className={`flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide ${className}`}>
          <Icon size={11} />
          {label}
        </span>
      ))}
    </div>
  )
}
