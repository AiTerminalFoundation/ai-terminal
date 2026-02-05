use crate::command::types::command_manager::CommandManager;
use crate::command::types::command_state::CommandState;
use crate::utils::file_system_utils::get_shell_path;
use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::{env, thread};
use tauri::{command, AppHandle, Emitter, Manager, State};

#[command]
pub fn execute_command(
    command: String,
    session_id: String,
    ssh_password: Option<String>,
    app_handle: AppHandle,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    const SSH_NEEDS_PASSWORD_MARKER: &str = "SSH_INTERACTIVE_PASSWORD_PROMPT_REQUESTED";
    const SSH_PRE_EXEC_PASSWORD_EVENT: &str = "ssh_pre_exec_password_request";
    const COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER: &str = "COMMAND_FORWARDED_TO_ACTIVE_SSH";

    // Phase 1: Check and handle active SSH session
    {
        let mut states_guard = command_manager.commands.lock().map_err(|e| e.to_string())?;

        let state = get_command_state(&mut states_guard, session_id.clone());

        if state.is_ssh_session_active {
            if let Some(stdin_arc_for_thread) = state.child_stdin.clone() {
                let active_pid_for_log = state.pid.unwrap_or(0);

                if let Err(e) = app_handle.emit("command_forwarded_to_ssh", command.clone()) {
                    eprintln!(
                        "[Rust EXEC DEBUG] Failed to emit command_forwarded_to_ssh: {}",
                        e
                    );
                }

                let app_handle_clone_for_thread = app_handle.clone();
                let command_clone_for_thread = command.clone();
                let session_id_clone_for_thread = session_id.clone();

                thread::spawn(move || {
                    let command_manager_state_for_thread =
                        app_handle_clone_for_thread.state::<CommandManager>();

                    let mut stdin_guard = match stdin_arc_for_thread.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            if let Ok(mut states_lock_in_thread) =
                                command_manager_state_for_thread.commands.lock()
                            {
                                if let Some(s) =
                                    states_lock_in_thread.get_mut(&session_id_clone_for_thread)
                                {
                                    if s.pid == Some(active_pid_for_log) && s.is_ssh_session_active
                                    {
                                        s.is_ssh_session_active = false;
                                        s.child_stdin = None;
                                        s.remote_current_dir = None;
                                    }
                                }
                            }
                            let _ = app_handle_clone_for_thread.emit("ssh_session_ended", serde_json::json!({ "pid": active_pid_for_log, "reason": format!("SSH session error (stdin lock): {}", e)}));
                            let _ = app_handle_clone_for_thread.emit(
                                "command_error",
                                format!(
                                    "Failed to send to SSH (stdin lock '{}'): {}",
                                    command_clone_for_thread, e
                                ),
                            );
                            let _ =
                                app_handle_clone_for_thread.emit("command_end", "Command failed.");
                            return;
                        }
                    };

                    let is_remote_cd = command_clone_for_thread.trim().starts_with("cd ");
                    let actual_command_to_write_ssh = if is_remote_cd {
                        let marker = format!(
                            "__REMOTE_CD_PWD_MARKER_{}__",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs_f64()
                                .to_string()
                                .replace('.', "")
                        );
                        let cd_command_part = command_clone_for_thread.trim();
                        format!(
                            "{} && printf '%s\\n' '{}' && pwd && printf '%s\\n' '{}'\n",
                            cd_command_part, marker, marker
                        )
                    } else {
                        format!("{}\n", command_clone_for_thread)
                    };

                    let write_attempt =
                        stdin_guard.write_all(actual_command_to_write_ssh.as_bytes());

                    let final_result = if write_attempt.is_ok() {
                        stdin_guard.flush()
                    } else {
                        write_attempt
                    };

                    if let Err(e) = final_result {
                        if let Ok(mut states_lock_in_thread) =
                            command_manager_state_for_thread.commands.lock()
                        {
                            if let Some(s) =
                                states_lock_in_thread.get_mut(&session_id_clone_for_thread)
                            {
                                if s.pid == Some(active_pid_for_log) && s.is_ssh_session_active {
                                    s.is_ssh_session_active = false;
                                    s.child_stdin = None;
                                    s.remote_current_dir = None;
                                }
                            }
                        }
                        let _ = app_handle_clone_for_thread.emit("ssh_session_ended", serde_json::json!({ "pid": active_pid_for_log, "reason": format!("SSH session ended (stdin write/flush error): {}", e)}));
                        let _ = app_handle_clone_for_thread.emit(
                            "command_error",
                            format!(
                                "Failed to send to SSH (stdin write/flush '{}'): {}",
                                command_clone_for_thread, e
                            ),
                        );
                        let _ = app_handle_clone_for_thread.emit("command_end", "Command failed.");
                    }
                });

                drop(states_guard);
                return Ok(COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER.to_string());
            } else {
                // state.child_stdin is None, but state.is_ssh_session_active was true
                let active_pid_for_log = state.pid.unwrap_or(0);
                state.is_ssh_session_active = false;
                state.pid = None; // Clear PID as session is now considered broken
                state.remote_current_dir = None;
                drop(states_guard);
                let _ = app_handle.emit("ssh_session_ended", serde_json::json!({ "pid": active_pid_for_log, "reason": "SSH session inconsistency: active but no stdin."}));
                return Err("SSH session conflict: active but no stdin. Please retry.".to_string());
            }
        }
    }

    // Phase 2: Handle 'cd' command (if not in an SSH session)
    // The `cd` command logic remains largely the same, it acquires its own lock.
    if command.starts_with("cd ") || command == "cd" {
        // This block is the original 'cd' handling logic.
        // It will lock `command_manager.commands` internally.
        let mut states_guard_cd = command_manager.commands.lock().map_err(|e| e.to_string())?;
        let command_state_cd = get_command_state(&mut states_guard_cd, session_id.clone());

        let path = command.trim_start_matches("cd").trim();
        if path.is_empty() || path == "~" || path == "~/" {
            return if let Some(home_dir) = dirs::home_dir() {
                let home_path = home_dir.to_string_lossy().to_string();
                command_state_cd.current_dir = home_path.clone();
                drop(states_guard_cd); // Release lock before emitting and returning
                let _ = app_handle.emit("command_end", "Command completed successfully.");
                Ok(format!("Changed directory to {}", home_path))
            } else {
                drop(states_guard_cd);
                let _ = app_handle.emit("command_end", "Command failed.");
                Err("Could not determine home directory".to_string())
            };
        }
        let current_path = Path::new(&command_state_cd.current_dir);
        let new_path = if path.starts_with('~') {
            if let Some(home_dir) = dirs::home_dir() {
                let without_tilde = path.trim_start_matches('~');
                let rel_path = without_tilde.trim_start_matches('/');
                if rel_path.is_empty() {
                    home_dir
                } else {
                    home_dir.join(rel_path)
                }
            } else {
                drop(states_guard_cd);
                return Err("Could not determine home directory".to_string());
            }
        } else if path.starts_with('/') {
            std::path::PathBuf::from(path)
        } else {
            let mut result_path = current_path.to_path_buf();
            let path_components: Vec<&str> = path.split('/').collect();
            for component in path_components {
                if component == ".." {
                    if let Some(parent) = result_path.parent() {
                        result_path = parent.to_path_buf();
                    } else {
                        drop(states_guard_cd);
                        let _ = app_handle.emit("command_end", "Command failed.");
                        return Err("Already at root directory".to_string());
                    }
                } else if component != "." && !component.is_empty() {
                    result_path = result_path.join(component);
                }
            }
            result_path
        };
        return if new_path.exists() {
            command_state_cd.current_dir = new_path.to_string_lossy().to_string();
            let current_dir_for_ok = command_state_cd.current_dir.clone();
            drop(states_guard_cd);
            let _ = app_handle.emit("command_end", "Command completed successfully.");
            Ok(format!("Changed directory to {}", current_dir_for_ok))
        } else {
            drop(states_guard_cd);
            let _ = app_handle.emit("command_end", "Command failed.");
            Err(format!("Directory not found: {}", path))
        };
    }

    // Phase 3: Prepare for and execute new command (local or new SSH)
    let current_dir_clone = {
        let mut states_guard_dir = command_manager.commands.lock().map_err(|e| e.to_string())?;
        let state_dir = get_command_state(&mut states_guard_dir, session_id.clone());
        state_dir.current_dir.clone()
    }; // Lock for current_dir released.

    // Proactive SSH password handling (if not in an SSH session)
    let is_plain_ssh_attempt =
        command.contains("ssh ") && !command.trim_start().starts_with("sudo ssh ");
    if is_plain_ssh_attempt && ssh_password.is_none() {
        app_handle
            .emit(SSH_PRE_EXEC_PASSWORD_EVENT, command.clone())
            .map_err(|e| e.to_string())?;
        return Ok(SSH_NEEDS_PASSWORD_MARKER.to_string());
    }

    let mut command_to_run = command.clone();
    let app_handle_clone = app_handle.clone();

    let mut env_map: HashMap<String, String> = std::env::vars().collect();
    if !env_map.contains_key("PATH") {
        if let Some(path_val) = get_shell_path() {
            env_map.insert("PATH".to_string(), path_val);
        }
    }

    // let script_path_option: Option<String> = None; // Removed unused variable

    // This flag determines if the command we are about to spawn *could* start a persistent SSH session
    let is_potential_ssh_session_starter = is_plain_ssh_attempt;

    let original_command_is_sudo = command.trim_start().starts_with("sudo ");
    let original_command_is_sudo_ssh = command.trim_start().starts_with("sudo ssh ");

    let mut cmd_to_spawn: Command;
    let mut child: Child;

    // Prepare command_to_run if it's an SSH command, before deciding on sshpass
    if is_potential_ssh_session_starter && !original_command_is_sudo_ssh {
        // Avoid mangling "sudo ssh ..." here
        let original_command_parts: Vec<&str> = command.split_whitespace().collect();
        let mut first_non_option_idx_after_ssh: Option<usize> = None;

        // Find the first argument after "ssh" that doesn't start with '-'
        // This helps distinguish `ssh host` from `ssh host remote_command`
        let ssh_keyword_idx = original_command_parts.iter().position(|&p| p == "ssh");

        if let Some(idx_ssh) = ssh_keyword_idx {
            for i in (idx_ssh + 1)..original_command_parts.len() {
                if !original_command_parts[i].starts_with('-') {
                    first_non_option_idx_after_ssh = Some(i);
                    break;
                }
            }

            let is_likely_interactive_ssh = match first_non_option_idx_after_ssh {
                Some(idx) => idx == original_command_parts.len() - 1, // True if the first non-option (host) is the last part
                None => false, // e.g., "ssh -p 22" without host, or just "ssh"
            };

            let ssh_options_prefix = "ssh -t -t -o StrictHostKeyChecking=accept-new";
            // Arguments are everything after "ssh" in the original command
            let args_after_ssh_keyword_in_original = original_command_parts
                .iter()
                .skip(idx_ssh + 1)
                .cloned()
                .collect::<Vec<&str>>()
                .join(" ");

            if is_likely_interactive_ssh {
                // For interactive: ssh -options user@host
                command_to_run = format!(
                    "{} {}",
                    ssh_options_prefix,
                    args_after_ssh_keyword_in_original.trim_end()
                );
            } else if first_non_option_idx_after_ssh.is_some() {
                // For non-interactive (ssh user@host remote_command): ssh -options user@host remote_command
                command_to_run = format!(
                    "{} {}",
                    ssh_options_prefix, args_after_ssh_keyword_in_original
                );
            } else {
                // Could be just "ssh" or "ssh -options", keep as is but with prefix, though likely won't connect
                command_to_run = format!(
                    "{} {}",
                    ssh_options_prefix, args_after_ssh_keyword_in_original
                );
            }
        }
    }

    // Now, use the (potentially transformed) command_to_run for direct/sshpass spawning
    if is_potential_ssh_session_starter && !original_command_is_sudo {
        let executable_name: String;
        let mut arguments: Vec<String> = Vec::new();

        if let Some(password_value) = ssh_password {
            executable_name = "sshpass".to_string();
            arguments.push("-p".to_string());
            arguments.push(password_value); // password_value is a String, gets moved here
                                            // command_to_run is the full "ssh -t -t ..." string
            arguments.extend(command_to_run.split_whitespace().map(String::from));
        } else {
            // No password provided: use plain ssh
            // command_to_run is already "ssh -t -t ..."
            let parts: Vec<String> = command_to_run
                .split_whitespace()
                .map(String::from)
                .collect();
            if parts.is_empty() || parts[0] != "ssh" {
                return Err(format!(
                    "Failed to parse SSH command for direct execution: {}",
                    command_to_run
                ));
            }
            executable_name = parts[0].clone(); // Should be "ssh"
            arguments.extend(parts.iter().skip(1).cloned());
        }

        cmd_to_spawn = Command::new(&executable_name);
        for arg in &arguments {
            cmd_to_spawn.arg(arg);
        }

        // env_map is passed as is. If SSH_ASKPASS was in it from a broader environment,
        // sshpass should take precedence or ssh (in key auth) would ignore it if not needed.
        cmd_to_spawn
            .current_dir(&current_dir_clone)
            .envs(&env_map)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        // setsid() was removed here in a previous step, which is good.

        child = match cmd_to_spawn.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Err(format!(
                    "Failed to start direct command ({}): {}",
                    executable_name, e
                ))
            }
        };
    } else {
        // Fallback to sh -c for non-SSH or sudo commands
        let final_shell_command = if original_command_is_sudo && !original_command_is_sudo_ssh {
            command_to_run.clone()
        } else {
            format!("exec {}", command_to_run)
        };

        let mut sh_cmd_to_spawn = Command::new("sh");
        sh_cmd_to_spawn
            .arg("-c")
            .arg(&final_shell_command)
            .current_dir(&current_dir_clone)
            .envs(&env_map)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped()); // Ensure stdin is piped for sh -c as well

        #[cfg(unix)]
        unsafe {
            sh_cmd_to_spawn.pre_exec(|| match nix::unistd::setsid() {
                Ok(_) => Ok(()),
                Err(e) => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("setsid failed: {}", e),
                )),
            });
        }

        child = match sh_cmd_to_spawn.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to start command via sh -c: {}", e)),
        };
    }

    let pid = child.id();
    // Take IO handles before moving child into Arc<Mutex<Child>>
    let child_stdin_handle = child.stdin.take().map(|stdin| Arc::new(Mutex::new(stdin)));
    let child_stdout_handle = child.stdout.take();
    let child_stderr_handle = child.stderr.take();
    let child_wait_handle_arc = Arc::new(Mutex::new(child)); // Now 'child' has no IO handles
    let session_id_for_wait_thread = session_id.clone();

    {
        let mut states_guard_update = command_manager.commands.lock().map_err(|e| e.to_string())?;
        let state_to_update = get_command_state(&mut states_guard_update, session_id.clone());

        state_to_update.pid = Some(pid);
        state_to_update.child_wait_handle = Some(child_wait_handle_arc.clone()); // Store wait handle

        if is_potential_ssh_session_starter {
            state_to_update.child_stdin = child_stdin_handle; // Store stdin handle for SSH
            state_to_update.is_ssh_session_active = true;
            state_to_update.remote_current_dir = Some("remote:~".to_string()); // Initial placeholder
            let _ = app_handle_clone.emit("ssh_session_started", serde_json::json!({ "pid": pid }));

            // Attempt to send initial PWD command
            if let Some(stdin_arc_for_init_pwd) = state_to_update.child_stdin.clone() {
                let app_handle_for_init_pwd_thread = app_handle_clone.clone(); // Clone app_handle for the thread
                let initial_pid_for_init_pwd_error = pid;
                let session_id_for_init_pwd_thread = session_id.clone();

                thread::spawn(move || {
                    // Get CommandManager state inside the thread using the moved app_handle
                    let command_manager_state_for_thread =
                        app_handle_for_init_pwd_thread.state::<CommandManager>();

                    let initial_pwd_marker = format!(
                        "__INITIAL_REMOTE_PWD_MARKER_{}__",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64()
                            .to_string()
                            .replace('.', "")
                    );
                    let initial_pwd_command = format!(
                        "echo '{}'; pwd; echo '{}'\n",
                        initial_pwd_marker, initial_pwd_marker
                    );

                    match stdin_arc_for_init_pwd.lock() {
                        Ok(mut stdin_guard) => {
                            if let Err(e) = stdin_guard
                                .write_all(initial_pwd_command.as_bytes())
                                .and_then(|_| stdin_guard.flush())
                            {
                                if let Ok(mut states_lock) =
                                    command_manager_state_for_thread.commands.lock()
                                {
                                    // Use state obtained within the thread
                                    if let Some(s) =
                                        states_lock.get_mut(&session_id_for_init_pwd_thread)
                                    {
                                        if s.pid == Some(initial_pid_for_init_pwd_error)
                                            && s.is_ssh_session_active
                                        {
                                            s.is_ssh_session_active = false;
                                            s.child_stdin = None;
                                            s.remote_current_dir = None;
                                            let _ = app_handle_for_init_pwd_thread.emit("ssh_session_ended", serde_json::json!({ "pid": initial_pid_for_init_pwd_error, "reason": format!("SSH session error (initial PWD send for pid {}): {}", initial_pid_for_init_pwd_error, e)}));
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if let Ok(mut states_lock) =
                                command_manager_state_for_thread.commands.lock()
                            {
                                // Use state obtained within the thread
                                if let Some(s) =
                                    states_lock.get_mut(&session_id_for_init_pwd_thread)
                                {
                                    if s.pid == Some(initial_pid_for_init_pwd_error)
                                        && s.is_ssh_session_active
                                    {
                                        s.is_ssh_session_active = false;
                                        s.child_stdin = None;
                                        s.remote_current_dir = None;
                                        let _ = app_handle_for_init_pwd_thread.emit("ssh_session_ended", serde_json::json!({ "pid": initial_pid_for_init_pwd_error, "reason": format!("SSH session error (initial PWD stdin lock for pid {}): {}", initial_pid_for_init_pwd_error, e)}));
                                    }
                                }
                            }
                        }
                    }
                });
            }
        } else {
            state_to_update.is_ssh_session_active = false;
            state_to_update.child_stdin = None; // Ensure stdin is None for non-SSH commands
            state_to_update.remote_current_dir = None; // Ensure remote_dir is None for non-SSH
        }
    } // states_guard_update lock released

    if let Some(stdout_stream) = child_stdout_handle {
        // Use the taken stdout
        let app_handle_for_stdout_mgr = app_handle_clone.clone();
        let app_handle_for_stdout_emit = app_handle_clone.clone();
        let current_pid_for_stdout_context = pid;
        let session_id_for_stdout_thread = session_id.clone();

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout_stream);
            let mut buffer = [0; 2048];
            let mut line_buffer = String::new();

            enum PwdMarkerParseState {
                Idle,
                AwaitingPwd(String),
                AwaitingEndMarker(String),
            }
            let mut pwd_marker_state = PwdMarkerParseState::Idle;

            let current_thread_id = std::thread::current().id();

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        if !line_buffer.is_empty() {
                            if let Err(e) = app_handle_for_stdout_emit
                                .emit("command_output", line_buffer.clone())
                            {
                                println!("[Rust STDOUT Thread {:?} PID {}] Error emitting final command_output: {}", current_thread_id, current_pid_for_stdout_context, e);
                            }
                        }
                        break;
                    }
                    Ok(n) => {
                        let output_chunk_str = String::from_utf8_lossy(&buffer[..n]).to_string();
                        line_buffer.push_str(&output_chunk_str);

                        while let Some(newline_pos) = line_buffer.find('\n') {
                            let line_segment =
                                line_buffer.drain(..=newline_pos).collect::<String>();
                            let current_line_trimmed = line_segment.trim().to_string();

                            if current_line_trimmed.is_empty() {
                                match pwd_marker_state {
                                    PwdMarkerParseState::Idle => {
                                        if let Err(e) = app_handle_for_stdout_emit
                                            .emit("command_output", line_segment.clone())
                                        {
                                            println!("[Rust STDOUT Thread {:?} PID {}] Error emitting whitespace/newline: {}", current_thread_id, current_pid_for_stdout_context, e);
                                        }
                                    }
                                    _ => {}
                                }
                                continue;
                            }

                            let mut emit_this_segment_to_frontend = true;

                            match pwd_marker_state {
                                PwdMarkerParseState::Idle => {
                                    if current_line_trimmed.starts_with("__REMOTE_CD_PWD_MARKER_")
                                        || current_line_trimmed
                                            .starts_with("__INITIAL_REMOTE_PWD_MARKER_")
                                    {
                                        pwd_marker_state = PwdMarkerParseState::AwaitingPwd(
                                            current_line_trimmed.clone(),
                                        );
                                        emit_this_segment_to_frontend = false;
                                    }
                                }
                                PwdMarkerParseState::AwaitingPwd(ref marker_val) => {
                                    let new_pwd = current_line_trimmed.clone();

                                    let command_manager_state =
                                        app_handle_for_stdout_mgr.state::<CommandManager>();
                                    if let Ok(mut states_guard) =
                                        command_manager_state.commands.lock()
                                    {
                                        if let Some(state) =
                                            states_guard.get_mut(&session_id_for_stdout_thread)
                                        {
                                            if state.pid == Some(current_pid_for_stdout_context)
                                                && state.is_ssh_session_active
                                            {
                                                state.remote_current_dir = Some(new_pwd.clone());
                                                if let Err(e) = app_handle_for_stdout_emit.emit(
                                                    "remote_directory_updated",
                                                    new_pwd.clone(),
                                                ) {
                                                    eprintln!("[Rust STDOUT Thread {:?} PID {}] Failed to emit remote_directory_updated: {}", current_thread_id, current_pid_for_stdout_context, e);
                                                }
                                            }
                                        }
                                    }
                                    pwd_marker_state =
                                        PwdMarkerParseState::AwaitingEndMarker(marker_val.clone());
                                    emit_this_segment_to_frontend = false;
                                }
                                PwdMarkerParseState::AwaitingEndMarker(ref marker_val) => {
                                    if current_line_trimmed == *marker_val {
                                        pwd_marker_state = PwdMarkerParseState::Idle;
                                        emit_this_segment_to_frontend = false;
                                    } else {
                                        pwd_marker_state = PwdMarkerParseState::Idle;
                                        if current_line_trimmed
                                            .starts_with("__REMOTE_CD_PWD_MARKER_")
                                            || current_line_trimmed
                                                .starts_with("__INITIAL_REMOTE_PWD_MARKER_")
                                        {
                                            pwd_marker_state = PwdMarkerParseState::AwaitingPwd(
                                                current_line_trimmed.clone(),
                                            );
                                            emit_this_segment_to_frontend = false;
                                        }
                                    }
                                }
                            }

                            if emit_this_segment_to_frontend {
                                if let Err(e) = app_handle_for_stdout_emit
                                    .emit("command_output", line_segment.clone())
                                {
                                    println!("[Rust STDOUT Thread {:?} PID {}] Error emitting command_output: {}", current_thread_id, current_pid_for_stdout_context, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        if !line_buffer.is_empty() {
                            if let Err(emit_e) = app_handle_for_stdout_emit
                                .emit("command_output", line_buffer.clone())
                            {
                                println!("[Rust STDOUT Thread {:?} PID {}] Error emitting final command_output on error: {}", current_thread_id, current_pid_for_stdout_context, emit_e);
                            }
                        }
                        break;
                    }
                }
            }
        });
    }

    if let Some(stderr_stream) = child_stderr_handle {
        // Use the taken stderr
        let app_handle_stderr = app_handle.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr_stream);
            let mut buffer = [0; 2048];
            let current_thread_id = std::thread::current().id(); // Get thread ID once
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        let error_chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                        if !error_chunk.contains("[sudo] password") {
                            if let Err(e) =
                                app_handle_stderr.emit("command_error", error_chunk.clone())
                            {
                                println!(
                                    "[Rust STDERR Thread {:?}] Error emitting command_error: {}",
                                    current_thread_id, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        break;
                    }
                }
            }
        });
    }

    // The wait thread now uses child_wait_handle_arc
    let app_handle_wait = app_handle_clone.clone();
    let app_handle_for_thread_state = app_handle.clone();
    let was_ssh_session_starter = is_potential_ssh_session_starter;
    let initial_child_pid_for_wait_thread = pid;

    thread::spawn(move || {
        let status_result = {
            // Lock the child_wait_handle_arc to wait on the child
            let mut child_guard = match child_wait_handle_arc.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    // Emit error and end messages
                    let _ = app_handle_wait.emit(
                        "command_error",
                        format!("Error locking child for wait: {}", e),
                    );
                    let _ = app_handle_wait
                        .emit("command_end", "Command failed due to wait lock error.");
                    return;
                }
            };
            // child_guard is MutexGuard<Child>
            child_guard.wait()
        };

        {
            // Cleanup block
            let command_manager_state_in_thread =
                app_handle_for_thread_state.state::<CommandManager>();
            let mut states_guard_cleanup = match command_manager_state_in_thread.commands.lock() {
                Ok(guard) => guard,
                Err(_e) => {
                    return;
                }
            };

            let key_cleanup = session_id_for_wait_thread.clone();
            if let Some(state_to_clear) = states_guard_cleanup.get_mut(&key_cleanup) {
                // Important: Only clear if the PID matches, to avoid race conditions
                // if another command started and this wait thread is for an older one.
                if state_to_clear.pid == Some(initial_child_pid_for_wait_thread) {
                    state_to_clear.child_wait_handle = None;
                    state_to_clear.pid = None; // PID is cleared here
                    if was_ssh_session_starter && state_to_clear.is_ssh_session_active {
                        state_to_clear.is_ssh_session_active = false;
                        state_to_clear.child_stdin = None; // Also clear stdin if it was an SSH session
                        state_to_clear.remote_current_dir = None; // Clear remote dir

                        let _ = app_handle_wait.emit("ssh_session_ended", serde_json::json!({ "pid": initial_child_pid_for_wait_thread, "reason": "SSH session ended normally."}));
                    } else if was_ssh_session_starter {
                        // SSH session starter but was already marked inactive (e.g. by write thread error)
                        // Ensure remote_current_dir is also cleared if it hasn't been.
                        state_to_clear.remote_current_dir = None;
                        state_to_clear.child_stdin = None;
                    }
                }
            }
        } // states_guard_cleanup lock released

        match status_result {
            Ok(status) => {
                let exit_msg = if status.success() {
                    "Command completed successfully."
                } else {
                    "Command failed."
                };
                let _ = app_handle_wait.emit("command_end", exit_msg);
            }
            Err(e) => {
                let _ = app_handle_wait
                    .emit("command_error", format!("Error waiting for command: {}", e));
                // Also emit command_end because the command effectively ended, albeit with an error during wait
                let _ = app_handle_wait.emit("command_end", "Command failed due to wait error.");
            }
        }
    });

    Ok("Command started. Output will stream in real-time.".to_string())
}

