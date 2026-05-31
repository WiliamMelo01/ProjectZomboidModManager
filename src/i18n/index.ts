import i18n from "i18next"
import { initReactI18next } from "react-i18next"

import { invokeTauri } from "@/lib/tauri"
import type { EffectiveLanguage, LanguagePreference } from "@/types/settings"
import { resources } from "@/i18n/resources"

export function resolveEffectiveLanguage(
  preference: LanguagePreference,
  systemLanguages = navigator.languages,
): EffectiveLanguage {
  if (preference !== "auto") {
    return preference
  }

  return systemLanguages.some((language) => language.toLowerCase().startsWith("pt"))
    ? "pt-BR"
    : "en"
}

export async function initializeI18n() {
  let preference: LanguagePreference = "auto"

  try {
    preference = await invokeTauri<LanguagePreference>("get_language_preference")
  } catch {
    // The browser-only development server has no Tauri backend.
  }

  const effectiveLanguage = resolveEffectiveLanguage(preference)

  await i18n
    .use(initReactI18next)
    .init({
      resources,
      lng: effectiveLanguage,
      fallbackLng: "en",
      interpolation: { escapeValue: false },
    })

  try {
    await invokeTauri<void>("sync_effective_language", { effectiveLanguage })
  } catch {
    // Keep browser-only development usable.
  }
}

export async function setLanguagePreference(preference: LanguagePreference) {
  const effectiveLanguage = resolveEffectiveLanguage(preference)
  const previousLanguage = i18n.language
  await i18n.changeLanguage(effectiveLanguage)
  try {
    await invokeTauri<void>("set_language_preference", { preference, effectiveLanguage })
  } catch (error) {
    await i18n.changeLanguage(previousLanguage)
    throw error
  }
}

export { i18n }
