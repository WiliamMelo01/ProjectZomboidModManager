[**English**](README.md) | [Português (Brasil)](README.pt-BR.md)

<div align="center">

# PZ Manager

### Manage Project Zomboid server mods, downloads, configs, and remote Linux servers from one desktop app.

[![Version](https://img.shields.io/badge/version-0.5.0-6d5dfc?style=for-the-badge)](package.json)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-0078D4?style=for-the-badge&logo=windows)
![Desktop](https://img.shields.io/badge/desktop-Tauri-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)
![Status](https://img.shields.io/badge/status-active%20development-F59E0B?style=for-the-badge)

</div>

---

## About

**PZ Manager** is a desktop application for running and maintaining **Project Zomboid** multiplayer server mod setups without hand-editing every profile file.

It can scan local and Steam Workshop mods, manage server profiles, write `Mods=` and `WorkshopItems=`, validate dependencies and load order, download Workshop content with SteamCMD, and operate remote Linux dedicated servers over SSH.

The app supports **Build 41** and **Build 42** profiles. Each server keeps its own build, mod list, Workshop items, configuration, logs, and runtime state.

## Version 0.5.0 Highlights

- **Community Workshop ID database**: syncs discovered `Mod ID -> Workshop ID` mappings with a shared service, keeps a local cache, and falls back visually when the service is unavailable.
- **Workshop ID repair tools**: edit IDs from mod details, fix missing Workshop IDs for active server mods, and keep `Mods=` aligned with `WorkshopItems=`.
- **Remote server start workflow**: check setup, configure Linux systemd/FIFO support, stream startup logs, send console commands, stop the server, and inspect status.
- **Start with `-nosteam`**: remote servers now have both a normal start button and a `Start -nosteam` button.
- **Remote SteamCMD and server setup**: install or reuse SteamCMD, install or point to the Project Zomboid dedicated server, and configure the helper on Linux.
- **Release hardening**: stricter CSP for the mapping service, clean production npm audit, Rust clippy with warnings denied, and unit coverage for the new launch scripts.

## Feature Overview

| Area | What you can do |
| --- | --- |
| **Workspaces** | Choose a local Windows workspace or connect to a remote Linux workspace over SSH with saved connection profiles. |
| **Remote setup** | Upload/configure the Linux helper, install SteamCMD, install or reuse the dedicated server folder, and verify required setup steps. |
| **Servers** | Create profiles, list existing servers, search, hide/show profiles, favorite servers, delete with safety backup, and clone mod lists from another server. |
| **Server configuration** | Edit common `.ini` server settings, public name/description, ports, player limits, PvP, backups, world settings, and SandboxVars where supported. |
| **B41 and B42** | Pick a build per server, switch builds with confirmation, detect incompatible mods, and preserve B42 versioned package layouts. |
| **Active mods** | Activate, deactivate, reorder, paginate, filter, inspect, and validate mods before writing the server profile. |
| **Dependencies** | Detect missing dependencies, protect dependency load order, install downloaded dependencies, and block unsafe server tests. |
| **Workshop IDs** | Resolve IDs from local metadata, manual edits, `.pzmm-workshop-id` markers, local cache, and the shared mapping database. |
| **Mod library** | Scan `Zomboid/mods`, Steam Workshop libraries, remote folders, and custom mod locations with cached results and image thumbnails. |
| **Downloads** | Download individual Workshop items or public collections with SteamCMD, per-item progress, cancellation, retry failed items, and optional validation. |
| **Workshop fallback** | Open Workshop pages/search when the embedded WebView cannot render Steam correctly and continue the download flow with SteamCMD. |
| **Remote mod upload** | Upload selected local mods to a remote Linux server and deploy a local server profile with its active mods through SCP. |
| **Server diagnostics** | Run controlled startup tests, stream logs, validate ports, find port conflicts, and optionally stop conflicting processes before retrying. |
| **Remote runtime control** | Start normally, start with `-nosteam`, monitor startup, send server commands through the command channel, stop safely, and poll status. |
| **Logs** | Browse local or remote server log files, preview log output, filter useful lines, copy logs, and refresh from the app. |
| **Settings** | Configure SteamCMD, custom mod directories, language, client/server RAM, executable paths, and download concurrency. |
| **Localization** | Use English or Brazilian Portuguese with automatic system-language detection and instant switching. |

## Remote Linux Management

Remote workspaces are designed for Linux dedicated servers. After connecting over SSH, the app can configure an app-managed workspace under the remote machine, upload the helper binary, and use that helper to run server-management commands.

Remote capabilities include:

- listing and editing server profiles;
- scanning remote mods and Workshop folders;
- installing SteamCMD or using an existing SteamCMD path;
- downloading the Project Zomboid dedicated server through SteamCMD;
- configuring systemd service/socket templates for startup and command input;
- checking firewall/setup readiness before launch;
- starting normally or with `-nosteam`;
- streaming `journalctl` startup output;
- sending console commands through the FIFO command channel;
- stopping with save/quit behavior;
- opening a remote terminal from the app;
- uploading local mods or deploying a whole local server profile.

The `-nosteam` path only changes the launch options. Firewall rules, logs, command channel, ports, status checks, and stop behavior stay the same as a normal remote start.

## Workshop ID Database

Project Zomboid server profiles need both `Mods=` and `WorkshopItems=`, but many installed mods only expose the Mod ID locally. PZ Manager keeps these relationships in a local mapping database and can sync known pairs with a shared HTTP mapping service.

The mapping sync stores only `Mod ID -> Workshop ID` pairs. It does not upload server profiles, configuration files, credentials, or mod files. If the service is offline or returns invalid data, the app shows a visual sync error and keeps using the local cache.

You can also fix IDs manually from mod details or use the missing Workshop ID assistant on a server profile.

## B41 and B42 Support

Existing profiles without metadata open as **B41**. New profiles can be created as `B41` or `B42`, and the build can be changed later with confirmation.

For B42 package layouts, PZ Manager preserves versioned folders and shared `common` content:

```text
mods/
└── ExampleMod/
    ├── common/
    ├── 42/
    │   └── mod.info
    └── 42.17/
        └── mod.info
```

When enabling mods:

- B41 profiles write the traditional Mod ID to `Mods=`.
- B42 profiles write the compatible variant ID.
- `WorkshopItems=` keeps unique Workshop IDs.
- incompatible mods remain visible for manual review/removal.
- preflight checks block missing dependencies, invalid order, and incompatible mods before tests.

## Library, Downloads, and Deployment

The library is built from installed local mods, Steam Workshop folders, remote server folders, and custom paths added in Settings. Cached metadata and thumbnails make repeat scans faster.

Downloads accept a numeric Workshop ID or Workshop URL:

- single Workshop item or public collection;
- per-item status and SteamCMD output;
- cancellation while downloading;
- retry only failed items;
- optional full validation;
- automatic library refresh when finished.

When moving a Workshop mod into the local Zomboid folder, PZ Manager copies the complete package and preserves B41 variants, B42 versioned directories, shared `common` content, dependencies, maps, and the `.pzmm-workshop-id` marker.

For remote servers, selected local mods can be uploaded to the Linux host. A local server profile can also be packaged and deployed with its active local mods.

## Server Tests and Logs

The server test flow starts the server in a controlled mode and streams output in real time. Before starting, it:

1. validates active mods and dependencies;
2. checks dependency load order;
3. checks build compatibility;
4. verifies configured ports;
5. reports conflicts and can stop conflicting processes before retrying.

B42 receives a longer startup timeout because it can take more time to initialize.

The log viewer can browse available server log files, preview local or remote logs, highlight relevant lines, copy output, and refresh without leaving the server detail screen.

## Getting Started

### Local Workspace

1. Open **Settings** and confirm the Project Zomboid executable and SteamCMD paths.
2. Add custom mod folders if you store mods outside the default locations.
3. Refresh the mod library.
4. Create or open a server profile.
5. Choose the build, configure server settings, and activate mods.
6. Fix missing Workshop IDs or dependencies if prompted.
7. Run a startup test before hosting.

### Remote Workspace

1. Choose **Remote Workspace** and connect to the Linux server over SSH.
2. Configure the remote helper from the setup guide.
3. Install or select SteamCMD.
4. Install or select the Project Zomboid dedicated server folder.
5. Create or import server profiles.
6. Upload/deploy local mods when needed.
7. Start the server normally or with `-nosteam`, then monitor logs and send commands from the app.

## Interface

<p align="center">
  <a href="docs/images/server.png"><img src="docs/images/server.png" alt="Server list" width="48%"></a>
  <a href="docs/images/server-detail.png"><img src="docs/images/server-detail.png" alt="Server details" width="48%"></a>
</p>

<p align="center">
  <a href="docs/images/mods.png"><img src="docs/images/mods.png" alt="Mod library" width="48%"></a>
  <a href="docs/images/download-mod.png"><img src="docs/images/download-mod.png" alt="Workshop mod download" width="48%"></a>
</p>

<p align="center">
  <a href="docs/images/download-collection.png"><img src="docs/images/download-collection.png" alt="Workshop collection download" width="48%"></a>
  <a href="docs/images/settings.png"><img src="docs/images/settings.png" alt="Mod and SteamCMD settings" width="48%"></a>
</p>

<p align="center">
  <a href="docs/images/performance.png"><img src="docs/images/performance.png" alt="Performance settings" width="48%"></a>
  <a href="docs/images/server-test-success.png"><img src="docs/images/server-test-success.png" alt="Completed server test" width="48%"></a>
</p>

<p align="center">
  <a href="docs/images/server-test-error.png"><img src="docs/images/server-test-error.png" alt="Server dependency validation" width="48%"></a>
</p>

## Development

### Prerequisites

- Windows 10/11 or Linux (Ubuntu/Debian)
- [Node.js](https://nodejs.org/) v20+ or v22+ with npm
- [Rust](https://www.rust-lang.org/tools/install) latest stable
- [Tauri prerequisites for Windows](https://v2.tauri.app/start/prerequisites/) or [Tauri prerequisites for Linux](https://v2.tauri.app/start/prerequisites/)
- Project Zomboid installed for local features
- SSH access to a Linux server for remote workspace features

### Running Locally

```bash
npm install
npm run tauri:dev
```

To work only on the interface:

```bash
npm run dev
```

To generate the desktop build and the Linux server helper:

```bash
npm run tauri:build
```

### Validation

```bash
npm run build
npm audit --omit=dev
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
cd ..
```

## Technologies

| Layer | Technologies |
| --- | --- |
| Interface | React 19, TypeScript, and Vite 8 |
| Styling | Tailwind CSS 4 |
| Components and icons | Base UI and Lucide React |
| Desktop application | Tauri 2 |
| Local backend | Rust |
| Remote server agent | Rust helper binary (`pzmm-helper-linux-x86_64`) |
| Remote transport | SSH and SCP |
| Server service control | Linux systemd service/socket and FIFO command channel |
| Workshop downloads | SteamCMD |
| Internationalization | i18next, react-i18next, and rust-i18n |

## Project Structure

```text
.
├── resources/             # Example files and bundled resources
├── src/                   # React interface, components, types, and frontend catalogs
├── src-tauri/
│   ├── locales/           # Backend rust-i18n catalogs
│   └── src/               # Rust backend, helper, and Tauri commands
├── package.json           # Frontend dependencies and scripts
├── README.pt-BR.md        # Brazilian Portuguese documentation
└── README.md              # Main English documentation
```

## Current Status

The project is under active development. Version 0.5.0 expands PZ Manager from a local mod/profile manager into a broader server-management tool with remote Linux operations, Workshop ID sync, SteamCMD workflows, diagnostics, logs, and safer startup controls.

## License

This repository does not have a license file yet. Before reusing or redistributing the code, confirm the applicable terms with the project author.
