use crate::util::hide_command_window;
use std::{
    path::Path,
    process::{Command, Output},
};

pub(crate) trait CommandRunner {
    fn run(&self, command_text: &str, working_directory: Option<&Path>) -> Result<Output, String>;
}

pub(crate) fn run_shell_command(
    command_text: &str,
    working_directory: Option<&Path>,
) -> Result<Output, String> {
    platform_command_runner().run(command_text, working_directory)
}

fn platform_command_runner() -> Box<dyn CommandRunner> {
    #[cfg(windows)]
    {
        Box::new(WindowsCommandRunner)
    }

    #[cfg(not(windows))]
    {
        Box::new(UnixCommandRunner)
    }
}

#[cfg(windows)]
struct WindowsCommandRunner;

#[cfg(windows)]
impl CommandRunner for WindowsCommandRunner {
    fn run(&self, command_text: &str, working_directory: Option<&Path>) -> Result<Output, String> {
        let mut command = Command::new("powershell.exe");
        hide_command_window(&mut command).args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            command_text,
        ]);

        if let Some(working_directory) = working_directory {
            if !working_directory.is_dir() {
                return Err(format!(
                    "Working directory not found: {}.",
                    working_directory.display()
                ));
            }

            command.current_dir(working_directory);
        }

        command
            .output()
            .map_err(|error| format!("Could not run the command: {error}"))
    }
}

#[cfg(not(windows))]
struct UnixCommandRunner;

#[cfg(not(windows))]
impl CommandRunner for UnixCommandRunner {
    fn run(&self, command_text: &str, working_directory: Option<&Path>) -> Result<Output, String> {
        let mut command = Command::new("sh");
        command.args(["-lc", command_text]);

        if let Some(working_directory) = working_directory {
            if !working_directory.is_dir() {
                return Err(format!(
                    "Working directory not found: {}.",
                    working_directory.display()
                ));
            }

            command.current_dir(working_directory);
        }

        command
            .output()
            .map_err(|error| format!("Could not run the command: {error}"))
    }
}
