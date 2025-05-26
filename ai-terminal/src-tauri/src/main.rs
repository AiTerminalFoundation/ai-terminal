// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate fix_path_env;

use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{command, AppHandle, Emitter, Manager, State};

// Define Ollama API models and structures
#[derive(Debug, Serialize, Deserialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaResponse {
    model: String,
    response: String,
    done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaModel {
    name: String,
    size: u64,
    modified_at: String,
    // Add other fields as needed
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaModelList {
    models: Vec<OllamaModel>,
}

// Store the current working directory for each command
struct CommandState {
    current_dir: String,
    child_wait_handle: Option<Arc<Mutex<Child>>>, // For wait() and kill()
    child_stdin: Option<Arc<Mutex<std::process::ChildStdin>>>, // For writing
    pid: Option<u32>,
    is_ssh_session_active: bool, // Added for persistent SSH
    remote_current_dir: Option<String>, // New field for remote SSH path
}

// Add Ollama state management
struct OllamaState {
    current_model: String,
    api_host: String,
}

// Structure to handle command output streaming
struct CommandManager {
    commands: Mutex<HashMap<String, CommandState>>,
    ollama: Mutex<OllamaState>,
}

impl CommandManager {
    fn new() -> Self {
        let mut initial_commands = HashMap::new();
        initial_commands.insert(
            "default_state".to_string(),
            CommandState {
                current_dir: env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
                child_wait_handle: None,
                child_stdin: None,
                pid: None,
                is_ssh_session_active: false, // Initialize here
                remote_current_dir: None, // Initialize new field
            },
        );
        CommandManager {
            commands: Mutex::new(initial_commands),
            ollama: Mutex::new(OllamaState {
                current_model: "llama3.2:latest".to_string(), // Default model will now be overridden by frontend
                api_host: "http://localhost:11434".to_string(), // Default Ollama host
            }),
        }
    }
}

fn get_shell_path() -> Option<String> {
    // First try to get the user's default shell
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        // Try to get the user's default shell from /etc/shells or fallback to common shells
        let shells = ["/bin/zsh", "/bin/bash", "/bin/sh"];
        for shell in shells.iter() {
            if std::path::Path::new(shell).exists() {
                return Some(shell.to_string());
            }
        }
        "sh" // Fallback
    };

    // Try to get PATH using the shell's login mode and sourcing initialization files
    let command = if shell.contains("zsh") {
        "source ~/.zshrc 2>/dev/null || true; source ~/.zshenv 2>/dev/null || true; echo $PATH"
    } else if shell.contains("bash") {
        "source ~/.bashrc 2>/dev/null || true; source ~/.bash_profile 2>/dev/null || true; echo $PATH"
    } else {
        "echo $PATH"
    };

    let output = Command::new(shell)
        .arg("-l") // Login shell to get proper environment
        .arg("-c")
        .arg(command)
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }

    // If shell method fails, try to get PATH from environment
    std::env::var("PATH").ok()
}

