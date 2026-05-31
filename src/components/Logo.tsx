import { cn } from "@/lib/utils"

interface LogoProps {
  className?: string
  size?: number
  iconOnly?: boolean
}

export function Logo({ className, size = 32, iconOnly = false }: LogoProps) {
  return (
    <div className={cn("flex items-center gap-3", className)}>
      <div
        className="relative flex items-center justify-center"
        style={{ width: size, height: size }}
      >
        {/* Background Glow */}
        <div className="absolute inset-0 bg-orange-500/20 blur-lg rounded-full" />

        {/* Hexagon Frame */}
        <svg
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          className="w-full h-full text-orange-500 drop-shadow-[0_0_8px_rgba(249,115,22,0.5)]"
        >
          <path
            d="M12 2L3.5 7V17L12 22L20.5 17V7L12 2Z"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            className="fill-orange-500/10"
          />
          {/* Stylized Z */}
          <path
            d="M7 8H17L7 16H17"
            stroke="white"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="drop-shadow-sm"
          />
          {/* "Scratches" for survival feel */}
          <path
            d="M9 10L8 11"
            stroke="currentColor"
            strokeWidth="1"
            strokeLinecap="round"
          />
          <path
            d="M16 13L15 14"
            stroke="currentColor"
            strokeWidth="1"
            strokeLinecap="round"
          />
        </svg>
      </div>

      {!iconOnly && (
        <div className="flex flex-col leading-none">
          <span className="text-xl font-black tracking-tighter bg-gradient-to-r from-white to-gray-400 bg-clip-text text-transparent uppercase italic">
            PZ Manager
          </span>
          <span className="text-[10px] font-bold text-orange-500/80 tracking-[0.2em] uppercase mt-0.5">
            Mod Manager
          </span>
        </div>
      )}
    </div>
  )
}
