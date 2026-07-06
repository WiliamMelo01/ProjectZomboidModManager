import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const command = process.argv[2]
const passthroughArgs = process.argv.slice(3)

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
const platformBuildArgs = command === "build" && process.platform === "linux" && !passthroughArgs.includes("--bundles") && !passthroughArgs.includes("-b")
  ? ["--bundles", "deb"]
  : []
const tauriArgs = [command, ...platformBuildArgs, ...passthroughArgs]
const commandArgs = existsSync(localTauri)
  ? tauriArgs
  : ["exec", "--package", "@tauri-apps/cli", "--", "tauri", ...tauriArgs]

function quoteWindowsCommandArg(value) {
  if (/^[A-Za-z0-9_@%+=:,./\\-]+$/.test(value)) {
    return value
  }

  return `"${value.replace(/(\\*)"/g, '$1$1\\"').replace(/\\+$/g, '$&$&')}"`
}

const spawnCommand = process.platform === "win32" ? "cmd.exe" : commandPath
const spawnArgs = process.platform === "win32"
  ? ["/d", "/c", [commandPath, ...commandArgs].map(quoteWindowsCommandArg).join(" ")]
  : commandArgs

const child = spawn(spawnCommand, spawnArgs, {
  stdio: "inherit",
  env,
  shell: false,
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

