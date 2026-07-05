import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const command = process.argv[2]

if (!command) {
  console.error("Missing Tauri command.")
  process.exit(1)
}

const env = { ...process.env }

if (process.platform === "linux") {
  for (const key of Object.keys(env)) {
    if (
      key === "LD_LIBRARY_PATH" ||
      key === "LD_PRELOAD" ||
      key === "GTK_PATH" ||
      key === "GTK_EXE_PREFIX" ||
      key === "GIO_MODULE_DIR" ||
      key === "GTK_MODULES" ||
      key.startsWith("SNAP_")
    ) {
      delete env[key]
    }
  }
}

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const localTauri = path.join(repoRoot, "node_modules", ".bin", process.platform === "win32" ? "tauri.cmd" : "tauri")
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm"
const commandPath = existsSync(localTauri) ? localTauri : npmCommand
const commandArgs = existsSync(localTauri) ? [command] : ["exec", "--", "tauri", command]

const child = spawn(commandPath, commandArgs, {
  stdio: "inherit",
  env,
  shell: process.platform === "win32",
})

child.on("error", (error) => {
  console.error(`Could not start Tauri CLI: ${error.message}`)
  process.exit(1)
})

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`tauri ${command} terminated by signal ${signal}`)
    process.exit(1)
  }

  process.exit(code ?? 1)
})

