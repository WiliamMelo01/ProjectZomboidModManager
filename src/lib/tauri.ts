import { invoke } from "@tauri-apps/api/core"

export function invokeTauri<T>(command: string, args?: Record<string, unknown>) {
  return invoke<T>(command, args)
}
