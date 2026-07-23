# Zomboid Server Mod Manager

## Overview

Zomboid Server Mod Manager, branded in the UI as **PZ Manager**, is a Tauri
desktop application for managing Project Zomboid multiplayer server profiles,
mods, Workshop downloads, diagnostics, and remote Linux dedicated servers.

The app started as a local Windows-focused mod/profile manager, but version
`0.5.0` now includes remote workspace support over SSH, a Linux helper, SteamCMD
workflows, server startup controls, log viewing, and a shared Workshop ID
mapping sync.

## Project Info

- Name: Zomboid Server Mod Manager / PZ Manager
- Package: `zomboid-server-mod-manager`
- Version: `0.5.0`
- Channel: `beta`
- Type: Desktop application
- Runtime shell: Tauri 2
- Frontend: React 19, TypeScript, Vite 8, Tailwind CSS 4
- Backend: Rust
- Remote helper: Rust helper binary for Linux servers
- Target platforms: Windows desktop app, Linux desktop builds, and remote Linux dedicated servers

## Product Goals

- Make Project Zomboid server mod setup manageable without hand-editing every
  profile file.
- Keep `Mods=` and `WorkshopItems=` aligned across local and remote profiles.
- Reduce errors around missing Workshop IDs, missing dependencies, wrong load
  order, incompatible B41/B42 mods, and port conflicts.
- Give server owners a single app for library scanning, downloading, profile
  editing, testing, remote startup, logs, and maintenance.
- Keep common workflows accessible for community server admins who are not
  comfortable operating fully from terminals.

## Current Feature Set

### Workspaces

- Local Windows workspace for installed Project Zomboid profiles and mods.
- Remote Linux workspace over SSH.
- Saved remote connection profile support.
- Automatic reconnect behavior where available.
- Separate cache keys for local and remote workspace data.

### Server Profiles

- List existing Project Zomboid server profiles.
- Create new profiles.
- Search, hide/show, and favorite servers.
- Delete profiles with safety backup.
- Clone active mod lists from another server using the same build.
- Edit common server `.ini` settings such as public name, description, ports,
  player limits, PvP, backups, and related server options.
- Read/update SandboxVars-like Lua settings where supported by the backend.

### Build Support

- Supports Build 41 and Build 42 server profiles.
- Stores build metadata per server.
- Allows build changes with confirmation.
- Detects incompatible active mods.
- Preserves B42 versioned mod layouts and shared `common` content.

### Mod Library

- Scans local `Zomboid/mods`.
- Scans Steam Workshop install folders.
- Scans custom mod locations configured in Settings.
- Scans remote mod folders through the Linux helper.
- Caches library metadata and thumbnails for faster repeat loads.
- Shows mod source, author, description, version, maps, dependencies, build
  compatibility, size, and Workshop ID when known.
- Allows deletion of local or remote mods with confirmation.

### Active Mod Management

- Activate/deactivate mods on a server profile.
- Reorder active mods.
- Protect dependency order.
- Paginate and filter server mod lists.
- Add maps when a mod includes map folders.
- Resolve B41/B42 variant IDs before writing `Mods=`.
- Keep `WorkshopItems=` unique and aligned with active mods.

### Workshop ID Mapping

- Local `Mod ID -> Workshop ID` mapping database.
- Manual Workshop ID editing from the mod detail modal.
- Missing Workshop ID repair flow for server profiles.
- Shared HTTP mapping service sync.
- Automatic upload of newly discovered mappings.
- Visual sync error when the shared service cannot be reached or returns invalid data.
- Local-cache fallback when the shared service is unavailable.

Only mapping pairs are synced. Server configs, credentials, and mod files are not
uploaded through the mapping sync.

### SteamCMD Downloads

- Download a single Workshop item by numeric ID or URL.
- Download public Workshop collections.
- Per-item progress tracking.
- SteamCMD output capture.
- Cancellation while a download is running.
- Retry only failed items.
- Optional full validation for troubleshooting.
- Automatic library refresh after completed downloads.
- Fallback flow when the embedded Steam Workshop WebView does not render correctly.

### Dependency Handling

- Detect missing mod dependencies.
- Show dependency warnings before activation/deactivation.
- Open Workshop search/pages for dependency lookup.
- Download dependencies through SteamCMD when a Workshop ID is available.
- Block unsafe server tests when required dependencies are missing.

### Diagnostics And Logs

- Controlled server startup tests.
- Real-time log streaming.
- Preflight checks for active mods, dependencies, load order, build compatibility,
  and configured ports.
- Port conflict detection.
- Option to stop conflicting processes before retrying a local test.
- Local and remote server log browser.
- Log preview, filtering/highlighting, copy, and refresh actions.

### Remote Linux Server Operations

- Upload/configure the remote helper.
- Install app-managed SteamCMD or use an existing SteamCMD path.
- Install or point to an existing Project Zomboid dedicated server folder.
- Configure systemd service/socket templates.
- Check remote firewall/setup readiness.
- Start a server normally.
- Start a server with `-nosteam`.
- Avoid duplicating `-nosteam` when the launcher already includes it.
- Stream startup output through the app.
- Send commands through the FIFO command channel.
- Stop servers with save/quit behavior.
- Poll remote server status.
- Open a remote terminal from the app.
- Upload selected local mods to the remote server.
- Deploy a local server profile and its active local mods to the remote server via SCP.

### Settings

- Language preference: automatic, English, or Brazilian Portuguese.
- SteamCMD path/status and download concurrency.
- Project Zomboid executable and memory settings.
- Client RAM and server RAM presets.
- Custom mod locations.
- Cache refresh and full rescan actions.

## Version 0.5.0 Release Notes

Version `0.5.0` focuses on making the app more useful for real server operators,
especially remote Linux server admins.

Key changes:

- Remote startup modal now has two launch paths: normal start and `-nosteam`.
- Remote helper propagates launch options to generated server startup scripts.
- Linux launch script generation handles `-nosteam` without duplicating the flag.
- Workshop mapping sync was hardened to merge remote and local data instead of
  replacing good local data with partial responses.
- Mapping sync failures now remain visible to the user while still preserving
  local-cache behavior.
- Mapping API endpoint and Tauri CSP were aligned for the configured HTTP-only
  service.
- Production dependency audit is clean with `npm audit --omit=dev`.
- Rust clippy passes with warnings denied.
- Unit tests cover normal and `-nosteam` script generation paths.

## Development Commands

```powershell
npm install
npm run dev
npm run build
npm run tauri:dev
npm run tauri:build
```

## Validation Commands

```powershell
npm run build
npm audit --omit=dev
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
cd ..
```

## Repository Notes

- Prefer small, clear changes with a commit after each meaningful update.
- Commit messages should follow the repository template in `.gitmessage.txt`
  when working from a full clone.
- Do not move release tags unless explicitly requested.
- For release builds, make sure generated desktop artifacts and the Linux helper
  are produced by CI from the intended commit.