#[command]
pub fn execute_sudo_command(
    command: String,
    session_id: String,
    password: String,
    app_handle: AppHandle,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let mut states = command_manager.commands.lock().map_err(|e| e.to_string())?;

    let key = session_id;
    let state = states.entry(key.clone()).or_insert_with(|| CommandState {
        current_dir: env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        child_wait_handle: None,
        child_stdin: None,
        pid: None,
        is_ssh_session_active: false,
        remote_current_dir: None,
    });

    let current_dir = state.current_dir.clone();

    let mut child_process = match Command::new("sudo")
        .arg("-S")
        .arg("bash")
        .arg("-c")
        .arg(
            command
                .split_whitespace()
                .skip(1)
                .collect::<Vec<&str>>()
                .join(" "),
        ) // Skip "sudo" and join the rest
        .current_dir(&current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return Err(format!("Failed to start sudo command: {}", e));
        }
    };

    let child_pid = child_process.id(); // Get PID
    let sudo_stdin = child_process.stdin.take().map(|s| Arc::new(Mutex::new(s))); // Take stdin
    let sudo_stdout = child_process.stdout.take(); // Take stdout
    let sudo_stderr = child_process.stderr.take(); // Take stderr

    let child_arc = Arc::new(Mutex::new(child_process)); // Store the Child itself for waiting

    state.child_wait_handle = Some(child_arc.clone()); // Store wait handle
    state.pid = Some(child_pid); // Store PID
                                 // For sudo, is_ssh_session_active remains false, child_stdin for SSH is not set.

    // Send password to stdin
    if let Some(stdin_arc) = sudo_stdin {
        // Use the taken and Arc-wrapped stdin
        let app_handle_stdin = app_handle.clone();
        thread::spawn(move || {
            let mut stdin_guard = match stdin_arc.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    let _ = app_handle_stdin.emit("command_error", e.to_string());
                    return;
                }
            };
            if stdin_guard
                .write_all(format!("{}", password).as_bytes())
                .is_err()
            {
                let _ = app_handle_stdin.emit("command_error", "Failed to send password to sudo");
            }
        });
    }

    // Use the taken stdout_stream
    if let Some(stdout_stream) = sudo_stdout {
        let app_handle_stdout = app_handle.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout_stream);
            let mut buffer = [0; 2048]; // Read in chunks
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let output_chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                        let _ = app_handle_stdout.emit("command_output", output_chunk);
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        let _ = app_handle_stdout
                            .emit("command_output", format!("Error reading stdout: {}", e));
                        break;
                    }
                }
            }
        });
    }

    // Use the taken stderr_stream
    if let Some(stderr_stream) = sudo_stderr {
        let app_handle_stderr = app_handle.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr_stream);
            let mut buffer = [0; 2048]; // Read in chunks
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let error_chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                        if !error_chunk.contains("[sudo] password") {
                            let _ = app_handle_stderr.emit("command_error", error_chunk.clone());
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        let _ = app_handle_stderr
                            .emit("command_error", format!("Error reading stderr: {}", e));
                        break;
                    }
                }
            }
        });
    }

    let child_arc_clone = child_arc.clone();
    let app_handle_wait = app_handle.clone();
    thread::spawn(move || {
        let status = {
            let mut child_guard = child_arc_clone.lock().unwrap();
            match child_guard.wait() {
                Ok(status) => status,
                Err(e) => {
                    let _ = app_handle_wait
                        .emit("command_error", format!("Error waiting for command: {}", e));
                    return;
                }
            }
        };

        let _ = app_handle_wait.emit("command_end", format!("Success: {}", status.success()));
    });

    Ok("Command started. Output will stream in realtime.".to_string())
}

fn get_command_state<'a>(
    command_state_guard: &'a mut MutexGuard<HashMap<String, CommandState>>,
    session_id: String,
) -> &'a mut CommandState {
    command_state_guard
        .entry(session_id)
        .or_insert_with(|| CommandState {
            current_dir: env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            child_wait_handle: None,
            child_stdin: None,
            pid: None,
            is_ssh_session_active: false, // ensure default
            remote_current_dir: None,
        })
}
