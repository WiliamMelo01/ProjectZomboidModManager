import type { ServerTestResult } from "@/types/serverTest"

export function getServerTestStatusLabel(status: ServerTestResult["status"]) {
  switch (status) {
    case "passed":
      return "Sem falhas criticas"
    case "failed":
      return "Falhas encontradas"
    case "setup_error":
      return "Configuracao incompleta"
  }
}

export function getServerTestStatusStyle(status: ServerTestResult["status"] | undefined, isTesting: boolean) {
  if (isTesting) {
    return {
      iconBg: "bg-orange-500/10",
      panel: "border-orange-500/20 bg-orange-500/10",
    }
  }

  switch (status) {
    case "passed":
      return {
        iconBg: "bg-green-500/10",
        panel: "border-green-500/20 bg-green-500/10",
      }
    case "failed":
      return {
        iconBg: "bg-red-500/10",
        panel: "border-red-500/20 bg-red-500/10",
      }
    default:
      return {
        iconBg: "bg-orange-500/10",
        panel: "border-orange-500/20 bg-orange-500/10",
      }
  }
}

export function formatDuration(totalSeconds: number) {
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60

  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
}

export function getLogLineClassName(line: string) {
  const normalizedLine = line.toLowerCase()

  if (
    normalizedLine.includes("*** server started") ||
    normalizedLine.includes("server is listening on port") ||
    normalizedLine.includes("raknet.startup() return code: 0")
  ) {
    return "text-green-300"
  }

  if (
    normalizedLine.includes("error") ||
    normalizedLine.includes("exception") ||
    normalizedLine.includes("java.lang") ||
    normalizedLine.includes("failed") ||
    normalizedLine.includes("required mod") ||
    normalizedLine.includes("workshop item") ||
    normalizedLine.includes("missing mod") ||
    normalizedLine.includes("missing required")
  ) {
    return "text-red-300"
  }

  if (normalizedLine.includes("warn")) {
    return "text-yellow-300"
  }

  if (normalizedLine.includes("log")) {
    return "text-gray-300"
  }

  return "text-gray-400"
}