#[command]
fn execute_command(
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
        let key = session_id.clone();
        println!("[Rust EXEC DEBUG] Phase 1: Checking for active SSH session for key: {}", key);

        let state = states_guard.entry(key.clone()).or_insert_with(|| {
            println!("[Rust EXEC DEBUG] No existing state for key {}, creating new.", key);
            CommandState {
                current_dir: env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
                child_wait_handle: None,
                child_stdin: None,
                pid: None,
                is_ssh_session_active: false,
                remote_current_dir: None,
            }
        });

        if state.is_ssh_session_active {
            println!("[Rust EXEC DEBUG] Active SSH session detected (is_ssh_session_active=true).");
            if let Some(stdin_arc_for_thread) = state.child_stdin.clone() {
                let active_pid_for_log = state.pid.unwrap_or(0);
                println!("[Rust EXEC DEBUG] Found child_stdin for active SSH (Original PID: {}).", active_pid_for_log);

                if let Err(e) = app_handle.emit("command_forwarded_to_ssh", command.clone()) {
                    eprintln!("[Rust EXEC DEBUG] Failed to emit command_forwarded_to_ssh: {}", e);
                } else {
                    println!("[Rust EXEC DEBUG] Emitted command_forwarded_to_ssh for command: {}", command);
                }

                let app_handle_clone_for_thread = app_handle.clone();
                let command_clone_for_thread = command.clone();
                let session_id_clone_for_thread = session_id.clone();

                println!("[Rust EXEC DEBUG] Attempting to forward command '{}' to active SSH session (Original PID: {})", command_clone_for_thread, active_pid_for_log);

                thread::spawn(move || {
                    println!("[Rust EXEC DEBUG SSH-Write-Thread] Spawned for command: {}", command_clone_for_thread);
                    let command_manager_state_for_thread = app_handle_clone_for_thread.state::<CommandManager>();

                    let mut stdin_guard = match stdin_arc_for_thread.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            eprintln!("[Rust EXEC DEBUG SSH-Write-Thread] Failed to lock SSH ChildStdin: {}. Resetting SSH state.", e);
                            if let Ok(mut states_lock_in_thread) = command_manager_state_for_thread.commands.lock() {
                                if let Some(s) = states_lock_in_thread.get_mut(&session_id_clone_for_thread) {
                                    if s.pid == Some(active_pid_for_log) && s.is_ssh_session_active {
                                        println!("[Rust EXEC DEBUG SSH-Write-Thread] Resetting SSH active state (stdin, pid:{}) due to ChildStdin lock failure.", active_pid_for_log);
                                        s.is_ssh_session_active = false;
                                        s.child_stdin = None;
                                        s.remote_current_dir = None;
                                    }
                                }
                            }
                            let _ = app_handle_clone_for_thread.emit("ssh_session_ended", serde_json::json!({ "pid": active_pid_for_log, "reason": format!("SSH session error (stdin lock): {}", e)}));
                            let _ = app_handle_clone_for_thread.emit("command_error", format!("Failed to send to SSH (stdin lock '{}'): {}", command_clone_for_thread, e));
                            let _ = app_handle_clone_for_thread.emit("command_end", "Command failed.");
                            return;
                        }
                    };
                    println!("[Rust EXEC DEBUG SSH-Write-Thread] Successfully locked SSH ChildStdin.");

                    println!("[Rust EXEC DEBUG SSH-Write-Thread] Writing command to SSH ChildStdin: {}", command_clone_for_thread);
                    
                    let is_remote_cd = command_clone_for_thread.trim().starts_with("cd ");
                    let actual_command_to_write_ssh = if is_remote_cd {
                        let marker = format!("__REMOTE_CD_PWD_MARKER_{}__", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64().to_string().replace('.', ""));
                        let cd_command_part = command_clone_for_thread.trim();
                        format!("{} && printf '%s\\n' '{}' && pwd && printf '%s\\n' '{}'\n", cd_command_part, marker, marker)
                    } else {
                        format!("{}\n", command_clone_for_thread)
                    };
                    
                    let write_attempt = stdin_guard.write_all(actual_command_to_write_ssh.as_bytes());
                    
                    let final_result = if write_attempt.is_ok() {
                        println!("[Rust EXEC DEBUG SSH-Write-Thread] Write successful. Flushing ChildStdin.");
                        stdin_guard.flush()
                    } else {
                        eprintln!("[Rust EXEC DEBUG SSH-Write-Thread] Write failed: {:?}. Won't flush.", write_attempt.as_ref().err());
                        write_attempt 
                    };

                    if let Err(e) = final_result {
                        eprintln!("[Rust EXEC DEBUG SSH-Write-Thread] Failed to write/flush to SSH ChildStdin: {}. Resetting SSH state.", e);
                        if let Ok(mut states_lock_in_thread) = command_manager_state_for_thread.commands.lock() {
                             if let Some(s) = states_lock_in_thread.get_mut(&session_id_clone_for_thread) {
                                if s.pid == Some(active_pid_for_log) && s.is_ssh_session_active {
                                    println!("[Rust EXEC DEBUG SSH-Write-Thread] Resetting SSH active state (stdin, pid:{}) due to write/flush failure.", active_pid_for_log);
                                    s.is_ssh_session_active = false;
                                    s.child_stdin = None;
                                    s.remote_current_dir = None;
                                }
                            }
                        }
                        let _ = app_handle_clone_for_thread.emit("ssh_session_ended", serde_json::json!({ "pid": active_pid_for_log, "reason": format!("SSH session ended (stdin write/flush error): {}", e)}));
                        let _ = app_handle_clone_for_thread.emit("command_error", format!("Failed to send to SSH (stdin write/flush '{}'): {}", command_clone_for_thread, e));
                        let _ = app_handle_clone_for_thread.emit("command_end", "Command failed.");
                        return;
                    }
                    println!("[Rust EXEC DEBUG SSH-Write-Thread] Write and flush successful for command: {}", command_clone_for_thread);
                    println!("[Rust EXEC DEBUG SSH-Write-Thread] Exiting for command: {}", command_clone_for_thread);
                });

                drop(states_guard); 
                println!("[Rust EXEC DEBUG] Returning COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER for forwarded command (PID: {}).", active_pid_for_log);
                return Ok(COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER.to_string());

            } else { // state.child_stdin is None, but state.is_ssh_session_active was true
                let active_pid_for_log = state.pid.unwrap_or(0);
                eprintln!("[Rust EXEC DEBUG] SSH session active but no child_stdin found (PID: {}). Resetting state.", active_pid_for_log);
                state.is_ssh_session_active = false;
                state.pid = None; // Clear PID as session is now considered broken
                state.remote_current_dir = None;
                drop(states_guard); 
                let _ = app_handle.emit("ssh_session_ended", serde_json::json!({ "pid": active_pid_for_log, "reason": "SSH session inconsistency: active but no stdin."}));
                return Err("SSH session conflict: active but no stdin. Please retry.".to_string());
            }
        } else {
            println!("[Rust EXEC DEBUG] Phase 1: Finished SSH check.");
        }
    }

    // Phase 2: Handle 'cd' command (if not in an SSH session)
    // The `cd` command logic remains largely the same, it acquires its own lock.
    if command.starts_with("cd ") || command == "cd" {
        // This block is the original 'cd' handling logic.
        // It will lock `command_manager.commands` internally.
        let mut states_guard_cd = command_manager.commands.lock().map_err(|e| e.to_string())?;
        let key_cd = session_id.clone();
        let state_cd = states_guard_cd.entry(key_cd.clone()).or_insert_with(|| CommandState {
             current_dir: env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
             child_wait_handle: None,
             child_stdin: None,
             pid: None,
             is_ssh_session_active: false, // ensure default
             remote_current_dir: None,
        });

        let path = command.trim_start_matches("cd").trim();
        if path.is_empty() || path == "~" || path == "~/" {
            return if let Some(home_dir) = dirs::home_dir() {
                let home_path = home_dir.to_string_lossy().to_string();
                state_cd.current_dir = home_path.clone();
                drop(states_guard_cd); // Release lock before emitting and returning
                let _ = app_handle.emit("command_end", "Command completed successfully.");
                Ok(format!("Changed directory to {}", home_path))
            } else {
                drop(states_guard_cd);
                let _ = app_handle.emit("command_end", "Command failed.");
                Err("Could not determine home directory".to_string())
            };
        }
        let current_path = Path::new(&state_cd.current_dir);
        let new_path = if path.starts_with('~') {
            if let Some(home_dir) = dirs::home_dir() {
                let without_tilde = path.trim_start_matches('~');
                let rel_path = without_tilde.trim_start_matches('/');
                if rel_path.is_empty() { home_dir } else { home_dir.join(rel_path) }
            } else { drop(states_guard_cd); return Err("Could not determine home directory".to_string()); }
        } else if path.starts_with('/') {
            std::path::PathBuf::from(path)
        } else {
            let mut result_path = current_path.to_path_buf();
            let path_components: Vec<&str> = path.split('/').collect();
            for component in path_components {
                if component == ".." {
                    if let Some(parent) = result_path.parent() { result_path = parent.to_path_buf(); } 
                    else { drop(states_guard_cd); let _ = app_handle.emit("command_end", "Command failed."); return Err("Already at root directory".to_string()); }
                } else if component != "." && !component.is_empty() {
                    result_path = result_path.join(component);
                }
            }
            result_path
        };
        return if new_path.exists() {
            state_cd.current_dir = new_path.to_string_lossy().to_string();
            let current_dir_for_ok = state_cd.current_dir.clone();
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
        let key_dir = session_id.clone();
        let state_dir = states_guard_dir.entry(key_dir.clone()).or_insert_with(|| CommandState {
            current_dir: env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
            child_wait_handle: None,
            child_stdin: None,
            pid: None,
            is_ssh_session_active: false,
            remote_current_dir: None,
        });
        state_dir.current_dir.clone()
    }; // Lock for current_dir released.


    // Proactive SSH password handling (if not in an SSH session)
    let is_plain_ssh_attempt = command.contains("ssh ") && !command.trim_start().starts_with("sudo ssh ");
    if is_plain_ssh_attempt && ssh_password.is_none() {
        app_handle.emit(SSH_PRE_EXEC_PASSWORD_EVENT, command.clone()).map_err(|e| e.to_string())?;
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
    if is_potential_ssh_session_starter && !original_command_is_sudo_ssh { // Avoid mangling "sudo ssh ..." here
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
            let args_after_ssh_keyword_in_original = original_command_parts.iter().skip(idx_ssh + 1).cloned().collect::<Vec<&str>>().join(" ");

            if is_likely_interactive_ssh {
                // For interactive: ssh -options user@host
                command_to_run = format!("{} {}", ssh_options_prefix, args_after_ssh_keyword_in_original.trim_end());
            } else if first_non_option_idx_after_ssh.is_some() {
                // For non-interactive (ssh user@host remote_command): ssh -options user@host remote_command
                command_to_run = format!("{} {}", ssh_options_prefix, args_after_ssh_keyword_in_original);
            } else {
                // Could be just "ssh" or "ssh -options", keep as is but with prefix, though likely won't connect
                command_to_run = format!("{} {}", ssh_options_prefix, args_after_ssh_keyword_in_original);
            }
            println!("[Rust EXEC] Transformed SSH command for execution: [{}]", command_to_run);
        }
    }

    // Now, use the (potentially transformed) command_to_run for direct/sshpass spawning
    if is_potential_ssh_session_starter && !original_command_is_sudo { 
        println!("[Rust EXEC] Preparing to spawn SSH directly (potentially with sshpass). Original user command: [{}]", command);
        println!("    Internally prepared base ssh command (command_to_run): [{}]", command_to_run);
        println!("    Current dir: [{}]", current_dir_clone);

        let executable_name: String;
        let mut arguments: Vec<String> = Vec::new();

        if let Some(password_value) = ssh_password { 
            executable_name = "sshpass".to_string();
            arguments.push("-p".to_string());
            arguments.push(password_value); // password_value is a String, gets moved here
            // command_to_run is the full "ssh -t -t ..." string
            arguments.extend(command_to_run.split_whitespace().map(String::from));
            println!("    Using sshpass with provided password.");
        } else {
            // No password provided: use plain ssh
            // command_to_run is already "ssh -t -t ..."
            let parts: Vec<String> = command_to_run.split_whitespace().map(String::from).collect();
            if parts.is_empty() || parts[0] != "ssh" {
                return Err(format!("Failed to parse SSH command for direct execution: {}", command_to_run));
            }
            executable_name = parts[0].clone(); // Should be "ssh"
            arguments.extend(parts.iter().skip(1).cloned());
            println!("    Using plain ssh (no password provided to backend, will rely on key auth or agent).");
        }
        
        cmd_to_spawn = Command::new(&executable_name);
        for arg in &arguments {
            cmd_to_spawn.arg(arg);
        }
        
        // env_map is passed as is. If SSH_ASKPASS was in it from a broader environment, 
        // sshpass should take precedence or ssh (in key auth) would ignore it if not needed.
        cmd_to_spawn.current_dir(&current_dir_clone)
            .envs(&env_map) 
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        // setsid() was removed here in a previous step, which is good.
        
        child = match cmd_to_spawn.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to start direct command ({}): {}", executable_name, e)),
        };

    } else { // Fallback to sh -c for non-SSH or sudo commands
        let final_shell_command = if original_command_is_sudo && !original_command_is_sudo_ssh {
            command_to_run.clone() 
        } else {
            format!("exec {}", command_to_run)
        };
        
        println!("[Rust EXEC] Final shell command for sh -c: [{}]", final_shell_command);
        println!("    About to spawn for command: [{}] (Original: {})", command_to_run, command);
        println!("    Current dir: [{}]", current_dir_clone);
        println!("    Plain SSH attempt (via sh -c): {}", is_plain_ssh_attempt);
        println!("    Is potential SSH starter (via sh -c): {}", is_potential_ssh_session_starter);

        let mut sh_cmd_to_spawn = Command::new("sh");
        sh_cmd_to_spawn.arg("-c")
            .arg(&final_shell_command)
            .current_dir(&current_dir_clone) 
            .envs(&env_map)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped()); // Ensure stdin is piped for sh -c as well

        #[cfg(unix)]
        unsafe {
            sh_cmd_to_spawn.pre_exec(|| {
                match nix::unistd::setsid() {
                    Ok(_) => Ok(()),
                    Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, format!("setsid failed: {}", e))),
                }
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
    let child_stderr_handle = child.stderr.take();        let child_wait_handle_arc = Arc::new(Mutex::new(child)); // Now 'child' has no IO handles
        let session_id_for_wait_thread = session_id.clone();

    {
        let mut states_guard_update = command_manager.commands.lock().map_err(|e| e.to_string())?;
        let key_update = session_id.clone();
        let state_to_update = states_guard_update.entry(key_update).or_insert_with(|| CommandState {
            current_dir: current_dir_clone.clone(), 
            child_wait_handle: None,
            child_stdin: None,
            pid: None,
            is_ssh_session_active: false,
            remote_current_dir: None,
        });

        state_to_update.pid = Some(pid);
        state_to_update.child_wait_handle = Some(child_wait_handle_arc.clone()); // Store wait handle

        if is_potential_ssh_session_starter {
            state_to_update.child_stdin = child_stdin_handle; // Store stdin handle for SSH
            state_to_update.is_ssh_session_active = true;
            state_to_update.remote_current_dir = Some("remote:~".to_string()); // Initial placeholder
            println!("[Rust EXEC] SSH session (pid: {}) marked active.", pid);
            let _ = app_handle_clone.emit("ssh_session_started", serde_json::json!({ "pid": pid }));

            // Attempt to send initial PWD command
            if let Some(stdin_arc_for_init_pwd) = state_to_update.child_stdin.clone() {
                let app_handle_for_init_pwd_thread = app_handle_clone.clone(); // Clone app_handle for the thread
                let initial_pid_for_init_pwd_error = pid;
                let session_id_for_init_pwd_thread = session_id.clone();
        
                thread::spawn(move || {
                    // Get CommandManager state inside the thread using the moved app_handle
                    let command_manager_state_for_thread = app_handle_for_init_pwd_thread.state::<CommandManager>();

                    let initial_pwd_marker = format!("__INITIAL_REMOTE_PWD_MARKER_{}__", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64().to_string().replace('.', ""));
                    let initial_pwd_command = format!("echo '{}'; pwd; echo '{}'\n", initial_pwd_marker, initial_pwd_marker);
                    
                    println!("[Rust EXEC SSH-Init-PWD-Thread] Attempting to send initial PWD command for PID {}: {}", initial_pid_for_init_pwd_error, initial_pwd_command.trim());

                    match stdin_arc_for_init_pwd.lock() {
                        Ok(mut stdin_guard) => {
                            if let Err(e) = stdin_guard.write_all(initial_pwd_command.as_bytes()).and_then(|_| stdin_guard.flush()) {
                                eprintln!("[Rust EXEC SSH-Init-PWD-Thread] Failed to write/flush initial PWD command for PID {}: {}. Resetting SSH state if still active.", initial_pid_for_init_pwd_error, e);
                                if let Ok(mut states_lock) = command_manager_state_for_thread.commands.lock() { // Use state obtained within the thread
                                    if let Some(s) = states_lock.get_mut(&session_id_for_init_pwd_thread) {
                                        if s.pid == Some(initial_pid_for_init_pwd_error) && s.is_ssh_session_active {
                                            s.is_ssh_session_active = false;
                                            s.child_stdin = None;
                                            s.remote_current_dir = None;
                                            let _ = app_handle_for_init_pwd_thread.emit("ssh_session_ended", serde_json::json!({ "pid": initial_pid_for_init_pwd_error, "reason": format!("SSH session error (initial PWD send for pid {}): {}", initial_pid_for_init_pwd_error, e)}));
                                        }
                                    }
                                }
                            } else {
                                println!("[Rust EXEC SSH-Init-PWD-Thread] Successfully sent initial PWD command for PID {}.", initial_pid_for_init_pwd_error);
                            }
                        }
                        Err(e) => {
                             eprintln!("[Rust EXEC SSH-Init-PWD-Thread] Failed to lock SSH ChildStdin for initial PWD command (PID {}): {}. Resetting SSH state if still active.", initial_pid_for_init_pwd_error, e);
                                if let Ok(mut states_lock) = command_manager_state_for_thread.commands.lock() { // Use state obtained within the thread
                                    if let Some(s) = states_lock.get_mut(&session_id_for_init_pwd_thread) {
                                        if s.pid == Some(initial_pid_for_init_pwd_error) && s.is_ssh_session_active {
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
            } else {
                eprintln!("[Rust EXEC] New SSH session (pid: {}) started, but child_stdin was None. Cannot send initial PWD command.", pid);
            }
        } else {
            state_to_update.is_ssh_session_active = false;
            state_to_update.child_stdin = None; // Ensure stdin is None for non-SSH commands
            state_to_update.remote_current_dir = None; // Ensure remote_dir is None for non-SSH
        }
    } // states_guard_update lock released

    if let Some(stdout_stream) = child_stdout_handle { // Use the taken stdout
        let app_handle_for_stdout_mgr = app_handle_clone.clone(); 
        let app_handle_for_stdout_emit = app_handle_clone.clone(); 
        let current_pid_for_stdout_context = pid;
        let session_id_for_stdout_thread = session_id.clone();

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout_stream);
            let mut buffer = [0; 2048];
            let mut line_buffer = String::new(); 
            
            enum PwdMarkerParseState { Idle, AwaitingPwd(String), AwaitingEndMarker(String) }
            let mut pwd_marker_state = PwdMarkerParseState::Idle;

            let current_thread_id = std::thread::current().id(); 
            println!("[Rust STDOUT Thread {:?} PID {}] Started.", current_thread_id, current_pid_for_stdout_context);
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        println!("[Rust STDOUT Thread {:?} PID {}] EOF reached.", current_thread_id, current_pid_for_stdout_context); 
                        if !line_buffer.is_empty() { 
                            println!("[Rust STDOUT Thread {:?} PID {}] Emitting remaining line_buffer: '{}'", current_thread_id, current_pid_for_stdout_context, line_buffer);
                            if let Err(e) = app_handle_for_stdout_emit.emit("command_output", line_buffer.clone()) {
                                println!("[Rust STDOUT Thread {:?} PID {}] Error emitting final command_output: {}", current_thread_id, current_pid_for_stdout_context, e); 
                            }
                        }
                        break;
                    }
                    Ok(n) => {
                        let output_chunk_str = String::from_utf8_lossy(&buffer[..n]).to_string();
                        line_buffer.push_str(&output_chunk_str);

                        while let Some(newline_pos) = line_buffer.find('\n') {
                            let line_segment = line_buffer.drain(..=newline_pos).collect::<String>();
                            let current_line_trimmed = line_segment.trim().to_string();

                            if current_line_trimmed.is_empty() {
                                match pwd_marker_state {
                                    PwdMarkerParseState::Idle => {
                                        if let Err(e) = app_handle_for_stdout_emit.emit("command_output", line_segment.clone()) {
                                            println!("[Rust STDOUT Thread {:?} PID {}] Error emitting whitespace/newline: {}", current_thread_id, current_pid_for_stdout_context, e);
                                        }
                                    },
                                    _ => {} 
                                }
                                continue;
                            }

                            let mut emit_this_segment_to_frontend = true;

                            match pwd_marker_state {
                                PwdMarkerParseState::Idle => {
                                    if current_line_trimmed.starts_with("__REMOTE_CD_PWD_MARKER_") || current_line_trimmed.starts_with("__INITIAL_REMOTE_PWD_MARKER_") {
                                        println!("[Rust STDOUT Thread {:?} PID {}] PWD Start Marker detected: {}", current_thread_id, current_pid_for_stdout_context, current_line_trimmed);
                                        pwd_marker_state = PwdMarkerParseState::AwaitingPwd(current_line_trimmed.clone());
                                        emit_this_segment_to_frontend = false;
                                    }
                                }
                                PwdMarkerParseState::AwaitingPwd(ref marker_val) => {
                                    let new_pwd = current_line_trimmed.clone();
                                    println!("[Rust STDOUT Thread {:?} PID {}] Captured PWD: '{}' for marker: {}", current_thread_id, current_pid_for_stdout_context, new_pwd, marker_val);
                                    
                                    let command_manager_state = app_handle_for_stdout_mgr.state::<CommandManager>();
                                    if let Ok(mut states_guard) = command_manager_state.commands.lock() {
                                        if let Some(state) = states_guard.get_mut(&session_id_for_stdout_thread) {
                                            if state.pid == Some(current_pid_for_stdout_context) && state.is_ssh_session_active {
                                                state.remote_current_dir = Some(new_pwd.clone());
                                                println!("[Rust STDOUT Thread {:?} PID {}] Updated remote_current_dir to: {}", current_thread_id, current_pid_for_stdout_context, new_pwd);
                                                if let Err(e) = app_handle_for_stdout_emit.emit("remote_directory_updated", new_pwd.clone()) {
                                                    eprintln!("[Rust STDOUT Thread {:?} PID {}] Failed to emit remote_directory_updated: {}", current_thread_id, current_pid_for_stdout_context, e);
                                                }
                                            } else {
                                                println!("[Rust STDOUT Thread {:?} PID {}] SSH no longer active or PID mismatch for PWD update. State PID: {:?}, Active: {}", current_thread_id, current_pid_for_stdout_context, state.pid, state.is_ssh_session_active);
                                            }
                                        }
                                    }
                                    pwd_marker_state = PwdMarkerParseState::AwaitingEndMarker(marker_val.clone());
                                    emit_this_segment_to_frontend = false;
                                }
                                PwdMarkerParseState::AwaitingEndMarker(ref marker_val) => {
                                    if current_line_trimmed == *marker_val {
                                        println!("[Rust STDOUT Thread {:?} PID {}] PWD End Marker detected: {}", current_thread_id, current_pid_for_stdout_context, current_line_trimmed);
                                        pwd_marker_state = PwdMarkerParseState::Idle;
                                        emit_this_segment_to_frontend = false;
                                    } else {
                                        println!("[Rust STDOUT Thread {:?} PID {}] WARNING: Expected PWD end marker '{}', got: '{}'. Resetting state and emitting line.", current_thread_id, current_pid_for_stdout_context, marker_val, current_line_trimmed);
                                        pwd_marker_state = PwdMarkerParseState::Idle;
                                        if current_line_trimmed.starts_with("__REMOTE_CD_PWD_MARKER_") || current_line_trimmed.starts_with("__INITIAL_REMOTE_PWD_MARKER_") {
                                            println!("[Rust STDOUT Thread {:?} PID {}] PWD Start Marker detected immediately after unexpected line: {}", current_thread_id, current_pid_for_stdout_context, current_line_trimmed);
                                            pwd_marker_state = PwdMarkerParseState::AwaitingPwd(current_line_trimmed.clone());
                                            emit_this_segment_to_frontend = false;
                                        }
                                    }
                                }
                            }

                            if emit_this_segment_to_frontend {
                                if let Err(e) = app_handle_for_stdout_emit.emit("command_output", line_segment.clone()) { 
                                    println!("[Rust STDOUT Thread {:?} PID {}] Error emitting command_output: {}", current_thread_id, current_pid_for_stdout_context, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("[Rust STDOUT Thread {:?} PID {}] Error reading stdout: {}", current_thread_id, current_pid_for_stdout_context, e); 
                        if e.kind() == std::io::ErrorKind::Interrupted { continue; }
                        if !line_buffer.is_empty() {
                             println!("[Rust STDOUT Thread {:?} PID {}] Emitting remaining line_buffer on error: '{}'", current_thread_id, current_pid_for_stdout_context, line_buffer);
                             if let Err(emit_e) = app_handle_for_stdout_emit.emit("command_output", line_buffer.clone()) {
                                 println!("[Rust STDOUT Thread {:?} PID {}] Error emitting final command_output on error: {}", current_thread_id, current_pid_for_stdout_context, emit_e);
                             }
                        }
                        break;
                    }
                }
            }
            println!("[Rust STDOUT Thread {:?} PID {}] Exiting.", current_thread_id, current_pid_for_stdout_context); 
        });
    }

    if let Some(stderr_stream) = child_stderr_handle { // Use the taken stderr
        let app_handle_stderr = app_handle.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr_stream);
            let mut buffer = [0; 2048];
            let current_thread_id = std::thread::current().id(); // Get thread ID once
            println!("[Rust STDERR Thread {:?}] Started for command.", current_thread_id); // LOG thread start
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        println!("[Rust STDERR Thread {:?}] EOF reached.", current_thread_id); // LOG
                        break;
                    }
                    Ok(n) => {
                        let error_chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                        println!("[Rust STDERR Thread {:?}] Read chunk: '{}'", current_thread_id, error_chunk); // LOG
                        if !error_chunk.contains("[sudo] password") {
                             if let Err(e) = app_handle_stderr.emit("command_error", error_chunk.clone()) { // LOG event emission
                                println!("[Rust STDERR Thread {:?}] Error emitting command_error: {}", current_thread_id, e); // LOG
                             }
                        }
                    }
                    Err(e) => {
                        println!("[Rust STDERR Thread {:?}] Error reading stderr: {}", current_thread_id, e); // LOG
                        if e.kind() == std::io::ErrorKind::Interrupted { continue; }
                        break;
                    }
                }
            }
            println!("[Rust STDERR Thread {:?}] Exiting.", current_thread_id); // LOG
        });
    }

    // The wait thread now uses child_wait_handle_arc
    let app_handle_wait = app_handle_clone.clone();
    let app_handle_for_thread_state = app_handle.clone(); 
    let was_ssh_session_starter = is_potential_ssh_session_starter;
    let initial_child_pid_for_wait_thread = pid; 

    thread::spawn(move || {
        println!("[Rust WAIT Thread] Started for PID: {}", initial_child_pid_for_wait_thread);
        
        let status_result = { 
            // Lock the child_wait_handle_arc to wait on the child
            let mut child_guard = match child_wait_handle_arc.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("[Rust WAIT Thread] Failed to lock child_wait_handle for PID {}: {}", initial_child_pid_for_wait_thread, e);
                    // Emit error and end messages
                    let _ = app_handle_wait.emit("command_error", format!("Error locking child for wait: {}", e));
                    let _ = app_handle_wait.emit("command_end", "Command failed due to wait lock error.");
                    return;
                }
            };
            // child_guard is MutexGuard<Child>
            child_guard.wait()
        };

        { // Cleanup block
            let command_manager_state_in_thread = app_handle_for_thread_state.state::<CommandManager>();
            let mut states_guard_cleanup = match command_manager_state_in_thread.commands.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("[Rust WAIT Thread] Error locking command_manager in wait thread for PID {}: {}", initial_child_pid_for_wait_thread, e);
                    // Cannot panic here, just log and proceed if possible or return
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
                        println!("SSH session (pid: {}) ended by wait thread. Marked inactive.", initial_child_pid_for_wait_thread);
                        let _ = app_handle_wait.emit("ssh_session_ended", serde_json::json!({ "pid": initial_child_pid_for_wait_thread, "reason": "SSH session ended normally."}));
                    } else if was_ssh_session_starter {
                        // SSH session starter but was already marked inactive (e.g. by write thread error)
                        // Ensure remote_current_dir is also cleared if it hasn't been.
                        state_to_clear.remote_current_dir = None;
                        println!("Wait thread: SSH session (pid: {}) was already inactive. Clearing handles.", initial_child_pid_for_wait_thread);
                         state_to_clear.child_stdin = None; 
                    }
                } else {
                     println!("[Rust WAIT Thread] PID mismatch during cleanup. Current state.pid: {:?}, waited_pid: {}. No cleanup performed by this thread.", state_to_clear.pid, initial_child_pid_for_wait_thread);
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
            },
            Err(e) => {
                let _ = app_handle_wait.emit("command_error", format!("Error waiting for command: {}", e));
                 // Also emit command_end because the command effectively ended, albeit with an error during wait
                let _ = app_handle_wait.emit("command_end", "Command failed due to wait error.");
            }
        }
    });

    Ok("Command started. Output will stream in real-time.".to_string())
}

