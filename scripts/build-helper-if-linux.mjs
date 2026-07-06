import { spawn } from "node:child_process"
import path from "node:path"
import { fileURLToPath } from "node:url"

const release = process.argv.includes("--release")

if (process.platform !== "linux") {
  console.log("Skipping Linux helper build on non-Linux host.")
  process.exit(0)
}

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const args = ["build", "--bin", "pzmm-helper-linux-x86_64"]

if (release) {
  args.splice(1, 0, "--release")
}

const child = spawn("cargo", args, {
  cwd: path.join(repoRoot, "src-tauri"),
  stdio: "inherit",
  shell: false,
})

child.on("error", (error) => {
  console.error(`Could not start cargo to build Linux helper: ${error.message}`)
  process.exit(1)
})

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`Linux helper build terminated by signal ${signal}`)
    process.exit(1)
  }

  process.exit(code ?? 1)
})