use crate::i18n::text;
use crate::util::hide_command_window;
use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
};

pub(crate) fn spawn_output_reader<R>(
    stream: Option<R>,
    label: &'static str,
    sender: mpsc::Sender<String>,
) where
    R: std::io::Read + Send + 'static,
{
    let Some(stream) = stream else {
        return;
    };

    thread::spawn(move || {
        let reader = BufReader::new(stream);

        for line in reader.lines().map_while(Result::ok) {
            let _ = sender.send(format!("[{label}] {line}"));
        }
    });
}

pub(crate) fn kill_process_tree(pid: u32) -> Result<(), String> {
    let mut command = Command::new("taskkill");

    hide_command_window(&mut command)
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            format!(
                "{}: {error}",
                text(
                    "Could not stop the test process",
                    "Nao foi possivel encerrar o processo do teste"
                )
            )
        })?;

    Ok(())
}

pub(super) fn kill_processes_by_pid_impl(pids: Vec<u32>) -> Result<(), String> {
    let mut seen = HashSet::new();

    for pid in pids {
        if pid == 0 || !seen.insert(pid) {
            continue;
        }

        kill_process_tree(pid)?;
    }

    Ok(())
}
