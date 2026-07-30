# Changelog

Update history for **PZ Manager / Project Zomboid Mod Manager**.

Dates use the `YYYY-MM-DD` format. This file summarizes what was added,
improved, and fixed in each public release.

## [Unreleased]

- No changes recorded yet.

## [0.6.2] - 2026-07-29

Release v0.6.2 - Sub-Mods & Upload QoL Patch

### Fixed
- **Mod Deduplication**: Fixed an issue where sub-mods sharing the same Steam Workshop ID were incorrectly hidden from the Server Details screen. All sub-mods inside a workshop item are now properly displayed and can be enabled.
- **Search State**: The search bar is now automatically cleared when switching between the Mods and Servers tabs, preventing confusion when filtering servers with mod names.

### Changed
- **Mod Uploading**: The "Upload Local Mods" modal now allows uploading mods downloaded via Steam Workshop in addition to mods located in the local `Zomboid/mods` folder.

## [0.6.1] - 2026-07-27

Release v0.6.1 - Build 42 Mod Detection Hotfix

### Fixed
- **Mod Discovery Engine**: Fixed an issue where Build 42 mods with their `mod.info` file located in the `common/` subdirectory were ignored. They are now correctly discovered and loaded by the application.

## [0.6.0] - 2026-07-25

Release v0.6.0 - Automated Server Versioning, Setup UI Simplification and Dependency Fixes

### Added
- Automated Server Versioning: the app now remembers the server version (B41/B42) during remote setup and persists it, streamlining the "Create Server" UI for remote workspaces.

### Improved
- Remote Server Compatibility: Drastically improved tolerance and detection of pre-existing Project Zomboid installations on Linux. The setup process now correctly identifies existing setups without forcing reinstalls or overwriting custom paths.
- Setup UI: Improved the Remote Setup modal to be more intuitive regarding existing installations and version detection.

### Fixed
- Dependency Resolution: Fixed a bug where downloading a missing dependency for a mod would install the dependency but fail to activate the original mod.
- Server Status: Addressed issues with false positives in server running state.

## [0.5.0] - 2026-07-20

Release focused on real server operations, especially remote Linux servers and
large Steam Workshop mod lists.

### Added

- Community `Mod ID -> Workshop ID` mapping database.
- Workshop mapping sync with local cache support.
- Automatic background upload of locally discovered mappings.
- Visible sync error when the Workshop mapping database fails to synchronize.
- Tools to edit Workshop IDs directly from mod details.
- Assistant for fixing missing Workshop IDs in active server mods.
- Remote startup modal with two buttons:
  - `Start server`;
  - `Start -nosteam`.
- Remote `-nosteam` support in the Linux helper.
- Clear logs when a server is started with `-nosteam`.
- Unit tests for normal and `-nosteam` startup script generation.
- In-app server log viewer.
- Local and remote log previews.
- Log filtering plus copy/refresh actions.
- Remote startup control with real-time output streaming.
- Remote command channel for sending server commands through FIFO.
- Remote stop flow using `save` and `quit`.
- Upload of selected local mods to a remote server.
- Deployment of a local server profile to a remote Linux server with its active mods.

### Improved

- Remote and local Workshop mappings are merged instead of replacing good local
  data with partial remote responses.
- The app keeps using the local cache when the mapping service is unavailable.
- `Mods=` and `WorkshopItems=` handling is safer for active lists with repaired IDs.
- Server test preflight is stricter for dependencies, load order, B41/B42
  compatibility, and port conflicts.
- Remote startup reuses the same firewall, logs, status, command, and stop flow
  for both normal and `-nosteam` starts.
- Tauri CSP was updated to allow the configured HTTP Workshop mapping service.
- Main documentation, Brazilian Portuguese documentation, and project summary
  were updated for version `0.5.0`.

### Fixed

- False visual failure during Workshop ID sync when the endpoint returned valid
  data in a shape the app did not handle.
- Loss of local mappings when the app automatically uploaded items that were not
  registered in the API yet.
- Duplicate `-nosteam` flag when the original launcher already contained it.
- `cargo clippy` warning related to log sorting.
- Unnecessary runtime npm dependency in the production package.

