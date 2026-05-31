export type ServerTestResult = {
  status: "passed" | "failed" | "setup_error"
  summary: string
  durationSeconds: number
  batPath: string
  command: string
  warningCount: number
  criticalCount: number
  logLines: string[]
}

export type ServerTestEvent = {
  serverId: string
  event: "started" | "line" | "finished" | "error"
  timeoutSeconds: number | null
  line: string | null
  result: ServerTestResult | null
  error: string | null
}