#[command]
fn execute_sudo_command(
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
    if let Some(stdin_arc) = sudo_stdin { // Use the taken and Arc-wrapped stdin
        let app_handle_stdin = app_handle.clone();
        thread::spawn(move || {
            let mut stdin_guard = match stdin_arc.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to lock sudo stdin: {}", e);
                    let _ = app_handle_stdin.emit("command_error", "Failed to lock sudo stdin");
                    return;
                }
            };
            if stdin_guard
                .write_all(format!("{}
", password).as_bytes())
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
                        if e.kind() == std::io::ErrorKind::Interrupted { continue; }
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
                        if e.kind() == std::io::ErrorKind::Interrupted { continue; }
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

        let exit_msg = if status.success() {
            "Command completed successfully."
        } else {
            "Command failed."
        };
        let _ = app_handle_wait.emit("command_end", exit_msg);
    });

    Ok("Sudo command started. Output will stream in real-time.".to_string())
}

#[command]
fn autocomplete(
    input: String,
    session_id: String,
    command_manager: State<'_, CommandManager>,
) -> Result<Vec<String>, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    let current_dir = if let Some(state) = states.get(&key) {
        &state.current_dir
    } else {
        return Err("Could not determine current directory".to_string());
    };

    let input_parts: Vec<&str> = input.split_whitespace().collect();

    // Autocomplete commands if it's the first word
    if input_parts.len() <= 1 {
        // Common shell commands to suggest
        let common_commands = vec![
            "cd", "ls", "pwd", "mkdir", "touch", "cat", "echo", "grep", "find", "cp", "mv", "rm",
            "tar", "gzip", "ssh", "curl", "wget", "history", "exit", "clear", "top", "ps", "kill",
            "ping",
        ];

        // Filter commands that match input prefix
        let input_prefix = input_parts.first().unwrap_or(&"");

        // Case-insensitive filtering for commands
        let matches: Vec<String> = common_commands
            .iter()
            .filter(|&cmd| cmd.to_lowercase().starts_with(&input_prefix.to_lowercase()))
            .map(|&cmd| cmd.to_string())
            .collect();

        if !matches.is_empty() {
            return Ok(matches);
        }
    }

    // If we have a cd command, autocomplete directories
    let path_to_complete = if input_parts.first() == Some(&"cd") {
        if input_parts.len() > 1 {
            // Handle cd command with argument
            input_parts.last().unwrap_or(&"")
        } else {
            // Handle cd with no argument - show all directories in current folder
            ""
        }
    } else if !input_parts.is_empty() && input_parts[0].contains('/') {
        // Handle path directly
        input_parts[0]
    } else if input_parts.len() > 1 {
        // Handle second argument as path for any command
        input_parts.last().unwrap_or(&"")
    } else {
        // Default to empty string if no path found
        ""
    };

    // If input starts with cd, or we have a potential path to complete
    if input_parts.first() == Some(&"cd") || !path_to_complete.is_empty() {
        let (dir_to_search, prefix) = split_path_prefix(path_to_complete);

        // Create a Path for the directory to search
        let search_path = if dir_to_search.starts_with('/') || dir_to_search.starts_with('~') {
            if dir_to_search.starts_with('~') {
                let home = dirs::home_dir().ok_or("Could not determine home directory")?;
                let without_tilde = dir_to_search.trim_start_matches('~');
                let rel_path = without_tilde.trim_start_matches('/');
                if rel_path.is_empty() {
                    home
                } else {
                    home.join(rel_path)
                }
            } else {
                PathBuf::from(dir_to_search)
            }
        } else {
            Path::new(current_dir).join(dir_to_search)
        };

        if search_path.exists() && search_path.is_dir() {
            let entries = fs::read_dir(search_path).map_err(|e| e.to_string())?;

            let mut matches = Vec::new();
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Include all entries for empty prefix, otherwise filter by prefix (case-insensitive)
                if prefix.is_empty()
                    || file_name_str
                        .to_lowercase()
                        .starts_with(&prefix.to_lowercase())
                {
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

                    // For 'cd' command, only show directories
                    if input_parts.first() == Some(&"cd") && !is_dir {
                        continue;
                    }

                    // Add trailing slash for directories
                    let suggestion = if is_dir {
                        format!("{}/", file_name_str)
                    } else {
                        file_name_str.to_string()
                    };

                    // Construct the full path suggestion for the command
                    let base_path = if dir_to_search.is_empty() {
                        "".to_string()
                    } else {
                        format!("{}/", dir_to_search.trim_end_matches('/'))
                    };

                    matches.push(format!("{}{}", base_path, suggestion));
                }
            }

            if !matches.is_empty() {
                // Sort matches alphabetically, case-insensitive
                matches.sort_by_key(|a| a.to_lowercase());
                return Ok(matches);
            }
        }
    }

    Ok(Vec::new())
}

