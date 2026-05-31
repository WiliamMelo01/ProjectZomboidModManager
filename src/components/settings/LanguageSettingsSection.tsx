import { Languages } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { LanguagePreference } from "@/types/settings"

type LanguageSettingsSectionProps = {
  preference: LanguagePreference
  onChange: (preference: LanguagePreference) => void
}

export function LanguageSettingsSection({ preference, onChange }: LanguageSettingsSectionProps) {
  const { t } = useTranslation()
  const options: Array<{ value: LanguagePreference; label: string }> = [
    { value: "auto", label: t("language.auto") },
    { value: "en", label: t("language.en") },
    { value: "pt-BR", label: t("language.ptBR") },
  ]

  return (
    <section className="mb-6 rounded-3xl border border-white/5 bg-[#2b3238] p-6 shadow-xl">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl border border-orange-500/20 bg-orange-500/10 text-orange-400">
            <Languages size={20} />
          </div>
          <div>
            <h3 className="text-lg font-bold text-white">{t("language.title")}</h3>
            <p className="text-xs text-gray-500">{t("language.description")}</p>
          </div>
        </div>
        <select
          value={preference}
          onChange={(event) => onChange(event.target.value as LanguagePreference)}
          className="rounded-xl border border-white/10 bg-[#1e2327] px-4 py-3 text-sm font-bold text-white outline-none transition-colors focus:border-orange-400/50"
        >
          {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </select>
      </div>
    </section>
  )
}
