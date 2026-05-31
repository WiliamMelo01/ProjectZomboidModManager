import { i18n } from "@/i18n"

export function getErrorMessage(error: unknown, fallback = i18n.t("errors.loadServers")) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  if (error) {
    return JSON.stringify(error)
  }

  return fallback
}
