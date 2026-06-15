import { Gauge, Lightbulb } from "lucide-react"
import { useTranslation } from "react-i18next"

export function SteamCmdTips() {
  const { t } = useTranslation()

  return (
    <div className="hidden lg:block absolute top-24 right-0 w-72 animate-in fade-in slide-in-from-right-4 duration-500">
      <section className="relative overflow-hidden rounded-3xl border border-orange-400/20 bg-[#2b3238] p-6 shadow-xl">
        <div className="absolute right-0 top-0 -mr-12 -mt-12 h-24 w-24 rounded-full bg-orange-500/5 blur-3xl" />

        <div className="mb-4 flex items-center gap-3">
          <div className="rounded-lg bg-orange-500/10 p-2 text-orange-400">
            <Lightbulb size={20} />
          </div>
          <h3 className="text-sm font-bold uppercase italic tracking-tight text-white">{t("steamcmdTips.title")}</h3>
        </div>

        <div className="space-y-5">
          <SteamCmdTipItem label={t("steamcmdTips.one")} description={t("steamcmdTips.oneDescription")} />
          <SteamCmdTipItem label={t("steamcmdTips.two")} description={t("steamcmdTips.twoDescription")} />
          <SteamCmdTipItem label={t("steamcmdTips.three")} description={t("steamcmdTips.threeDescription")} />

          <div className="rounded-2xl border border-white/5 bg-[#1e2327] p-4">
            <div className="mb-2 flex items-center gap-2">
              <Gauge size={12} className="text-orange-400" />
              <span className="text-[9px] font-bold uppercase italic text-white">{t("steamcmdTips.isolation")}</span>
            </div>
            <p className="text-[10px] italic leading-relaxed text-gray-500">
              {t("steamcmdTips.isolationDescription")}
            </p>
          </div>
        </div>
      </section>
    </div>
  )
}

function SteamCmdTipItem({ label, description }: { label: string; description: string }) {
  return (
    <div className="flex gap-2">
      <div className="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-orange-500" />
      <p className="text-[11px] text-gray-400">
        <span className="font-bold text-white">{label}</span> {description}
      </p>
    </div>
  )
}
