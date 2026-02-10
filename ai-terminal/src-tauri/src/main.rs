extern crate fix_path_env;

use ai_terminal_lib::command::types::command_manager::CommandManager;
use ai_terminal_lib::command::types::pty_manager::PtyManager;
use ai_terminal_lib::{command, ollama, utils};
use std::env;

fn main() {
    let _ = fix_path_env::fix();

    let command_manager = CommandManager::new();
    let pty_manager = PtyManager::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| Ok(()))
        .manage(command_manager)
        .manage(pty_manager)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            command::core::execute_command::execute_command,
            command::core::execute_command::execute_sudo_command,
            command::core::terminate_command::terminate_command,
            command::core::pty::pty_create_session,
            command::core::pty::pty_write,
            command::core::pty::pty_resize,
            command::core::pty::pty_close_session,
            utils::operating_system_utils::get_current_pid,
            command::autocomplete::autocomplete_command::autocomplete,
            utils::file_system_utils::get_working_directory,
            utils::file_system_utils::get_home_directory,
            ollama::model_request::request::ask_ai,
            ollama::model_request::request::get_models,
            ollama::model_request::request::switch_model,
            ollama::model_request::request::get_host,
            ollama::model_request::request::set_host,
            command::git_commands::git::get_git_branch,
            utils::operating_system_utils::get_system_environment_variables,
        ])
        .run(tauri::generate_context!())
        .expect("Error launcing AI Terminal");
}
