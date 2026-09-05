import { spawn } from "node:child_process"
import { existsSync, readFileSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const command = process.argv[2]
const passthroughArgs = process.argv.slice(3)

if (!command) {
  console.error("Missing Tauri command.")
  process.exit(1)
}

const env = { ...process.env }
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")

loadDotEnv(path.join(repoRoot, ".env"), env)

// On Windows, ensure .cargo/bin and the Node.js bin directory are in PATH so
// cargo and node can be found even when the process env does not inherit the
// full user PATH (e.g. IDE terminals or restricted shells).
if (process.platform === "win32") {
  const sep = ";"
  const extraPaths = []

  // Add Node.js bin directory (parent of node.exe)
  const nodeBin = path.dirname(process.execPath)
  extraPaths.push(nodeBin)

  // Add .cargo/bin
  const userProfile = env.USERPROFILE || env.HOME || ""
  if (userProfile) {
    extraPaths.push(path.join(userProfile, ".cargo", "bin"))
  }

  // Add essential Windows system directories (ssh, scp, powershell, etc.)
  const systemRoot = env.SystemRoot || env.SYSTEMROOT || "C:\\Windows"
  extraPaths.push(path.join(systemRoot, "System32"))
  extraPaths.push(path.join(systemRoot, "System32", "OpenSSH"))
  extraPaths.push(path.join(systemRoot, "System32", "WindowsPowerShell", "v1.0"))
  extraPaths.push(path.join(systemRoot))
  extraPaths.push(path.join(systemRoot, "System32", "Wbem"))

  // PowerShell 7+ (optional, may not be installed)
  extraPaths.push("C:\\Program Files\\PowerShell\\7")

  // Add Git (ships its own ssh.exe as fallback)
  extraPaths.push("C:\\Program Files\\Git\\cmd")
  extraPaths.push("C:\\Program Files\\Git\\usr\\bin")

  for (const entry of extraPaths) {
    if (!env.PATH?.split(sep).some((p) => p.toLowerCase() === entry.toLowerCase())) {
      env.PATH = (env.PATH ? env.PATH + sep : "") + entry
    }
  }
}

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

const spawnCommand = process.platform === "win32"
  ? (process.env.COMSPEC || "C:\\Windows\\System32\\cmd.exe")
  : commandPath
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

function loadDotEnv(filePath, targetEnv) {
  if (!existsSync(filePath)) {
    return
  }

  const content = readFileSync(filePath, "utf8")
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim()
    if (!line || line.startsWith("#")) {
      continue
    }

    const separator = line.indexOf("=")
    if (separator <= 0) {
      continue
    }

    const key = line.slice(0, separator).trim()
    let value = line.slice(separator + 1).trim()
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1)
    }

    if (!(key in targetEnv)) {
      targetEnv[key] = value
    }
  }
}

