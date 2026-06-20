import { invokeTauri } from "@/lib/tauri"

export type TerminalCommandTarget = "local" | "remote"

export type RemoteConnectionDraft = {
  name: string
  host: string
  port: string
  username: string
  authMethod: "password" | "key"
  password: string
  sshKeyPath: string
  serverPath: string
}

export type RemoteWorkspaceConfig = RemoteConnectionDraft & {
  remoteSteamcmdDir: string
  remoteSteamcmdPath: string
  remoteZomboidServerDir: string
  remoteZomboidServerPath: string
  remoteClientRam: string
  remoteServerRam: string
  remoteModLocations: string[]
}

export type TerminalCommandResult = {
  target: TerminalCommandTarget
  command: string
  exitCode: number | null
  success: boolean
  stdout: string
  stderr: string
}

export type CommandRunner = {
  target: TerminalCommandTarget
  unavailableReason?: string
  run: (command: string) => Promise<TerminalCommandResult>
}

type CommandRunnerConfig = {
  target: TerminalCommandTarget
  localWorkingDirectory: string
  remoteConnection: RemoteConnectionDraft
  isRemoteConnected: boolean
}

export function createCommandRunner(config: CommandRunnerConfig): CommandRunner {
  switch (config.target) {
    case "local":
      return createLocalCommandRunner(config.localWorkingDirectory)
    case "remote":
      return createRemoteCommandRunner(config.remoteConnection, config.isRemoteConnected)
  }
}

function createLocalCommandRunner(workingDirectory: string): CommandRunner {
  return {
    target: "local",
    run: (command: string) =>
      invokeTauri<TerminalCommandResult>("run_terminal_command", {
        request: {
          target: "local",
          command,
          workingDirectory,
          connection: null,
        },
      }),
  }
}

function createRemoteCommandRunner(
  connection: RemoteConnectionDraft,
  isConnected: boolean,
): CommandRunner {
  const unavailableReason = getRemoteUnavailableReason(connection, isConnected)

  return {
    target: "remote",
    unavailableReason,
    run: (command: string) =>
      invokeTauri<TerminalCommandResult>("run_terminal_command", {
        request: {
          target: "remote",
          command,
          workingDirectory: "",
          connection,
        },
      }),
  }
}

function getRemoteUnavailableReason(connection: RemoteConnectionDraft, isConnected: boolean) {
  if (connection.authMethod !== "key") {
    return "Remote command execution is prepared for SSH key authentication. Switch to a private key file to run commands on the server."
  }

  if (!isConnected) {
    return "Connect to the remote host before running SSH commands."
  }

  return undefined
}
