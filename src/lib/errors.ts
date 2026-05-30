export function getErrorMessage(error: unknown, fallback = "Nao foi possivel buscar os servidores.") {
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