// Helper function to split a path into directory and file prefix parts
fn split_path_prefix(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(index) => {
            let (dir, file) = path.split_at(index + 1);
            (dir, file)
        }
        None => ("", path),
    }
}

#[command]
fn get_working_directory(session_id: String, command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    if let Some(state) = states.get(&key) {
        if state.is_ssh_session_active {
            // Return the stored remote CWD, or a default if not yet known
            Ok(state.remote_current_dir.clone().unwrap_or_else(|| "remote:~".to_string()))
        } else {
            Ok(state.current_dir.clone())
        }
    } else {
        // Fallback if session doesn't exist - create new default state
        Ok(env::current_dir().unwrap_or_default().to_string_lossy().to_string())
    }
}

#[command]
fn get_home_directory() -> Result<String, String> {
    dirs::home_dir()
        .map(|path| path.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

// Add a helper function to get the OS information
fn get_operating_system() -> String {
    #[cfg(target_os = "windows")]
    return "Windows".to_string();

    #[cfg(target_os = "macos")]
    return "macOS".to_string();

    #[cfg(target_os = "linux")]
    return "Linux".to_string();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return "Unknown".to_string();
}

// Implement the ask_ai function for Ollama integration
#[command]
async fn ask_ai(
    question: String,
    model_override: Option<String>,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    // Check if this is a special command
    if question.starts_with('/') {
        return handle_special_command(question, command_manager).await;
    }

    // Regular message to Ollama
    let model;
    let api_host;

    // Scope the mutex lock to drop it before any async operations
    {
        let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
        // Use the model_override if provided, otherwise use the default
        model = model_override.unwrap_or_else(|| ollama_state.current_model.clone());
        api_host = ollama_state.api_host.clone();
        // MutexGuard is dropped here at the end of scope
    }

    // Get the current operating system
    let os = get_operating_system();

    // Create a system prompt that includes OS information and formatting instructions
    let system_prompt = format!(
        "You are a helpful terminal assistant. The user is using a {} operating system. \
        When providing terminal commands, ensure they are compatible with {}. \
        When asked for a command, respond with ONLY the command in this format: ```command```\
        The command should be a single line without any explanation or additional text.",
        os, os
    );

    // Combine the system prompt with the user's question
    let combined_prompt = format!("{}\n\nUser: {}", system_prompt, question);

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/generate", api_host))
        .json(&OllamaRequest {
            model,
            prompt: combined_prompt,
            stream: false,
        })
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Ollama API: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Ollama API error: {}", res.status()));
    }

    let response: OllamaResponse = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    Ok(response.response)
}

// Handle special commands like /help, /models, /model
async fn handle_special_command(
    command: String,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    match command.as_str() {
        "/help" => Ok("Available commands:\n\
                /help - Show this help message\n\
                /models - List available models\n\
                /model [name] - Show current model or switch to a different model\n\
                /host [url] - Show current API host or set a new one"
            .to_string()),
        "/models" => {
            // Get list of available models from Ollama API
            let api_host;

            // Scope the mutex lock to drop it before any async operations
            {
                let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
                api_host = ollama_state.api_host.clone();
                // MutexGuard is dropped here
            }

            let client = reqwest::Client::new();
            let res = client
                .get(format!("{}/api/tags", api_host))
                .send()
                .await
                .map_err(|e| format!("Failed to get models from Ollama API: {}", e))?;

            if !res.status().is_success() {
                return Err(format!("Ollama API error: {}", res.status()));
            }

            let models: OllamaModelList = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse models list: {}", e))?;

            let mut result = String::from("Available models:\n");
            for model in models.models {
                result.push_str(&format!("- {} ({} bytes)\n", model.name, model.size));
            }
            Ok(result)
        }
        cmd if cmd.starts_with("/model") => {
            let parts: Vec<&str> = cmd.split_whitespace().collect();

            // Handle showing current model
            if parts.len() == 1 {
                let current_model;
                {
                    let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
                    current_model = ollama_state.current_model.clone();
                }
                Ok(format!("Current model: {}", current_model))
            }
            // Handle switching model
            else if parts.len() >= 2 {
                let new_model = parts[1].to_string();
                {
                    let mut ollama_state =
                        command_manager.ollama.lock().map_err(|e| e.to_string())?;
                    ollama_state.current_model = new_model.clone();
                }
                Ok(format!("Switched to model: {}", new_model))
            } else {
                Err("Invalid model command. Use /model [name] to switch models.".to_string())
            }
        }
        cmd if cmd.starts_with("/host") => {
            let parts: Vec<&str> = cmd.split_whitespace().collect();

            // Handle showing current host
            if parts.len() == 1 {
                let current_host;
                {
                    let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
                    current_host = ollama_state.api_host.clone();
                }
                Ok(format!("Current Ollama API host: {}", current_host))
            }
            // Handle changing host
            else if parts.len() >= 2 {
                let new_host = parts[1].to_string();
                {
                    let mut ollama_state =
                        command_manager.ollama.lock().map_err(|e| e.to_string())?;
                    ollama_state.api_host = new_host.clone();
                }
                Ok(format!("Changed Ollama API host to: {}", new_host))
            } else {
                Err("Invalid host command. Use /host [url] to change the API host.".to_string())
            }
        }
        _ => Err(format!(
            "Unknown command: {}. Type /help for available commands.",
            command
        )),
    }
}

// Add function to get models from Ollama API
#[command]
async fn get_models(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    // Get the API host from the Ollama state
    let api_host;
    {
        let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
        api_host = ollama_state.api_host.clone();
    }

    // Request the list of models from Ollama
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/tags", api_host))
        .send()
        .await
        .map_err(|e| format!("Failed to get models from Ollama API: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Ollama API error: {}", res.status()));
    }

    // Parse the response
    let models: OllamaModelList = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse models list: {}", e))?;

    // Format the response
    let mut result = String::from("Available models:\n");
    for model in models.models {
        result.push_str(&format!("- {} ({} bytes)\n", model.name, model.size));
    }
    Ok(result)
}

