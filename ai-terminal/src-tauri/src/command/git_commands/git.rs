use crate::command::types::command_manager::CommandManager;
use crate::utils::file_system_utils::get_shell_path;
use std::process::Command;
use tauri::{command, State};

pub fn new_git_command() -> Command {
    let mut cmd = Command::new("git");
    if let Some(path_val) = get_shell_path() {
        if let Ok(current_path) = std::env::var("PATH") {
            let new_path = format!("{}{}{}", path_val, std::path::MAIN_SEPARATOR, current_path);
            cmd.env("PATH", new_path);
        } else {
            cmd.env("PATH", path_val);
        }
    }
    cmd
}

#[command]
pub fn get_git_branch(
    session_id: String,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    let current_dir = if let Some(state) = states.get(&key) {
        &state.current_dir
    } else {
        return Ok("".to_string());
    };

    // Get current branch
    let mut cmd = new_git_command();
    cmd.arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(current_dir);

    let output = cmd.output().map_err(|e| e.to_string())?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(branch)
    } else {
        Ok("".to_string())
    }
}
