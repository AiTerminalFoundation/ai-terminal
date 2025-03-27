// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::{Command, Stdio, Child};
use std::sync::{Mutex, Arc};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::thread;
use tauri::{command, State, AppHandle, Emitter};
use std::env;
use dirs;
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};

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
    current_process: Option<Arc<Mutex<Child>>>,
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
        CommandManager {
            commands: Mutex::new(HashMap::new()),
            ollama: Mutex::new(OllamaState {
                current_model: "llama3.2:latest".to_string(), // Default model will now be overridden by frontend
                api_host: "http://localhost:11434".to_string(), // Default Ollama host
            }),
        }
    }
}

#[command]
fn execute_command(
    command: String, 
    app_handle: AppHandle,
    command_manager: State<'_, CommandManager>
) -> Result<String, String> {
    let mut states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    
    // Create a key that doesn't include the command itself to maintain state across commands
    let key = "default_state".to_string();
    
    let state = states.entry(key.clone()).or_insert_with(|| CommandState {
        current_dir: std::env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
        current_process: None,
    });

    // Handle cd command specially
    if command.starts_with("cd ") || command == "cd" {
        let path = command.trim_start_matches("cd").trim();
        
        // Handle empty cd or cd ~ to go to home directory
        if path.is_empty() || path == "~" {
            if let Some(home_dir) = dirs::home_dir() {
                let home_path = home_dir.to_string_lossy().to_string();
                state.current_dir = home_path.clone();
                
                // Emit command_end event to mark the command as complete
                let _ = app_handle.emit("command_end", "Command completed successfully.");
                
                return Ok(format!("Changed directory to {}", home_path));
            } else {
                // Emit command_end event even for errors
                let _ = app_handle.emit("command_end", "Command failed.");
                return Err("Could not determine home directory".to_string());
            }
        }
        
        // Create a path object for proper path resolution
        let current_path = std::path::Path::new(&state.current_dir);
        let new_path = if path.starts_with('/') {
            std::path::PathBuf::from(path)
        } else {
            // For parent directory navigation (..) and relative paths
            let mut result_path = current_path.to_path_buf();
            
            // Handle paths like ../../.. by resolving each component
            let path_components: Vec<&str> = path.split('/').collect();
            for component in path_components {
                if component == ".." {
                    // Go up one directory
                    if let Some(parent) = result_path.parent() {
                        result_path = parent.to_path_buf();
                    } else {
                        // Emit command_end event for errors
                        let _ = app_handle.emit("command_end", "Command failed.");
                        return Err("Already at root directory".to_string());
                    }
                } else if component != "." && !component.is_empty() {
                    // Add subdirectory (skip . and empty components)
                    result_path = result_path.join(component);
                }
            }
            
            result_path
        };
        
        if new_path.exists() {
            // Update current directory
            state.current_dir = new_path.to_string_lossy().to_string();
            
            // Emit command_end event immediately to mark the command as complete
            let _ = app_handle.emit("command_end", "Command completed successfully.");
            
            return Ok(format!("Changed directory to {}", state.current_dir));
        } else {
            // Emit command_end event for errors immediately
            let _ = app_handle.emit("command_end", "Command failed.");
            return Err(format!("Directory not found: {}", path));
        }
    }

    // Handle long-running commands with real-time output
    let is_long_running = command.starts_with("cargo") || 
                          command.starts_with("ping") || 
                          command.starts_with("npm") ||
                          command.starts_with("node") ||
                          command.starts_with("python") ||
                          command.starts_with("java") ||
                          command.contains("watch") ||
                          command.contains("--progress") ||
                          command.contains("-v") ||
                          command.contains("tail") ||
                          command.contains("top") ||
                          command.contains("sleep");

    // For fast commands, execute synchronously
    if !is_long_running {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&state.current_dir)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            // Emit command_end event for successful commands
            let _ = app_handle.emit("command_end", "Command completed successfully.");
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        } else {
            // Emit command_end event for failed commands
            let _ = app_handle.emit("command_end", "Command failed.");
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    }
    
    // For long-running commands, stream output in real-time
    let current_dir = state.current_dir.clone();
    let command_clone = command.clone();
    let app_handle_clone = app_handle.clone();
    
    // Return immediately with initial message
    let mut child = match Command::new("sh")
        .arg("-c")
        // On macOS, prefix the command with exec to ensure signals propagate correctly
        .arg(format!("exec {}", &command_clone))
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn() {
            Ok(child) => child,
            Err(e) => {
                return Err(format!("Failed to start command: {}", e));
            }
        };
            
    // Store the child process to allow for termination
    let child_arc = Arc::new(Mutex::new(child));
    state.current_process = Some(child_arc.clone());
    
    // Create separate thread to read stdout
    if let Some(stdout) = child_arc.lock().unwrap().stdout.take() {
        let app_handle_stdout = app_handle_clone.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let _ = app_handle_stdout.emit("command_output", line);
                    }
                    Err(e) => {
                        let _ = app_handle_stdout.emit("command_output", format!("Error reading output: {}", e));
                        break;
                    }
                }
            }
        });
    }
    
    // Create separate thread to read stderr
    if let Some(stderr) = child_arc.lock().unwrap().stderr.take() {
        let app_handle_stderr = app_handle_clone.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let _ = app_handle_stderr.emit("command_error", line);
                    }
                    Err(e) => {
                        let _ = app_handle_stderr.emit("command_error", format!("Error reading stderr: {}", e));
                        break;
                    }
                }
            }
        });
    }
    
    // Create a thread to wait for the process to complete
    let child_arc_clone = child_arc.clone();
    let app_handle_wait = app_handle_clone.clone();
    thread::spawn(move || {
        let status = {
            let mut child_guard = child_arc_clone.lock().unwrap();
            match child_guard.wait() {
                Ok(status) => status,
                Err(e) => {
                    let _ = app_handle_wait.emit("command_error", format!("Error waiting for command: {}", e));
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
    
    Ok("Command started. Output will stream in real-time.".to_string())
}

#[command]
fn terminate_command(
    command_manager: State<'_, CommandManager>
) -> Result<String, String> {
    let mut states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = "default_state".to_string();
    
    if let Some(state) = states.get_mut(&key) {
        if let Some(process) = &state.current_process {
            // Try to get the process ID
            let pid = match process.lock() {
                Ok(child) => child.id(),
                Err(_) => 0, // Invalid PID
            };
            
            // Terminate all child processes directly from the shell
            if pid > 0 {
                #[cfg(unix)]
                {
                    // MacOS-specific approach - use process groups
                    // First, try kill with SIGINT (equivalent to Ctrl+C)
                    let _ = Command::new("sh")
                        .arg("-c")
                        .arg(format!("kill -INT -{}", pid))
                        .status();
                    
                    // Give a brief moment for graceful termination
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    
                    // Then kill with SIGTERM (more forceful termination)
                    let _ = Command::new("sh")
                        .arg("-c")
                        .arg(format!("kill -TERM -{}", pid))
                        .status();
                        
                    // Brief pause
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    
                    // And finally, if still alive, use SIGKILL (force kill)
                    let _ = Command::new("sh")
                        .arg("-c")
                        .arg(format!("kill -KILL -{}", pid))
                        .status();
                        
                    // Also try direct process kill as fallback
                    let _ = Command::new("sh")
                        .arg("-c")
                        .arg(format!("kill -KILL {}", pid))
                        .status();
                }
                
                #[cfg(windows)]
                {
                    let _ = Command::new("taskkill")
                        .arg("/F") // Force kill
                        .arg("/T") // Kill child processes too
                        .arg(format!("/PID {}", pid))
                        .status();
                }
            }
            
            // Clear our reference to the process
            state.current_process = None;
        }
    }
    
    // Return success regardless to unblock the UI
    Ok("Command terminated".to_string())
}

#[command]
fn autocomplete(
    input: String,
    command_manager: State<'_, CommandManager>
) -> Result<Vec<String>, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    let key = "default_state".to_string();
    
    let current_dir = if let Some(state) = states.get(&key) {
        &state.current_dir
    } else {
        return Err("Could not determine current directory".to_string());
    };

    let input_parts: Vec<&str> = input.trim().split_whitespace().collect();
    
    // Autocomplete commands if it's the first word
    if input_parts.len() <= 1 {
        // Common shell commands to suggest
        let common_commands = vec![
            "cd", "ls", "pwd", "mkdir", "touch", "cat", "echo", "grep",
            "find", "cp", "mv", "rm", "tar", "gzip", "ssh", "curl", "wget",
            "history", "exit", "clear", "top", "ps", "kill", "ping"
        ];
        
        // Filter commands that match input prefix
        let input_prefix = input_parts.get(0).unwrap_or(&"");
        
        // Case-insensitive filtering for commands
        let matches: Vec<String> = common_commands.iter()
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
    } else if input_parts.len() > 0 && input_parts[0].contains('/') {
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
        let (dir_to_search, prefix) = split_path_prefix(&path_to_complete);
        
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
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_name = entry.file_name();
                    let file_name_str = file_name.to_string_lossy();
                    
                    // Include all entries for empty prefix, otherwise filter by prefix (case-insensitive)
                    if prefix.is_empty() || file_name_str.to_lowercase().starts_with(&prefix.to_lowercase()) {
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
            }
            
            if !matches.is_empty() {
                // Sort matches alphabetically, case-insensitive
                matches.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
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
fn get_working_directory(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let states = command_manager.commands.lock().map_err(|e| e.to_string())?;
    // Get the current directory from the default state
    let key = "default_state".to_string();
    
    let dir = if let Some(state) = states.get(&key) {
        state.current_dir.clone()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };
    
    Ok(dir)
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
    command_manager: State<'_, CommandManager>
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
    command_manager: State<'_, CommandManager>
) -> Result<String, String> {
    match command.as_str() {
        "/help" => {
            Ok("Available commands:\n\
                /help - Show this help message\n\
                /models - List available models\n\
                /model [name] - Show current model or switch to a different model\n\
                /host [url] - Show current API host or set a new one".to_string())
        }
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
                return Ok(format!("Current model: {}", current_model));
            } 
            // Handle switching model
            else if parts.len() >= 2 {
                let new_model = parts[1].to_string();
                {
                    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
                    ollama_state.current_model = new_model.clone();
                }
                return Ok(format!("Switched to model: {}", new_model));
            } else {
                return Err("Invalid model command. Use /model [name] to switch models.".to_string());
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
                return Ok(format!("Current Ollama API host: {}", current_host));
            } 
            // Handle changing host
            else if parts.len() >= 2 {
                let new_host = parts[1].to_string();
                {
                    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
                    ollama_state.api_host = new_host.clone();
                }
                return Ok(format!("Changed Ollama API host to: {}", new_host));
            } else {
                return Err("Invalid host command. Use /host [url] to change the API host.".to_string());
            }
        }
        _ => Err(format!("Unknown command: {}. Type /help for available commands.", command))
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
fn switch_model(model: String, command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.current_model = model.clone();
    Ok(format!("Switched to model: {}", model))
}

// Add function to get current API host
#[command]
fn get_host(command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    Ok(format!("Current Ollama API host: {}", ollama_state.api_host))
}

// Add function to set API host
#[command]
fn set_host(host: String, command_manager: State<'_, CommandManager>) -> Result<String, String> {
    let mut ollama_state = command_manager.ollama.lock().map_err(|e| e.to_string())?;
    ollama_state.api_host = host.clone();
    Ok(format!("Changed Ollama API host to: {}", host))
}

fn main() {
    // Create a new command manager
    let command_manager = CommandManager::new();

    tauri::Builder::default()
        .setup(|_app| {
            // Add any setup logic here
            Ok(())
        })
        .manage(command_manager)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            execute_command,
            terminate_command,
            autocomplete,
            get_working_directory,
            get_home_directory,
            ask_ai,
            get_models,
            switch_model,
            get_host,
            set_host
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