// Add function to switch model
#[command]
fn switch_model(
    model: String,
    command_manager: State<'_, CommandManager>,
) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.current_model = model.clone();
    Ok(format!("Switched to model: {}", model))
}

// Add function to get current API host
#[command]
fn get_host(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    Ok(format!(
        "Current Ollama API host: {}",
        ollama_state.api_host
    ))
}

// Add function to set API host
#[command]
fn set_host(host: String, command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.api_host = host.clone();
    Ok(format!("Changed Ollama API host to: {}", host))
}

#[command]
fn get_git_branch(session_id: String, command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    let current_dir = if let Some(state) = states.get(&key) {
        &state.current_dir
    } else {
        return Ok("".to_string());
    };

    // Check if .git directory exists
    let git_dir = Path::new(current_dir).join(".git");
    if !git_dir.exists() {
        return Ok("".to_string());
    }

    // Get current branch
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(current_dir)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(branch)
    } else {
        Ok("".to_string())
    }
}

#[tauri::command]
fn get_current_pid(session_id: String, command_manager: State<'_, CommandManager>) -> Result<u32, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    if let Some(state) = states.get(&key) {
        Ok(state.pid.unwrap_or(0))
    } else {
        Ok(0)
    }
}

#[tauri::command]
fn terminate_command(session_id: String, command_manager: State<'_, CommandManager>) -> Result<(), String> {
    let mut states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = session_id;

    let pid = if let Some(state) = states.get(&key) {
        state.pid.unwrap_or(0)
    } else {
        return Err("No active process found".to_string());
    };

    if pid == 0 {
        return Err("No active process to terminate".to_string());
    }

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        // Try to send SIGTERM first
        if let Err(err) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
            return Err(format!("Failed to send SIGTERM: {}", err));
        }

        // Give the process a moment to terminate gracefully
        std::thread::sleep(std::time::Duration::from_millis(100));

        // If it's still running, force kill with SIGKILL
        if let Err(err) = kill(Pid::from_raw(pid as i32), Signal::SIGKILL) {
            return Err(format!("Failed to send SIGKILL: {}", err));
        }
    }

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid);
            if handle.is_invalid() {
                return Err("Failed to open process".to_string());
            }

            if !TerminateProcess(handle, 0).as_bool() {
                CloseHandle(handle);
                return Err("Failed to terminate process".to_string());
            }

            CloseHandle(handle);
        }
    }

    // Clear the PID after successful termination
    if let Some(state) = states.get_mut(&key) {
        state.pid = None;
    }

    Ok(())
}

// Add a new command to get all system environment variables
#[tauri::command]
fn get_system_env() -> Result<Vec<(String, String)>, String> {
    let env_vars: Vec<(String, String)> = std::env::vars().collect();
    Ok(env_vars)
}

fn main() {
    let _ = fix_path_env::fix();
    // Create a new command manager
    let command_manager = CommandManager::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| {
            // Add any setup logic here
            Ok(())
        })
        .manage(command_manager)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            execute_command,
            execute_sudo_command,
            terminate_command,
            get_current_pid,
            autocomplete,
            get_working_directory,
            get_home_directory,
            ask_ai,
            get_models,
            switch_model,
            get_host,
            set_host,
            get_git_branch,
            get_system_env,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