### Validation

- `npm run build`
- `npm audit --omit=dev`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

## [0.4.0] - 2026-07-05

Release focused on Linux remote workspaces, cross-platform compatibility, and
Windows/Linux release build preparation.

### Added

- Workspace selector when opening the app.
- Local workspace for the existing Windows flow.
- Remote workspace for Linux servers over SSH.
- Saved SSH connection profiles.
- Connectivity test before entering a remote workspace.
- Linux helper for remote operations.
- Remote setup screens.
- Initial support for remote server lifecycle control.
- Command sending to a running remote server.
- Remote server log streaming.
- Remote mod and server caches.
- Linux desktop build support.
- GitHub Actions release workflows for Windows and Linux assets.
- Linux helper bundle `pzmm-helper-linux-x86_64`.
- Full localization for workspace, SSH, and OpenSSH guide screens.
- Safe deletion of local or remote mods from the library.
- Remote dashboard metrics, including ping and connected players formatted as `X/Y`.

### Improved

- SSH errors now show friendly translated messages.
- Folder scanning was optimized to respect configured paths.
- Local workspaces no longer show unnecessary remote metrics.
- Remote dashboard counters and icons were clarified.
- CI compatibility across Windows and Linux runners was improved.

### Notes

- Linux remote support was still experimental in this version.
- The initial remote target was Ubuntu/Debian with `systemd`.
- Remote Windows servers should be accessed through RDP, running the app locally
  inside the VM.

## [0.3.0] - 2026-06-16

Release focused on mod library performance and reducing repeated scans.

### Added

- Persistent backend cache for the mod library.
- Reuse of the cached library during preflight validation.
- Full library rescan action that clears frontend and backend caches.
- Fast cache revalidation based on relevant mod package files.

### Improved

- Mod listing is faster for large libraries.
- The Settings screen hydrates from the last known settings, reducing flicker.
- Local mod images load correctly through Tauri's asset protocol.
- CI compatibility for `cargo clippy` checks was improved.
- Release metadata and changelog were prepared for version `0.3.0`.

### Fixed

- Incorrect SteamCMD simultaneous download count displayed in Settings.
- Local mod images that stopped loading after mod listing optimizations.

## [0.2.0] - 2026-06-14

Release focused on editing server configuration from inside the app.

### Added

- Server configuration modal.
- Editing for common `.ini` options.
- `SandboxVars.lua` editing in a dedicated tab.
- Readable options for Sandbox values instead of raw numeric editing.
- Visual badge for default Sandbox option values.
- Grouping for Sandbox settings by base-game sections and mod sections.
- Better handling for `.lua` files related to the server profile.

### Improved

- Server creation interface was refreshed.
- Configuration flow became less dependent on manual file editing.
- Server profiles became easier to review before tests.

### Kept

- Server profile creation and management.
- Build 41 and Build 42 support.
- Mod activation, deactivation, and reordering.
- Automatic `.ini` updates.
- Missing dependency and invalid order detection.
- SteamCMD downloads.
- Startup diagnostics with real-time logs.
- Language selection between English, Brazilian Portuguese, and automatic detection.

## [0.1.0] - 2026-06-01

First public release of PZ Manager.

### Added

- Project Zomboid server profile creation and management.
- Initial Build 41 and Build 42 support.
- Active mod activation, deactivation, and reordering.
- Automatic server `.ini` writing.
- Missing dependency detection.
- Load order validation.
- Mod reading from local folders, Steam Workshop folders, and custom directories.
- Workshop item and public collection downloads through SteamCMD.
- Server startup diagnostics with real-time logs.
- Configured port checks before server tests.
- English, Brazilian Portuguese, and automatic language detection.

### Notes

- The project was mainly focused on Windows.
- Some features were still early and subject to change.

## Links

- [0.6.1](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.6.1)
- [0.6.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.6.0)
- [0.5.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.5.0)
- [0.4.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.4.0)
- [0.3.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.3.0)
- [0.2.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.2.0)
- [0.1.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.1.0)
