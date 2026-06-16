# Changelog

## 0.3.0

- Added a persistent backend cache for the mod library.
- Reused the cached library in server preflight validation.
- Added a full mod library rescan action that clears the backend and frontend caches.
- Reduced settings-screen flicker by hydrating the view from the last known settings.
- Fixed simultaneous SteamCMD download count display in settings.
- Kept local mod images loading through Tauri's asset protocol.
