# Zomboid Server Mod Manager

## Overview

Zomboid Server Mod Manager is a desktop application for managing Project
Zomboid dedicated servers and their mod setup. The project is currently a
minimal Tauri scaffold with a Vite frontend, intended to evolve into a local
tool for server administration, mod organization, and configuration support.

## Project Info

- Name: Zomboid Server Mod Manager
- Package: `zomboid-server-mod-manager`
- Version: `0.0.1`
- Type: Desktop application
- Runtime shell: Tauri
- Frontend tooling: Vite
- Target platform: Windows first

## Goals

- Make Project Zomboid dedicated server setup easier to manage.
- Keep mod lists organized and easier to inspect.
- Reduce manual editing of server configuration files.
- Provide a local desktop interface for repeated server maintenance tasks.

## Planned Features

- Server profile management.
- Mod list organization.
- Workshop mod tracking.
- Server configuration editing.
- Validation for missing or inconsistent mod entries.
- Build and launch helpers for local server workflows.
- Clear status feedback for common setup issues.

## Current State

The project currently contains the base application structure:

- Vite frontend under `src/`.
- Tauri backend under `src-tauri/`.
- Basic project metadata in `package.json`.
- Tauri window configuration in `src-tauri/tauri.conf.json`.

## Development Commands

```powershell
npm install
npm run dev
npm run build
npm run tauri:dev
npm run tauri:build
```

## Notes

This project should prefer small, clear changes with commits after each
meaningful update. Commit messages should follow the repository template in
`.gitmessage.txt`.
