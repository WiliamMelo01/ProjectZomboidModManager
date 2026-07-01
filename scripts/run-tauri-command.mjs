import { spawn } from "node:child_process"

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

const child = spawn("tauri", [command], {
  stdio: "inherit",
  env,
  shell: false,
})

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`tauri ${command} terminated by signal ${signal}`)
    process.exit(1)
  }

  process.exit(code ?? 1)
})

