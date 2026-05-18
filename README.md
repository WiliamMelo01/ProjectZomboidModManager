# Zomboid Server Mod Manager

Minimal Tauri scaffold for managing Project Zomboid dedicated servers and mods.

Setup (Windows):

```powershell
# Install prerequisites
# 1) Rust (https://rustup.rs)
# 2) Node.js (https://nodejs.org)
# 3) Tauri prerequisites (see https://tauri.app/v1/guides/getting-started/prerequisites)

# Install JS deps
npm install

# Start dev (frontend + tauri)
npm run dev
# In a separate terminal, run the Tauri dev command when ready:
npx tauri dev
```

Initialize repository (local)

Run the included script for your platform to initialize a local git repository
and make the initial commit. This is intended to be run on your machine
where Git is installed.

PowerShell (Windows):

```powershell
.
\scripts\init-repo.ps1
```

Unix / Git Bash:

```bash
sh ./scripts/init-repo.sh
```
