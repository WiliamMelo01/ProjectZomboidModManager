import { ChevronDown } from "lucide-react"
import { useState } from "react"

type RamDropdownProps = {
  value: string
  onChange: (value: string) => void
  options: string[]
}

export function RamDropdown({ value, onChange, options }: RamDropdownProps) {
  const [isOpen, setIsOpen] = useState(false)

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full bg-[#1e2327] border rounded-2xl py-4 px-5 text-sm font-medium transition-all flex items-center justify-between text-white group ${
          isOpen ? "border-orange-400/50 ring-1 ring-orange-400/20" : "border-white/5 hover:border-white/10"
        }`}
      >
        <span>{value} GB</span>
        <ChevronDown
          size={18}
          className={`text-gray-500 group-hover:text-orange-400 transition-all ${isOpen ? "rotate-180 text-orange-400" : ""}`}
        />
      </button>

      {isOpen && (
        <>
          <div className="fixed inset-0 z-[60]" onClick={() => setIsOpen(false)} />
          <div className="absolute top-full left-0 right-0 mt-2 bg-[#1e2327] border border-white/10 rounded-2xl overflow-hidden shadow-2xl z-[70] animate-in fade-in zoom-in-95 duration-200">
            <div className="max-h-60 overflow-y-auto custom-scrollbar">
              {options.map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => {
                    onChange(option)
                    setIsOpen(false)
                  }}
                  className={`w-full text-left px-5 py-3 text-sm transition-colors hover:bg-orange-500/10 hover:text-orange-400 ${
                    value === option ? "text-orange-400 bg-orange-500/5 font-bold" : "text-gray-400"
                  }`}
                >
                  {option} GB
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  )
}
