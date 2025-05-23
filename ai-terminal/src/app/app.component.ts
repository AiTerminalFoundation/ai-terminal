import {
  AfterViewChecked,
  Component,
  ElementRef,
  HostListener,
  NgZone,
  OnDestroy,
  OnInit,
  ViewChild
} from '@angular/core';
import { CommonModule } from '@angular/common';
import { invoke } from "@tauri-apps/api/core";
import { FormsModule } from '@angular/forms';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';

interface CommandHistory {
  command: string;
  output: string[];
  timestamp: Date;
  isComplete: boolean;
  isStreaming?: boolean;
  success?: boolean;
  expectingSshEcho?: boolean;
}

interface ChatHistory {
  message: string;
  response: string;
  timestamp: Date;
  isCommand?: boolean; // Flag to indicate if this is a command message
  codeBlocks?: { code: string, language: string }[]; // Store extracted code blocks
}

interface TerminalSession {
  id: string;
  name: string;
  commandHistory: CommandHistory[];
  currentWorkingDirectory: string;
  isActive: boolean;
  gitBranch: string;
  isSshSessionActive: boolean;
  currentSshUserHost: string | null;
}

@Component({
  selector: 'app-root',
  imports: [CommonModule, FormsModule],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css'
})
export class AppComponent implements OnInit, AfterViewChecked, OnDestroy {
  // Terminal sessions
  terminalSessions: TerminalSession[] = [];
  activeSessionId: string = '';
  
  // Terminal properties
  commandHistory: CommandHistory[] = [];
  currentCommand: string = '';
  isProcessing: boolean = false;
  currentWorkingDirectory: string = '~';
  commandHistoryIndex: number = -1; // Current position in command history navigation
  gitBranch: string = ''; // Add Git branch property
  version: string = ''; // Add version property

  // Autocomplete properties
  autocompleteSuggestions: string[] = [];
  showSuggestions: boolean = false;
  selectedSuggestionIndex: number = -1;

  // AI Chat properties
  chatHistory: ChatHistory[] = [];
  currentQuestion: string = '';
  isProcessingAI: boolean = false;
  isAIPanelVisible: boolean = true;
  currentLLMModel: string = 'llama3.2:latest'; // Default model with proper namespace
  ollamaApiHost: string = 'http://localhost:11434'; // Default Ollama host

  // Resizing properties
  leftPanelWidth: number = 600;
  isResizing: boolean = false;
  startX: number = 0;
  startWidth: number = 0;

  // Event listeners
  private unlistenFunctions: UnlistenFn[] = [];

  // Auto-scroll
  @ViewChild('outputArea') outputAreaRef!: ElementRef;
  shouldScroll = false;

  // Cache home directory path to avoid repeated requests
  private homePath: string | null = null;

  // New property for useProxy
  useProxy: boolean = false;

  // Sudo handling
  isSudoPasswordPrompt: boolean = false;
  // Flag for SSH password prompt
  isSSHPasswordPrompt: boolean = false;
  originalSudoCommand: string = '';
  originalSSHCommand: string = ''; // Added for proactive SSH password
  passwordValue: string = '';
  displayValue: string = '';

  // SSH session state
  isSshSessionActive: boolean = false;
  currentSshUserHost: string | null = null; // To store user@host for current SSH session

  // Constants for SSH interaction
  readonly SSH_NEEDS_PASSWORD_MARKER = "SSH_INTERACTIVE_PASSWORD_PROMPT_REQUESTED";
  readonly SSH_PRE_EXEC_PASSWORD_EVENT = "ssh_pre_exec_password_request";
  readonly COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER = "COMMAND_FORWARDED_TO_ACTIVE_SSH";

  constructor(private sanitizer: DomSanitizer, private ngZone: NgZone) { }

  getPlaceholder(): string {
    if (this.isSudoPasswordPrompt) {
      return 'Sudo Password:';
    }
    if (this.isSSHPasswordPrompt) {
      return 'SSH Password:';
    }
    // TODO: Consider making the default placeholder dynamic if needed, e.g., based on currentWorkingDirectory
    return 'Enter command...';
  }

  // Public method to sanitize HTML content
  public sanitizeHtml(html: string): SafeHtml {
    return this.sanitizer.bypassSecurityTrustHtml(html);
  }

  async ngOnInit() {
    // Initialize first terminal session
    this.createNewSession('Terminal 1', true);
    
    // Load saved command history
    this.loadCommandHistory();

    // Get initial working directory
    await this.getCurrentDirectory();

    // Load version from package.json
    try {
      const packageJson = await import('../../package.json');
      this.version = packageJson.version;
    } catch (error) {
      console.error('Failed to load version from package.json:', error);
      this.version = 'unknown';
    }

    // Clean any existing code blocks to ensure no backticks are displayed
    this.sanitizeAllCodeBlocks();

    // Test the Ollama connection
    await this.testOllamaConnection();

    // Set up event listeners for command output streaming
    try {
      // Listen for command output
      const unlisten1 = await listen('command_output', (event) => {
        this.ngZone.run(() => {
          if (this.commandHistory.length > 0) {
            const currentCmdEntry = this.commandHistory[this.commandHistory.length - 1];
            const outputLine = event.payload as string;

            const { lineToDisplay, newExpectingSshEchoState } = this.cleanOutputLine(
              outputLine,
              currentCmdEntry.command,
              this.isSshSessionActive,
              currentCmdEntry.expectingSshEcho || false
            );
            currentCmdEntry.expectingSshEcho = newExpectingSshEchoState;

            if (lineToDisplay === null) {
              return; // Line was a prompt/echo/artifact and should be hidden
            }

            // Mark command as streaming and handle initial system messages
            if (!currentCmdEntry.isStreaming) {
              currentCmdEntry.isStreaming = true;
              // If the output array currently contains only a system message, clear it.
              if (currentCmdEntry.output.length === 1 &&
                (currentCmdEntry.output[0] === "Processing..." ||
                  currentCmdEntry.output[0] === "Processing sudo command..." ||
                  currentCmdEntry.output[0] === "Command started. Output will stream in real-time." ||
                  currentCmdEntry.output[0] === "Sudo command started. Output will stream in real-time." ||
                  currentCmdEntry.output[0] === "Command sent to active SSH session.")) {
                currentCmdEntry.output = [];
              }
            }

            // Skip specific system messages we don't want to display further
            const skipLine =
              lineToDisplay === "Command started. Output will stream in real-time." ||
              lineToDisplay === "Sudo command started. Output will stream in real-time." ||
              (currentCmdEntry.command.startsWith('sudo ') &&
                outputLine.includes("[sudo] password for"));

            if (!skipLine) {
              currentCmdEntry.output.push(lineToDisplay);
              this.shouldScroll = true;
            }
          }
        });
      });

      // Listen for command errors
      const unlisten2 = await listen('command_error', (event) => {
        this.ngZone.run(() => {
          if (this.commandHistory.length > 0) {
            const currentCmdEntry = this.commandHistory[this.commandHistory.length - 1];
            currentCmdEntry.output.push(event.payload as string);
            this.shouldScroll = true;
          }
        });
      });

      // Listen for command completion
      const unlisten3 = await listen('command_end', async (event) => {
        this.ngZone.run(async () => {
          if (this.commandHistory.length > 0) {
            const currentCmdEntry = this.commandHistory[this.commandHistory.length - 1];
            currentCmdEntry.isComplete = true;
            currentCmdEntry.isStreaming = false;

            // Set success flag based on the exit message
            const message = event.payload as string;
            currentCmdEntry.success = message === "Command completed successfully.";

            // Save command history when a command completes
            this.saveCommandHistory();

            // Handle directory updates for cd commands
            const commandText = currentCmdEntry.command.trim();
            const isCdCommand = commandText === 'cd' || commandText.startsWith('cd ');

            // if (isCdCommand) { // This was original
            // For CD commands, update the directory immediately
            // await this.getCurrentDirectory();
            // }
            // Only update for local cd. SSH cd relies on remote_directory_updated event.
            if (isCdCommand && !this.isSshSessionActive) {
              await this.getCurrentDirectory();
            }

            this.isProcessing = false;
            this.shouldScroll = true;
          }
        });
      });

      // When a command is forwarded to an existing SSH session, we only get this custom event.
      // Treat it like an immediate completion so the input textbox is re-enabled right away.
      const unlistenSshForward = await listen('command_forwarded_to_ssh', (_event) => {
        this.ngZone.run(() => {
          this.isProcessing = false;
        });
      });

      // NEW: Listen for proactive SSH password request from backend
      const unlisten_ssh_prompt = await listen(this.SSH_PRE_EXEC_PASSWORD_EVENT, (event) => {
        this.ngZone.run(() => {
          const originalCommandFromEvent = event.payload as string;

          // This listener is now responsible for creating the CommandHistory entry for the password prompt.
          const sshPromptEntry: CommandHistory = {
            command: originalCommandFromEvent, // Store the original command
            output: [`SSH Password for ${this.extractUserHostFromSshCommand(originalCommandFromEvent)}:`],
            timestamp: new Date(),
            isComplete: false, // Not complete until password submitted or cancelled
            isStreaming: false // Not streaming yet, just prompting
          };
          this.commandHistory.push(sshPromptEntry);
          this.saveCommandHistory(); // Save history

          this.originalSSHCommand = originalCommandFromEvent;
          this.isSSHPasswordPrompt = true;
          this.passwordValue = '';
          this.displayValue = '';
          this.currentCommand = ''; // Clear the input field for password

          this.shouldScroll = true;
          this.isProcessing = false; // Allow password input
          this.focusTerminalInput();
        });
      });

      this.unlistenFunctions.push(unlisten1, unlisten2, unlisten3, unlistenSshForward, unlisten_ssh_prompt);

      // Listen for remote directory updates from SSH
      const unlistenRemoteDir = await listen('remote_directory_updated', (event) => {
        this.ngZone.run(() => {
          const newRemotePath = event.payload as string;
          console.log('Remote directory updated event:', newRemotePath);
          if (this.isSshSessionActive) {
            if (this.currentSshUserHost) {
              this.currentWorkingDirectory = `${this.currentSshUserHost}:${newRemotePath}`;
            } else {
              this.currentWorkingDirectory = newRemotePath; // Fallback
            }
            // Git branch is not applicable for remote, ensure it's clear
            this.gitBranch = '';
          }
        });
      });
      this.unlistenFunctions.push(unlistenRemoteDir);

      // Listen for SSH session start and end events
      const unlistenSshStarted = await listen('ssh_session_started', (event) => {
        this.ngZone.run(async () => {
          console.log("SSH Session Started:", event.payload);
          this.isSshSessionActive = true;
          // When SSH starts, explicitly fetch the directory, which should now be remote.
          await this.getCurrentDirectory();
          this.gitBranch = ''; // Clear local git branch on SSH start
          this.isProcessing = false; // <<< Added this line to re-enable input
        });
      });
      this.unlistenFunctions.push(unlistenSshStarted);

      const unlistenSshEnded = await listen('ssh_session_ended', (event) => {
        this.ngZone.run(async () => {
          console.log("SSH Session Ended:", event.payload);
          this.isSshSessionActive = false;
          this.currentSshUserHost = null; // Clear user@host
          // On SSH end, revert to local directory and git branch.
          await this.getCurrentDirectory();
        });
      });
      this.unlistenFunctions.push(unlistenSshEnded);

    } catch (error) {
      console.error('Failed to set up event listeners:', error);
    }
  }

  ngOnDestroy() {
    // Clean up all event listeners
    for (const unlisten of this.unlistenFunctions) {
      unlisten();
    }
  }

  ngAfterViewChecked() {
    // Scroll to bottom if needed
    if (this.shouldScroll && this.outputAreaRef) {
      this.scrollToBottom();
      this.shouldScroll = false;
    }
  }

  scrollToBottom() {
    try {
      const outputArea = this.outputAreaRef.nativeElement;
      // Force a reflow to ensure the content height is updated
      void outputArea.offsetHeight;
      // Scroll to the maximum possible position
      outputArea.scrollTop = outputArea.scrollHeight;

      // Double-check the scroll position after a small delay
      // This helps with dynamic content that might affect the scroll height
      setTimeout(() => {
        outputArea.scrollTop = outputArea.scrollHeight;
      }, 50);
    } catch (err) {
      console.error('Error scrolling to bottom:', err);
    }
  }

  async getCurrentDirectory() {
    try {
      // Invoke will get either local or remote based on backend logic.
      const result = await invoke<string>("get_working_directory", {
        sessionId: this.activeSessionId
      });

      if (!this.isSshSessionActive) {
        // Local session: get git branch and process home path for tilde expansion
        if (!this.homePath) {
          // If homePath is not cached, fetch it along with the git branch
          const [homePath, gitBranch] = await Promise.all([
            invoke<string>("get_home_directory"),
            invoke<string>("get_git_branch", {
              sessionId: this.activeSessionId
            })
          ]);
          this.homePath = homePath;
          this.gitBranch = gitBranch;
        } else {
          // homePath is cached, just fetch git branch
          this.gitBranch = await invoke<string>("get_git_branch", {
            sessionId: this.activeSessionId
          });
        }

        // Replace local home directory path with ~
        if (this.homePath && result.startsWith(this.homePath)) {
          this.currentWorkingDirectory = '~' + result.substring(this.homePath.length);
        } else {
          this.currentWorkingDirectory = result.trim();
        }
      } else {
        // SSH session: path is remote, display as is. Clear local git branch.
        const remotePath = result.trim();
        if (this.currentSshUserHost) {
          this.currentWorkingDirectory = `${this.currentSshUserHost}:${remotePath}`;
        } else {
          // Fallback if currentSshUserHost is somehow not set (e.g., session restored without command context)
          this.currentWorkingDirectory = remotePath;
        }
        this.gitBranch = '';
      }

    } catch (error) {
      console.error('Failed to get current directory:', error);
      this.currentWorkingDirectory = this.isSshSessionActive ? "remote:error" : "local:error";
      this.gitBranch = ''; // Clear git branch on error too
    }
  }

  @HostListener('document:mousemove', ['$event'])
  onMouseMove(event: MouseEvent) {
    if (this.isResizing) {
      const diff = event.clientX - this.startX;
      const newWidth = this.startWidth + diff;
      this.leftPanelWidth = Math.min(
        Math.max(200, newWidth),
        window.innerWidth * 0.8
      );
    }
  }

  @HostListener('document:touchmove', ['$event'])
  onTouchMove(event: TouchEvent) {
    if (this.isResizing) {
      event.preventDefault(); // Prevent scrolling during resize
      const diff = event.touches[0].clientX - this.startX;
      const newWidth = this.startWidth + diff;
      this.leftPanelWidth = Math.min(
        Math.max(200, newWidth),
        window.innerWidth * 0.8
      );
    }
  }

  @HostListener('document:mouseup')
  onMouseUp() {
    this.isResizing = false;
  }

  @HostListener('document:touchend')
  onTouchEnd() {
    this.isResizing = false;
  }

  // Handle key presses globally
  @HostListener('document:keydown', ['$event'])
  handleKeyboardEvent(event: KeyboardEvent) {


    // Handle Ctrl+C to terminate running command
    if (event.ctrlKey && event.key === 'c' && this.isProcessing) {
      event.preventDefault();
      event.stopPropagation();
      this.terminateCommand();
      return;
    }
  }

  startResize(event: MouseEvent | TouchEvent) {
    this.isResizing = true;
    this.startX = event instanceof MouseEvent ? event.clientX : event.touches[0].clientX;
    this.startWidth = this.leftPanelWidth;
  }

  autoResize(event: Event) {
    const textarea = event.target as HTMLTextAreaElement;
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
  }

  async terminateCommand(): Promise<void> {
    // First, let's force the UI to update immediately
    this.isProcessing = false;

    // Clear any active suggestions
    this.showSuggestions = false;
    this.autocompleteSuggestions = [];

    if (this.commandHistory.length === 0) return;

    const currentCommand = this.commandHistory[this.commandHistory.length - 1];
    // Update UI immediately to show we're handling the termination
    currentCommand.isComplete = true;
    currentCommand.isStreaming = false;
    currentCommand.success = false;
    this.shouldScroll = true;


    // Force immediate UI update and focus the input
    this.focusTerminalInput();

    // Fire and forget - don't await the backend response
    // This ensures the UI stays responsive regardless of how long the backend takes
    invoke<string>("terminate_command", {
      sessionId: this.activeSessionId
    })
      .then(result => {
        console.log('Command terminated:', result);
      })
      .catch(error => {
        console.error('Failed to terminate command:', error);
      });

  }

  async requestAutocomplete(): Promise<void> {
    try {
      const trimmedCommand = this.currentCommand.trim();
      const isCdCommand = trimmedCommand === 'cd' || trimmedCommand.startsWith('cd ');

      // Don't show suggestions for empty input unless it's a cd command with no args
      if (trimmedCommand.length === 0 && !isCdCommand) {
        this.autocompleteSuggestions = [];
        this.showSuggestions = false;
        return;
      }

      // Get autocomplete suggestions from backend
      this.autocompleteSuggestions = await invoke<string[]>("autocomplete", {
        input: this.currentCommand,
        sessionId: this.activeSessionId
      });

      // Don't automatically show suggestions - they will be shown on Tab
      // Just collect them in the background

      // Reset selection index
      this.selectedSuggestionIndex = -1;
    } catch (error) {
      console.error('Failed to get autocomplete suggestions:', error);
      this.autocompleteSuggestions = [];
      this.showSuggestions = false;
    }
  }

  applySuggestion(suggestion: string): void {
    const parts = this.currentCommand.trim().split(' ');

    if (parts.length <= 1) {
      // If it's just one word, replace it
      this.currentCommand = suggestion;
    } else {
      // For cd commands or similar, preserve the command and replace the argument
      const command = parts[0];
      this.currentCommand = `${command} ${suggestion}`;
    }

    // Hide suggestions - won't show again until Tab is pressed
    this.showSuggestions = false;
    this.selectedSuggestionIndex = -1;
  }

  // Helper method to focus the autocomplete container
  focusSuggestions(): void {
    setTimeout(() => {
      const container = document.querySelector('.autocomplete-container');
      if (container) {
        (container as HTMLElement).focus();
      }
    }, 0);
  }

  async executeCommand(event: KeyboardEvent): Promise<void> {
    // Hide suggestions when pressing Esc
    if (event.key === 'Escape') {
      this.showSuggestions = false;
      event.preventDefault();
      return;
    }

    // Handle arrow keys for command history navigation when no suggestions are visible
    if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && !this.showSuggestions) {
      event.preventDefault();
      this.navigateCommandHistory(event.key === 'ArrowUp' ? 'up' : 'down');
      return;
    }

    // Handle password input for sudo or ssh
    if (this.isSudoPasswordPrompt || this.isSSHPasswordPrompt) {
      // Don't show autocomplete for password input
      if (event.key === 'Tab') {
        event.preventDefault();
        return;
      }

      // Handle backspace for password
      if (event.key === 'Backspace') {
        this.passwordValue = this.passwordValue.slice(0, -1);
        this.displayValue = '*'.repeat(this.passwordValue.length);
        this.currentCommand = this.displayValue;
        event.preventDefault();
        return;
      }

      // Handle Enter to submit password
      if (event.key === 'Enter') {
        event.preventDefault();

        const password = this.passwordValue;
        // Reset the password state
        this.passwordValue = '';
        this.displayValue = '';
        this.currentCommand = '';

        const wasSudo = this.isSudoPasswordPrompt;
        this.isSudoPasswordPrompt = false;
        this.isSSHPasswordPrompt = false;
        this.isProcessing = true; // Mark processing while backend works

        // Get the most recent command entry which should be the sudo/ssh command
        let cmdEntry: CommandHistory | undefined;
        if (wasSudo) {
          cmdEntry = this.commandHistory.find(
            entry => entry.command === this.originalSudoCommand &&
              !entry.isComplete && // Find the one still processing/prompting
              entry.output.some(line => line.includes("[sudo] password"))
          );
          if (cmdEntry) {
            cmdEntry.output = ["Processing sudo command..."]; // Update existing entry
            cmdEntry.isStreaming = true;
          }
        } else { // SSH password submission
          cmdEntry = this.commandHistory.find(
            entry => entry.command === this.originalSSHCommand &&
              !entry.isComplete && // Find the one still processing/prompting
              entry.output.some(line => line.startsWith("SSH Password for"))
          );
          if (cmdEntry) {
            cmdEntry.output.push("Processing ssh password..."); // Append to "SSH Password for..."
            cmdEntry.isStreaming = true; // Expecting output from connection attempt
            cmdEntry.expectingSshEcho = true; // Expect the SSH command itself to be echoed by remote PTY
          }
        }

        try {
          if (wasSudo) {
            await invoke<string>("execute_sudo_command", {
              command: this.originalSudoCommand,
              password: password,
              sessionId: this.activeSessionId
            });
          } else { // SSH path: send password to backend
            await invoke<string>("execute_command", { // This is the re-invocation with password
              command: this.originalSSHCommand,
              sshPassword: password,
              sessionId: this.activeSessionId
            });
            // isProcessing will be set to false by ssh_session_started or command_end listeners
          }
        } catch (error) {
          if (cmdEntry) {
            cmdEntry.output.push(`Error: ${error}`);
            cmdEntry.isComplete = true;
            cmdEntry.success = false;
          } // else: if cmdEntry wasn't found, error is unassociated, but will hit general error handler
          this.isProcessing = false; // Ensure input is re-enabled on error here too
        }
        return;
      }

      // For any other key press in password mode, add to password but display asterisk
      if (event.key.length === 1) {
        this.passwordValue += event.key;
        this.displayValue = '*'.repeat(this.passwordValue.length);
        this.currentCommand = this.displayValue;
        event.preventDefault();
        return;
      }

      return;
    }

    // Tab completion - show suggestions
    if (event.key === 'Tab') {
      event.preventDefault();

      // Only trigger autocomplete if there's at least one character
      // Exception: 'cd' command should allow tab completion with empty argument
      const trimmedCommand = this.currentCommand.trim();
      const isCdCommand = trimmedCommand === 'cd' || trimmedCommand.startsWith('cd ');

      if (trimmedCommand.length >= 1 || isCdCommand) {
        // If suggestions are already showing and a suggestion is selected
        if (this.showSuggestions && this.selectedSuggestionIndex >= 0) {
          // Apply the selected suggestion
          this.applySuggestion(this.autocompleteSuggestions[this.selectedSuggestionIndex]);
          // Make sure focus is maintained
          this.focusTerminalInput();
          return;
        }

        // Get suggestions from backend
        await this.requestAutocomplete();

        // Show suggestions if we have any
        if (this.autocompleteSuggestions.length > 0) {
          this.showSuggestions = true;

          // If only one suggestion, apply it directly
          if (this.autocompleteSuggestions.length === 1) {
            this.applySuggestion(this.autocompleteSuggestions[0]);
            // Make sure focus is maintained
            this.focusTerminalInput();
            return;
          }

          // Select the first suggestion by default
          this.selectedSuggestionIndex = 0;

          // Focus the suggestions container for keyboard navigation
          this.focusSuggestions();
        }
      }
      return;
    }

    // Auto-suggest in the background (but don't show) as the user types
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight' &&
      this.currentCommand.trim().length >= 1 && !this.isProcessing) {
      await this.requestAutocomplete();
    }

    // Hide suggestions when pressing Enter to execute command
    if (event.key === 'Enter') {
      // Don't hide suggestions if a suggestion is selected (global handler will handle this case)
      if (!(this.showSuggestions && this.selectedSuggestionIndex >= 0)) {
        this.showSuggestions = false;
      }
    }

    // Execute command on Enter - only if no suggestions are visible or selected
    if (event.key === 'Enter' && !event.shiftKey && this.currentCommand.trim()) {
      // Skip if we're in the process of selecting a suggestion
      if (this.showSuggestions && this.selectedSuggestionIndex >= 0) {
        return;
      }

      event.preventDefault();
      this.isProcessing = true;

      // Clear suggestions when a command is executed
      this.showSuggestions = false;

      // Store command before clearing
      const commandToSend = this.currentCommand.trim();

      // Handle cls/clear command locally
      if (commandToSend === 'cls' || commandToSend === 'clear') {
        // Clear the command history
        this.commandHistory = [];
        // Clear input
        this.currentCommand = '';
        this.isProcessing = false;
        return;
      }

      // Handle sudo commands
      if (commandToSend.startsWith('sudo ')) {
        this.originalSudoCommand = commandToSend;
        this.isSudoPasswordPrompt = true;
        this.passwordValue = '';
        this.displayValue = '';
        this.currentCommand = ''; // Clear input for password

        // Add password prompt to history
        const commandEntry: CommandHistory = {
          command: commandToSend,
          output: ["[sudo] password for user:"], // Placeholder, actual prompt might differ
          timestamp: new Date(),
          isComplete: false,
          isStreaming: true // Will stream after password
        };
        this.commandHistory.push(commandEntry);
        this.saveCommandHistory(); // Save history
        this.shouldScroll = true;
        this.isProcessing = false; // Allow password input
        this.focusTerminalInput();
        return;
      }

      // --- New SSH Proactive Password Handling ---
      const isPlainSsh = commandToSend.startsWith('ssh ') && !commandToSend.startsWith('sudo ssh ');

      if (isPlainSsh) {
        this.currentCommand = ''; // Clear input
        this.commandHistoryIndex = -1; // Reset history navigation
        this.isProcessing = true; // Set processing true for the initial invoke
        this.currentSshUserHost = this.extractUserHostFromSshCommand(commandToSend); // Store user@host

        invoke<string>("execute_command", { 
          command: commandToSend, 
          sshPassword: null,
          sessionId: this.activeSessionId
        })
          .then(result => {
            if (result === this.SSH_NEEDS_PASSWORD_MARKER) {
              // The 'ssh_pre_exec_password_request' event listener will handle:
              // - Creating the CommandHistory entry with the password prompt.
              // - Setting up isSSHPasswordPrompt, originalSSHCommand, etc.
              // - Setting isProcessing = false to allow password input.
              // So, no CommandHistory entry is created here for this specific case.
              // isProcessing will be set to false by the listener.
            } else if (result === this.COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER) {
              // Command was forwarded to an already active SSH session.
              // Create a history entry for the command that was forwarded.
              const forwardedCommandEntry: CommandHistory = {
                command: commandToSend,
                output: [], // Output will come from 'command_output' events via the active session
                timestamp: new Date(),
                isComplete: false, // This command is ongoing within the SSH session
                isStreaming: true, // Expecting streamed output
                expectingSshEcho: true // Expect the remote shell to echo this command
              };
              this.commandHistory.push(forwardedCommandEntry);
              this.saveCommandHistory();
              this.isProcessing = false; // Input can be re-enabled.
            } else {
              // Other direct results: e.g., key authentication worked, or an immediate error occurred.
              // Create a history entry for this command.
              const directResultEntry: CommandHistory = {
                command: commandToSend,
                output: result ? [result] : [], // If result is empty, output is empty array
                timestamp: new Date(),
                // This command might be complete or might start streaming.
                // If 'result' contains typical streaming start messages, it's not complete.
                // Otherwise, assume it's complete unless output events follow.
                // For simplicity here, we'll rely on 'command_end' to mark true completion.
                isComplete: false,
                success: undefined, // Will be set by command_end
                isStreaming: !!result // If there's any initial result, consider it streaming.
                // Backend's "Output will stream..." message is a good indicator.
              };
              this.commandHistory.push(directResultEntry);
              this.saveCommandHistory();
              // If the command isn't one that typically streams (like an immediate error message),
              // or if it's a success that doesn't stream (less common for SSH connect),
              // isProcessing should be false. The command_end event is the primary way to set this.
              // For now, optimistically set to false if no streaming indicators.
              if (!result || (!result.includes("Output will stream") && !result.includes(this.COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER))) {
                this.isProcessing = false;
              }
              // If backend sends "Output will stream...", isProcessing remains true until command_end
            }
            this.shouldScroll = true;
          })
          .catch(error => {
            // Handle errors from the initial invoke call for SSH itself.
            const errorEntry: CommandHistory = {
              command: commandToSend,
              output: [`Error initiating SSH: ${error}`],
              timestamp: new Date(),
              isComplete: true,
              success: false
            };
            this.commandHistory.push(errorEntry);
            this.saveCommandHistory();
            this.isProcessing = false;
            this.shouldScroll = true;
          });
        return; // Prevent falling through to generic command handling
      }
      // --- End of New SSH Proactive Password Handling ---

      // Generic command handling (not sudo, not plain SSH initial attempt)
      // This will also handle commands typed *after* an SSH session is active.
      this.isProcessing = true;
      const commandEntry: CommandHistory = {
        command: commandToSend,
        output: [], // Start with an empty array
        timestamp: new Date(),
        isComplete: false
      };
      // If currently in an active SSH session, mark that we expect an echo.
      if (this.isSshSessionActive) {
        commandEntry.expectingSshEcho = true;
      }
      this.commandHistory.push(commandEntry);
      this.saveCommandHistory();

      // Clear input immediately
      this.currentCommand = '';

      // Reset command history navigation index
      this.commandHistoryIndex = -1;

      // For cd commands, update directory proactively
      const isCdCommand = commandToSend === 'cd' || commandToSend.startsWith('cd ');
      // if (isCdCommand) { // Original logic
      // Update directory immediately to reduce perceived lag
      // Will be refreshed again when command completes
      // setTimeout(() => this.getCurrentDirectory(), 50);
      // }
      // For local cd, update proactively. For remote, rely on events.
      if (isCdCommand && !this.isSshSessionActive) {
        setTimeout(() => this.getCurrentDirectory(), 50);
      }

      try {
        // Execute command using Tauri
        // For non-streaming commands, the result will be returned directly
        // For streaming commands, the events will update the output
        // Pass sshPassword: null in case the backend signature expects it generally,
        // it will be ignored if not relevant for this specific command execution path.
        const result = await invoke<string>("execute_command", { 
          command: commandToSend, 
          sshPassword: null,
          sessionId: this.activeSessionId
        });

        // If the result indicates the command was forwarded to an active SSH session
        if (result === this.COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER) {
          this.isProcessing = false;
          // Mark that we expect the SSH shell to echo the command
          if (commandEntry) { // commandEntry is the last pushed entry
            commandEntry.expectingSshEcho = true;
          }
          // The commandEntry for this forwarded command remains 'isComplete = false'
          // and 'isStreaming = false' (or true if output starts).
          // Its output will be populated by 'command_output' events from the SSH PTY.
          // It will only be marked 'isComplete = true' when the entire SSH session ends
          // and the original SSH command's PTY emits 'command_end'.
        } else if (result && result.trim() !== "") {
          // Avoid pushing the old "Command sent to active SSH session." message if it's still sent by backend,
          // as the new marker handles the logic.
          // This also prevents adding empty strings or whitespace-only results.
          if (result.trim() !== "Command sent to active SSH session.") {
            commandEntry.output.push(result);
          }
        }

        // Note: We don't mark the command as complete here for most cases.
        // For regular commands, 'command_end' event listener will handle completion and isProcessing = false.
        // For commands forwarded to SSH, 'isProcessing' is handled above or by 'command_forwarded_to_ssh' event.
      } catch (error) {
        commandEntry.output = [`Error: ${error}`];
        commandEntry.isComplete = true;
        commandEntry.success = false; // Explicitly mark as failed
        this.isProcessing = false;
      }
    }
  }

  private stripAnsiCodes(text: string): string {
    // Regex to remove ANSI escape codes
    return text.replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, '');
  }

  private stripTerminalTitle(text: string): string {
    // Regex to remove terminal title escape sequences like \x1b]0;...\x07
    return text.replace(/\x1b\]0;.*\x07/g, '');
  }

  private cleanOutputLine(rawLine: string, commandText: string, isSsh: boolean, isCurrentlyExpectingEcho: boolean): { lineToDisplay: string | null, newExpectingSshEchoState: boolean } {
    let cleanedLine = this.stripAnsiCodes(rawLine);
    cleanedLine = this.stripTerminalTitle(cleanedLine);

    let finalLineToDisplay: string | null = cleanedLine;
    let updatedExpectingEcho = isCurrentlyExpectingEcho;

    if (isSsh) {
      const trimmedCleanedLine = cleanedLine.trim();
      const trimmedCommandText = commandText.trim();

      if (isCurrentlyExpectingEcho) {
        // For 'cd' commands in SSH, our backend appends marker logic.
        // The echoed line will be complex. If we see our marker, hide it.
        if (trimmedCommandText.startsWith('cd ') && cleanedLine.includes('__REMOTE_CD_PWD_MARKER_')) {
          finalLineToDisplay = null;
          updatedExpectingEcho = false; // Echo handled
          return { lineToDisplay: finalLineToDisplay, newExpectingSshEchoState: updatedExpectingEcho };
        }

        // Original echo hiding logic for other commands:
        const commandStartIndex = trimmedCleanedLine.indexOf(trimmedCommandText);
        if (commandStartIndex !== -1) {
          // Consider it an echo if the command is found.
          // More sophisticated checks could verify what comes before (prompt) and after (nothing).
          finalLineToDisplay = null; // Hide this line
          updatedExpectingEcho = false; // We've processed the expected echo
          return { lineToDisplay: finalLineToDisplay, newExpectingSshEchoState: updatedExpectingEcho };
        }
        // If command not found on this line, but we were expecting echo, maybe it's a multi-line prompt or just a weird first line.
        // For safety, stop expecting echo after the first line is processed when in this state.
        // updatedExpectingEcho = false; 
        // ^ Let's be more conservative: only stop expecting if we positively ID the echo.
        // If the first line wasn't it, subsequent lines are definitely not the echo.
        // However, if the first line *was* an empty line after stripping, we should still expect echo.
        if (trimmedCleanedLine !== "") { // if it wasn't just an empty line
          updatedExpectingEcho = false;
        }

      }

      // Try to detect and hide lines that are *only* a shell prompt.
      // Example: `pi@pi:~$ ` or `$`
      // This regex is a basic attempt and might need refinement for different prompts.
      const promptRegex = /^([\w\W]*?([\w.-]+@[\w.-]+(:\s?[\/\w\.~-]+)?)\s*)?([\$#%])\s*$/;
      if (promptRegex.test(trimmedCleanedLine)) {
        // Check if this line *only* contains the prompt and nothing else of substance
        // For example, if `trimmedCleanedLine` is `pi@pi:~$ ` or `$`
        // A simple check: if removing the prompt leaves an empty string.
        const potentialPromptOnly = trimmedCleanedLine.replace(promptRegex, "").trim();
        if (potentialPromptOnly === "") {
          finalLineToDisplay = null; // Hide this line as it's just a prompt
          // Don't change updatedExpectingEcho here, a prompt can appear before or after an echo.
        }
      }
    }

    // If, after all stripping, the line is empty, don't display it unless it was an intentional empty line from the command.
    // However, the newline characters are preserved if the original rawLine had them, 
    // so `cleanedLine` might be "\n" which is fine.
    // What we want to avoid is displaying lines that become empty *after* stripping codes/prompts.
    if (finalLineToDisplay !== null && finalLineToDisplay.trim() === "" && rawLine.trim() !== "") {
      // It became empty after cleaning, and wasn't originally empty. Hide it.
      // Exception: if the original line was just ANSI codes that we stripped, that's fine.
      // This case is mostly for prompts that are stripped entirely.
      // if (this.stripAnsiCodes(rawLine).trim() === "") {
      // Original was just ANSI, now empty, this is an empty line effectively, display if not null already
      // } else {
      finalLineToDisplay = null;
      // }
    }

    return { lineToDisplay: finalLineToDisplay, newExpectingSshEchoState: updatedExpectingEcho };
  }

  // Add a new method to parse commands from AI responses
  parseCommandFromResponse(response: string): { command: string, fullText: string }[] {
    const results: { command: string, fullText: string }[] = [];
    let lastIndex = 0;

    // First handle triple backtick blocks
    const tripleCommandRegex = /```([^`]+)```/g;
    let match;

    while ((match = tripleCommandRegex.exec(response)) !== null) {
      // Get the text before this command block
      const textBefore = response.slice(lastIndex, match.index);

      // Process any single backticks in the text before
      if (textBefore) {
        const processedText = this.processSingleBackticks(textBefore);
        if (processedText) {
          results.push({
            command: '',
            fullText: processedText
          });
        }
      }

      // Add the command with its backticks
      results.push({
        command: match[1].trim(),
        fullText: match[0] // Include the full match with backticks
      });

      lastIndex = match.index + match[0].length;

      // Check if there's an escaped newline (\\n) after this command
      const nextChars = response.slice(lastIndex, lastIndex + 4);
      if (nextChars === '\\n') {
        results.push({
          command: '',
          fullText: '\n'
        });
        lastIndex += 4; // Skip the \\n
      }
    }

    // Process any remaining text for single backticks
    const textAfter = response.slice(lastIndex);
    if (textAfter) {
      const processedText = this.processSingleBackticks(textAfter);
      if (processedText) {
        results.push({
          command: '',
          fullText: processedText
        });
      }
    }

    return results;
  }

  // Helper method to process single backticks into bold text
  private processSingleBackticks(text: string): string {
    // Replace single backticks with bold markers and sanitize
    const processed = text.replace(/`([^`]+)`/g, '<b>$1</b>');
    return processed;
  }

  // Extract code blocks from response text
  extractCodeBlocks(text: string): { formattedText: string, codeBlocks: { code: string, language: string }[] } {
    const codeBlocks: { code: string, language: string }[] = [];

    // Special handling for command responses (single line enclosed in triple backticks)
    const commandParts = this.parseCommandFromResponse(text);
    if (commandParts.length > 0) {
      console.log("Found command parts:", commandParts);

      // Build the formatted text with placeholders and collect code blocks
      const formattedParts = commandParts.map((part) => {
        if (part.command) {
          // This is a command block
          codeBlocks.push({
            code: part.command,
            language: 'command'
          });
          return `<code-block-${codeBlocks.length - 1}></code-block-${codeBlocks.length - 1}>`;
        } else {
          // This is regular text
          return part.fullText;
        }
      });

      // Join with newlines to preserve the model's formatting
      return {
        formattedText: formattedParts.join(''),
        codeBlocks
      };
    }

    // First, check if the entire response is just a single code block with backticks
    if (text.trim().startsWith('```') && text.trim().endsWith('```')) {
      const trimmedText = text.trim();
      // Check if there's any content inside the backticks
      const content = trimmedText.slice(3, -3).trim();
      if (content) {
        // Check if the first line might be a language identifier
        const lines = content.split('\n');
        let code: string;
        let language: string = 'text';

        if (lines.length > 1 && !lines[0].includes(' ') && lines[0].length < 20) {
          // First line might be a language identifier
          language = lines[0];
          code = lines.slice(1).join('\n').trim();
        } else {
          // No language identifier
          code = content;
        }

        codeBlocks.push({ code, language });
        return { formattedText: `<code-block-0></code-block-0>`, codeBlocks };
      }
    }

    // If this is a very short response (e.g., just a command), treat it as a command
    if (text.length < 100 && !text.includes('\n') && !text.includes('```')) {
      codeBlocks.push({
        code: text.trim(),
        language: 'command'
      });

      return { formattedText: `<code-block-0></code-block-0>`, codeBlocks };
    }

    // If not a single block, use regex to find all occurrences of text between triple backticks
    // This regex handles the triple backtick pattern with optional language identifier
    const codeBlockRegex = /```([\w-]*)?(?:\s*\n)?([\s\S]*?)```/gm;

    // Replace code blocks with placeholders while storing extracted code
    let formattedText = text.replace(codeBlockRegex, (language, code) => {
      // Skip empty matches
      if (!code || !code.trim()) {
        return '';
      }

      const trimmedCode = code.trim();
      const index = codeBlocks.length;

      codeBlocks.push({
        code: trimmedCode,
        language: language ? language.trim() : 'text'
      });

      // Return a placeholder that won't be confused with actual content
      return `<code-block-${index}></code-block-${index}>`;
    });

    return { formattedText, codeBlocks };
  }

  // Handle code copy button click
  copyCodeBlock(code: string): void {
    this.copyToClipboard(code);

    // Show a brief "Copied!" notification
    this.showCopiedNotification();
  }

  // Add visual feedback when copying
  showCopiedNotification(): void {
    const notification = document.createElement('div');
    notification.className = 'copy-notification';
    notification.textContent = 'Copied!';
    document.body.appendChild(notification);

    // Animate and remove
    setTimeout(() => {
      notification.classList.add('show');
      setTimeout(() => {
        notification.classList.remove('show');
        setTimeout(() => {
          document.body.removeChild(notification);
        }, 300);
      }, 1200);
    }, 10);
  }

  // Check if a code block is a simple command (no special formatting needed)
  isSimpleCommand(code: string): boolean {
    if (!code) return false;

    // Clean the code first by removing any backticks
    const cleanCode = code.replace(/```/g, '').trim();

    // For commands extracted by parseCommandFromResponse, we always return true
    // if they're relatively short and simple
    if (cleanCode.length < 100 && !cleanCode.includes('\n')) {
      // Check if it has common terminal command patterns
      if (cleanCode.split(' ').length <= 5) {
        return true;
      }
    }

    // A simple command is a single line terminal command that doesn't need
    // a full code block for display
    const isSimple = !cleanCode.includes('\n') &&
      !cleanCode.includes('|') &&
      !cleanCode.includes('>') &&
      !cleanCode.includes('<') &&
      !cleanCode.includes('=') &&
      cleanCode.length < 80;

    // Specific check for common commands
    const isCommonCommand = cleanCode.startsWith('ls') ||
      cleanCode.startsWith('cd') ||
      cleanCode.startsWith('mkdir') ||
      cleanCode.startsWith('rm') ||
      cleanCode.startsWith('cp') ||
      cleanCode.startsWith('mv') ||
      cleanCode.startsWith('cat') ||
      cleanCode.startsWith('grep') ||
      cleanCode.startsWith('find') ||
      cleanCode.startsWith('echo');

    return isSimple && (isCommonCommand || cleanCode.split(' ').length <= 3);
  }

  // Helper method to directly call Ollama API from frontend
  async callOllamaDirectly(question: string, model: string): Promise<string> {
    try {
      console.log(`Calling Ollama API with model: ${model}`);

      // Get the current operating system
      const os = navigator.platform.toLowerCase().includes('mac') ?
        'macOS' : navigator.platform.toLowerCase().includes('win') ?
          'Windows' : 'Linux';

      // Create a system prompt that includes OS information and formatting instructions
      const systemPrompt = `You are a helpful terminal assistant. The user is using a ${os} operating system. 
When providing terminal commands, you MUST follow this EXACT format without any deviations:

CRITICAL FORMAT RULES:
1. Each command block must be on ONE LINE ONLY - NO NEWLINES INSIDE COMMAND BLOCKS
2. Each command must be followed by a colon and a space, then the explanation
3. Use exactly three backticks to wrap each command
4. Put each command-explanation pair on its own line using \\n
5. NEVER include language identifiers (like 'bash')
6. NEVER include newlines or line breaks inside the command blocks

Examples of INCORRECT format:
❌ \`\`\`ls
\`\`\` : Lists files (NO NEWLINES IN COMMAND)
❌ \`\`\`bash ls\`\`\` : Lists files (NO LANGUAGE IDENTIFIERS)
❌ \`\`\`ls\`\`\` Lists files (MISSING COLON)
❌ \`\`\`ls -la\`\`\`
   : Lists all files (NO SEPARATE LINES)

Your response must look EXACTLY like the correct format above, with:
- One command per line or if you need to run multiple commands together, put them on the same line separated by a & symbol
- No newlines within command blocks
- A colon and space after each command block
- A brief explanation after the colon
- Use the html new line character to separate each command-explanation pair, do not use any other newline method

Example of CORRECT format:
\`\`\`ls\`\`\` : Lists files in current directory \`\`\`pwd && ls\`\`\` : Shows current directory path and lists files\`\`\`cd Documents\`\`\` : Changes to Documents directory

IMPORTANT RULES:
1. NEVER use 'bash' or any other language identifier
2. NEVER include backticks within the command itself
3. ALWAYS put each command on a new line using the html new line character
4. ALWAYS use exactly three backticks (\`\`\`) around each command
5. ALWAYS follow each command with : and a brief explanation`;

      // Combine the system prompt with the user's question
      const combinedPrompt = `${systemPrompt}\n\nUser: ${question}`;

      const requestBody = {
        model: model,
        prompt: combinedPrompt,
        stream: false
      };

      // Use relative path with proxy instead of absolute URL
      const apiEndpoint = this.useProxy ? '/api/generate' : `${this.ollamaApiHost}/api/generate`;
      console.log(`Sending request to ${apiEndpoint}`, requestBody);

      // Call Ollama directly
      const response = await fetch(apiEndpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(requestBody)
      });

      console.log(`Response status: ${response.status}`);

      if (!response.ok) {
        const errorText = await response.text();
        console.error(`Ollama API error (${response.status}):`, errorText);
        throw new Error(`Ollama API error: ${response.status} - ${errorText}`);
      }

      const data = await response.json();
      console.log('Ollama response:', data);

      if (!data.response) {
        console.error('Unexpected response format:', data);
        return 'Error: Unexpected response format from Ollama';
      }

      return data.response;
    } catch (error: any) {
      console.error('Error calling Ollama API directly:', error);

      // Add more specific error messages for different failure types
      if (error instanceof TypeError && error.message.includes('Failed to fetch')) {
        return `Error: Could not connect to Ollama at ${this.ollamaApiHost}. Make sure Ollama is running.`;
      }

      return `Error: ${error.message || 'Unknown error calling Ollama API'}`;
    }
  }

  async askAI(event: KeyboardEvent): Promise<void> {
    // Skip if not Enter key or Shift+Enter held (for newlines)
    if (event.key !== 'Enter' || event.shiftKey) {
      return;
    }

    event.preventDefault();

    // Skip if no question or currently processing
    if (!this.currentQuestion.trim() || this.isProcessingAI) {
      return;
    }

    // Handle commands (starting with /)
    const isCommand = this.currentQuestion.startsWith('/');
    let response = '';

    this.isProcessingAI = true;

    try {
      // Add to chat history immediately to show pending state
      const chatEntry: ChatHistory = {
        message: this.currentQuestion,
        response: "Thinking...",
        timestamp: new Date(),
        isCommand: isCommand
      };

      this.chatHistory.push(chatEntry);
      this.shouldScroll = true;

      if (isCommand) {
        response = await this.handleAICommand(this.currentQuestion);
      } else {
        // Verify the model exists before calling Ollama
        const modelExists = await this.checkModelExists(this.currentLLMModel);

        if (!modelExists) {
          // The default model doesn't exist, and we've already tried to auto-switch
          response = "Error: The model could not be found. Please check available models with /models and select one with /model [name].";
        } else {
          // Call Ollama directly
          response = await this.callOllamaDirectly(this.currentQuestion, this.currentLLMModel);

          // Check if the response contains a command we can execute
          const commandParts = this.parseCommandFromResponse(response);
          const hasCommands = commandParts.some(part => part.command);
          if (hasCommands) {
            // If this is a direct shell command question, we can enhance the UI by marking it as a command
            chatEntry.isCommand = true;
          }
        }
      }

      // Use the new method to process the response
      this.processNewChatEntry(chatEntry, response);

      // Clear current question and scroll to bottom
      this.currentQuestion = '';
      this.shouldScroll = true;
    } catch (error) {
      console.error('Failed to process AI request:', error);
      this.chatHistory[this.chatHistory.length - 1].response = `Error: ${error}`;
    } finally {
      this.isProcessingAI = false;
    }
  }

  async copyToClipboard(text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      console.error('Failed to copy text: ', err);
    }
  }

  // Add helper method to filter out completion messages
  getFilteredOutput(output: string[]): string {
    return output
      .filter(line =>
        !line.includes('Command completed successfully') &&
        !line.includes('Command failed.'))
      .join('\n');
  }

  toggleAIPanel(): void {
    this.isAIPanelVisible = !this.isAIPanelVisible;

    // If we're showing the AI panel again, restore the previous width
    // Otherwise the terminal panel will use the full-width class from the CSS
    if (this.isAIPanelVisible) {
      // Make sure the terminal isn't too wide or too narrow
      this.leftPanelWidth = Math.min(
        Math.max(200, this.leftPanelWidth),
        window.innerWidth * 0.6
      );
    }
  }

  // Helper method to determine if a chat history entry is a command response
  isCommandResponse(entry: ChatHistory): boolean {
    return !!entry.isCommand;
  }

  // Code specific functions
  isCodeBlockPlaceholder(text: string): boolean {
    // Check for exact match of <code-block-N> format
    const exactMatch = /^<code-block-\d+><\/code-block-\d+>$/.test(text);

    if (exactMatch) {
      return true;
    }

    // More flexible check for variations of the format
    return text.trim().startsWith('<code-block-') && text.trim().includes('>');
  }

  getCodeBlockIndex(placeholder: string): number {
    // First try to match the full placeholder with opening and closing tags
    let match = placeholder.match(/<code-block-(\d+)><\/code-block-\d+>/);

    // If that doesn't work, try a more flexible approach for partial matches
    if (!match) {
      match = placeholder.match(/<code-block-(\d+)>/);
    }

    return match ? parseInt(match[1]) : -1;
  }

  // Handle AI commands starting with /
  async handleAICommand(command: string): Promise<string> {
    const parts = command.split(' ');
    const cmd = parts[0].toLowerCase();

    switch (cmd) {
      case '/help':
        return `
Available commands:
/help - Show this help message
/models - List available models
/model [name] - Show current model or switch to a different model
/host [url] - Show current API host or set a new one
/retry - Retry connection to Ollama API
/clear - Clear the AI chat history`;

      case '/models':
        try {
          // Get list of models directly from Ollama API
          const response = await fetch(`${this.ollamaApiHost}/api/tags`);

          if (!response.ok) {
            throw new Error(`Ollama API error: ${response.status}`);
          }

          const data = await response.json();

          // Format the response
          let result = 'Available models:\n';
          for (const model of data.models) {
            result += `- ${model.name} (${model.size} bytes)\n`;
          }
          return result;
        } catch (error) {
          return `Error: Failed to get models from Ollama API: ${error}`;
        }

      case '/model':
        if (parts.length > 1) {
          const modelName = parts[1];
          try {
            // Just update the model locally - no need to call backend
            this.currentLLMModel = modelName;
            return `Switched to model: ${modelName}`;
          } catch (error) {
            return `Error: Failed to switch model: ${error}`;
          }
        } else {
          return `Current model: ${this.currentLLMModel}`;
        }

      case '/host':
        if (parts.length > 1) {
          const hostUrl = parts.slice(1).join(' ');
          try {
            // Update API host locally
            this.ollamaApiHost = hostUrl;
            // Test the connection with the new host
            setTimeout(() => this.testOllamaConnection(), 100);
            return `Changed Ollama API host to: ${hostUrl}`;
          } catch (error) {
            return `Error: Failed to set host: ${error}`;
          }
        } else {
          return `Current Ollama API host: ${this.ollamaApiHost}`;
        }

      case '/retry':
        // Retry connection and return a message
        setTimeout(() => this.retryOllamaConnection(), 100);
        return `Attempting to reconnect to Ollama API...`;

      case '/clear':
        // Clear the chat history
        this.chatHistory = [];
        return `AI chat history cleared`;

      default:
        return `Unknown command: ${cmd}. Type /help for available commands.`;
    }
  }

  // Get the original unprocessed response for copying
  getOriginalResponse(entry: ChatHistory): string {
    // If there are no code blocks, just return the response
    if (!entry.codeBlocks || entry.codeBlocks.length === 0) {
      return entry.response;
    }

    // Otherwise reconstruct the original response by replacing placeholders with code only (no backticks)
    let originalResponse = entry.response;
    for (let i = 0; i < entry.codeBlocks.length; i++) {
      const placeholder = `<code-block-${i}></code-block-${i}>`;

      // Just use the code without any backticks or formatting
      originalResponse = originalResponse.replace(placeholder, entry.codeBlocks[i].code);
    }

    return originalResponse;
  }

  // Copy the full response including code blocks
  copyFullResponse(entry: ChatHistory): void {
    this.copyToClipboard(this.getOriginalResponse(entry));
    this.showCopiedNotification();
  }

  // Method to get command explanation from code block
  getCommandExplanation(code: string): string | null {
    if (!code) return null;

    // Split by colon to separate command from explanation
    const parts = code.split(':');

    // If there's more than one part and the second part isn't empty
    if (parts.length > 1 && parts[1].trim()) {
      // Return everything after the first colon
      return parts.slice(1).join(':').trim();
    }

    return null;
  }

  // Update transformCodeForDisplay to handle explanations
  transformCodeForDisplay(code: string): string {
    if (!code) return '';

    // Remove any backticks
    let cleanCode = code.replace(/```/g, '').trim();

    // If there's a colon, only show the command part
    const colonIndex = cleanCode.indexOf(':');
    if (colonIndex > -1) {
      cleanCode = cleanCode.substring(0, colonIndex).trim();
    }

    return cleanCode;
  }

  // Make sure all code blocks in the chat history are properly sanitized
  sanitizeAllCodeBlocks(): void {
    // Go through all chat history entries
    for (const entry of this.chatHistory) {
      // Skip entries without code blocks
      if (!entry.codeBlocks || entry.codeBlocks.length === 0) {
        continue;
      }

      // Sanitize each code block to remove backticks
      for (const codeBlock of entry.codeBlocks) {
        codeBlock.code = this.transformCodeForDisplay(codeBlock.code);
      }
    }
  }

  // Process newly added chat entry
  processNewChatEntry(entry: ChatHistory, response: string): void {
    // Process the response to extract code blocks
    const { formattedText, codeBlocks } = this.extractCodeBlocks(response);

    // Sanitize all code blocks to remove backticks
    for (const codeBlock of codeBlocks) {
      codeBlock.code = this.transformCodeForDisplay(codeBlock.code);
    }

    // Update the chat entry
    entry.response = formattedText;
    entry.codeBlocks = codeBlocks;
  }

  // Handle input events as user types
  onKeyInput(event: Event | string): void {
    // Auto-resize textarea if event is not a string
    if (typeof event !== 'string') {
      this.autoResize(event);
    }

    // When in password mode, don't do autocomplete
    if (this.isSudoPasswordPrompt || this.isSSHPasswordPrompt) {
      return;
    }

    // Check input content after any change
    const trimmedCommand = this.currentCommand.trim();

    // Clear suggestions if input is empty or only contains spaces
    if (trimmedCommand.length === 0) {
      this.showSuggestions = false;
      return;
    }

    // Only update suggestions in the background but never show them
    // They will be shown only when the user presses Tab
    if (!this.isProcessing) {
      this.requestAutocomplete();
    }

    // Trigger scroll to bottom when typing
    this.shouldScroll = true;
  }

  // Handle click on suggestion
  selectSuggestion(suggestion: string, event: MouseEvent): void {
    // Apply the suggestion
    this.applySuggestion(suggestion);

    // Hide suggestions until Tab is pressed again
    this.showSuggestions = false;

    // Focus the terminal input
    this.focusTerminalInput();

    // Prevent the event from bubbling
    event.preventDefault();
    event.stopPropagation();
  }

  // Helper method to focus the terminal textarea
  focusTerminalInput(): void {
    setTimeout(() => {
      const textarea = document.querySelector('.terminal-panel .prompt-container textarea');
      if (textarea) {
        (textarea as HTMLTextAreaElement).focus();
      }
    }, 0);
  }

  // Navigate to the next suggestion (for arrow keys)
  navigateToSuggestion(direction: 'up' | 'down'): void {
    if (!this.showSuggestions || this.autocompleteSuggestions.length === 0) {
      return;
    }

    if (direction === 'down') {
      this.selectedSuggestionIndex = Math.min(
        this.selectedSuggestionIndex + 1,
        this.autocompleteSuggestions.length - 1
      );
    } else {
      this.selectedSuggestionIndex = Math.max(this.selectedSuggestionIndex - 1, 0);
    }

    // Make sure the suggestions container maintains focus
    this.focusSuggestions();
  }

  // Navigate through command history with arrow keys
  navigateCommandHistory(direction: 'up' | 'down'): void {
    // Do nothing if there's no command history or processing a command
    if (this.commandHistory.length === 0 || this.isProcessing) {
      return;
    }

    if (direction === 'up') {
      // If not navigating history yet, start from the last command
      if (this.commandHistoryIndex === -1) {
        this.commandHistoryIndex = this.commandHistory.length - 1;
      } else {
        // Move up in history (if not at the beginning)
        this.commandHistoryIndex = Math.max(0, this.commandHistoryIndex - 1);
      }

      // Set the current command to the historical command
      this.currentCommand = this.commandHistory[this.commandHistoryIndex].command;
    } else if (direction === 'down') {
      // If already at the end of history, do nothing
      if (this.commandHistoryIndex === -1) {
        return;
      }

      // Move down in history
      this.commandHistoryIndex++;

      // If we went past the end of history, clear input and reset index
      if (this.commandHistoryIndex >= this.commandHistory.length) {
        this.currentCommand = '';
        this.commandHistoryIndex = -1;
      } else {
        // Otherwise set to the command at current index
        this.currentCommand = this.commandHistory[this.commandHistoryIndex].command;
      }
    }

    // Make sure the terminal input maintains focus
    this.focusTerminalInput();
  }

  // Load command history from localStorage
  loadCommandHistory(): void {
    // Initialize with empty array - no longer loading from localStorage
    this.commandHistory = [];
  }

  // Save command history to localStorage
  saveCommandHistory(): void {
    // Do nothing - no longer saving to localStorage
    // Command history will be kept in memory only and cleared on refresh
  }

  // Test the Ollama connection
  async testOllamaConnection(): Promise<void> {
    try {
      // Try to fetch the list of models to test the connection
      const response = await fetch(`${this.ollamaApiHost}/api/tags`, {
        method: 'GET'
      });

      if (response.ok) {
        console.log('Ollama connection test successful');
        // We could also pre-populate the model list here
        const data = await response.json();
        if (data && data.models && data.models.length > 0) {
          const availableModels = data.models.map((m: any) => m.name).join(', ');
          console.log(`Available models: ${availableModels}`);

          // Set default model to the first available model if our default isn't in the list
          const modelExists = data.models.some((m: any) => m.name === this.currentLLMModel);
          if (!modelExists && data.models.length > 0) {
            this.currentLLMModel = data.models[0].name;
            console.log(`Set default model to ${this.currentLLMModel}`);

            // Notify in chat history
            this.chatHistory.push({
              message: " System",
              response: `Connected to Ollama API. Available models: ${availableModels}
Using: ${this.currentLLMModel}`,
              timestamp: new Date(),
              isCommand: true
            });
          } else if (modelExists) {
            // If our model exists, just show success
            this.chatHistory.push({
              message: " System",
              response: `Connected to Ollama API. Using model: ${this.currentLLMModel}`,
              timestamp: new Date(),
              isCommand: true
            });
          }
        } else {
          // Ollama is running but no models are available
          this.chatHistory.push({
            message: "System",
            response: `Connected to Ollama API, but no models are available. Please install models with \"ollama pull <model>\".`,
            timestamp: new Date(),
            isCommand: true
          });
        }
      } else {
        console.error('Ollama connection test failed:', response.status);
        // Add to chat history a message about Ollama not being available
        this.chatHistory.push({
          message: "System",
          response: "Could not connect to Ollama API. Please make sure Ollama is running on " +
            this.ollamaApiHost + " or change the host using /host command.",
          timestamp: new Date(),
          isCommand: true
        });
      }
    } catch (error) {
      console.error('Error testing Ollama connection:', error);
      // Add to chat history a message about Ollama not being available
      this.chatHistory.push({
        message: "System",
        response: "Could not connect to Ollama API. Please make sure Ollama is running on " +
          this.ollamaApiHost + " or change the host using /host command.",
        timestamp: new Date(),
        isCommand: true
      });
    }
  }

  // Method to retry Ollama connection
  async retryOllamaConnection(): Promise<void> {
    this.chatHistory.push({
      message: "System",
      response: "🔄 Retrying connection to Ollama API...",
      timestamp: new Date(),
      isCommand: true
    });
    await this.testOllamaConnection();
  }

  // Check if a specific model exists in Ollama
  async checkModelExists(modelName: string): Promise<boolean> {
    try {
      console.log(`Checking if model ${modelName} exists...`);
      // Try to fetch the list of models
      const response = await fetch(`${this.ollamaApiHost}/api/tags`, {
        method: 'GET'
      });

      if (!response.ok) {
        console.error(`Failed to get models: ${response.status}`);
        return false;
      }

      const data = await response.json();

      if (!data.models || !Array.isArray(data.models)) {
        console.error('Unexpected response format when checking models:', data);
        return false;
      }

      const modelExists = data.models.some((m: any) => m.name === modelName);
      console.log(`Model ${modelName} exists: ${modelExists}`);

      if (!modelExists) {
        console.log('Available models:', data.models.map((m: any) => m.name).join(', '));

        // If model doesn't exist, automatically switch to the first available model
        if (data.models.length > 0) {
          this.currentLLMModel = data.models[0].name;
          console.log(`Auto-switched to available model: ${this.currentLLMModel}`);

          // Notify in chat
          this.chatHistory.push({
            message: "System",
            response: `ℹ️ Model '${modelName}' not found. Automatically switched to '${this.currentLLMModel}'.`,
            timestamp: new Date(),
            isCommand: true
          });

          return true; // Return true since we've fixed the issue by switching
        }
      }

      return modelExists;
    } catch (error: any) {
      console.error('Error checking if model exists:', error);
      return false;
    }
  }

  // Method to copy code to terminal input
  sendCodeToTerminal(code: string): void {
    // Update the terminal command input
    this.currentCommand = this.transformCodeForDisplay(code);

    // Focus the terminal input
    this.focusTerminalInput();

    // Show a brief notification
    const notification = document.createElement('div');
    notification.className = 'copy-notification';
    notification.textContent = 'Copied to terminal';
    document.body.appendChild(notification);

    // Animate and remove notification
    setTimeout(() => {
      notification.classList.add('show');
      setTimeout(() => {
        notification.classList.remove('show');
        setTimeout(() => {
          document.body.removeChild(notification);
        }, 300);
      }, 1200);
    }, 10);

    // Toggle to the terminal panel if we're on mobile
    if (window.innerWidth < 768) {
      this.isAIPanelVisible = false;
    }
  }

  // Method to execute code directly
  executeCodeDirectly(code: string): void {
    // Set the current command
    this.currentCommand = this.transformCodeForDisplay(code);

    // Create a fake keyboard event to simulate pressing Enter
    const event = new KeyboardEvent('keydown', {
      key: 'Enter',
      code: 'Enter',
      keyCode: 13,
      which: 13,
      bubbles: true
    });

    // Execute the command
    this.executeCommand(event);

    // Toggle to the terminal panel if we're on mobile
    if (window.innerWidth < 768) {
      this.isAIPanelVisible = false;
    }
  }

  // Method to copy question back to input and send it
  copyQuestionToInput(question: string): void {
    // Set the current question
    this.currentQuestion = question;

    // Focus the input
    setTimeout(() => {
      const textarea = document.querySelector('.ai-panel .prompt-container textarea');
      if (textarea) {
        (textarea as HTMLTextAreaElement).focus();
      }

      // Create and dispatch an Enter key event to send the question
      const event = new KeyboardEvent('keydown', {
        key: 'Enter',
        code: 'Enter',
        keyCode: 13,
        which: 13,
        bubbles: true
      });

      // Send the question
      this.askAI(event);
    }, 0);

    // Show a brief notification
    const notification = document.createElement('div');
    notification.className = 'copy-notification';
    notification.textContent = 'Question copied and sent';
    document.body.appendChild(notification);

    // Animate and remove notification
    setTimeout(() => {
      notification.classList.add('show');
      setTimeout(() => {
        notification.classList.remove('show');
        setTimeout(() => {
          document.body.removeChild(notification);
        }, 300);
      }, 1200);
    }, 10);
  }

  // Method to scroll to top of a command output block
  scrollToTop(entry: CommandHistory): void {
    const element = document.querySelector(`[data-command-id="${entry.timestamp.getTime()}"]`);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }

  // Helper to extract user@host from ssh command for a nicer prompt
  extractUserHostFromSshCommand(sshCommand: string): string {
    const parts = sshCommand.trim().split(/\s+/);
    const sshIndex = parts.findIndex(part => part === 'ssh');
    if (sshIndex !== -1 && parts.length > sshIndex + 1) {
      // Find the part that looks like user@host or just host
      for (let i = sshIndex + 1; i < parts.length; i++) {
        if (parts[i].includes('@') || (!parts[i].startsWith('-') && parts[i].includes('.')) || (sshIndex + 1 === i && !parts[i].startsWith('-'))) {
          // Heuristic: if it contains @, or contains . (likely domain), or is the first arg after ssh and not an option
          return parts[i];
        }
      }
    }
    return 'remote host'; // Fallback
  }

  // Session Management Methods
  createNewSession(name?: string, setAsActive: boolean = false): string {
    const sessionId = `session-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const sessionName = name || `Terminal ${this.terminalSessions.length + 1}`;
    
    const newSession: TerminalSession = {
      id: sessionId,
      name: sessionName,
      commandHistory: [],
      currentWorkingDirectory: '~',
      isActive: setAsActive,
      gitBranch: '',
      isSshSessionActive: false,
      currentSshUserHost: null
    };

    this.terminalSessions.push(newSession);
    
    if (setAsActive || this.terminalSessions.length === 1) {
      this.switchToSession(sessionId);
    }
    
    return sessionId;
  }

  switchToSession(sessionId: string): void {
    console.log(`Switching from session ${this.activeSessionId} to ${sessionId}`);
    
    // Save current session state
    if (this.activeSessionId) {
      this.saveCurrentSessionState();
      console.log(`Saved state for session ${this.activeSessionId}:`, {
        cwd: this.currentWorkingDirectory,
        commandHistoryLength: this.commandHistory.length
      });
    }

    // Find and activate new session
    const targetSession = this.terminalSessions.find(s => s.id === sessionId);
    if (!targetSession) {
      console.error('Session not found:', sessionId);
      return;
    }

    // Mark all sessions as inactive
    this.terminalSessions.forEach(session => session.isActive = false);
    
    // Activate target session
    targetSession.isActive = true;
    this.activeSessionId = sessionId;
    
    // Restore session state
    this.restoreSessionState(targetSession);
    console.log(`Restored session ${sessionId}:`, {
      cwd: this.currentWorkingDirectory,
      commandHistoryLength: this.commandHistory.length
    });
  }

  closeSession(sessionId: string): void {
    // Prevent closing the last session
    if (this.terminalSessions.length <= 1) {
      return;
    }

    const sessionIndex = this.terminalSessions.findIndex(s => s.id === sessionId);
    if (sessionIndex === -1) {
      return;
    }

    const wasActive = this.terminalSessions[sessionIndex].isActive;
    
    // Remove the session
    this.terminalSessions.splice(sessionIndex, 1);
    
    // If we closed the active session, switch to another one
    if (wasActive) {
      // Switch to the session before the closed one, or the first one if we closed the first
      const newActiveIndex = Math.max(0, sessionIndex - 1);
      this.switchToSession(this.terminalSessions[newActiveIndex].id);
    }
  }

  renameSession(sessionId: string, newName: string): void {
    const session = this.terminalSessions.find(s => s.id === sessionId);
    if (session && newName.trim()) {
      session.name = newName.trim();
    }
  }

  private saveCurrentSessionState(): void {
    const activeSession = this.terminalSessions.find(s => s.id === this.activeSessionId);
    if (activeSession) {
      activeSession.commandHistory = [...this.commandHistory];
      activeSession.currentWorkingDirectory = this.currentWorkingDirectory;
      activeSession.gitBranch = this.gitBranch;
      activeSession.isSshSessionActive = this.isSshSessionActive;
      activeSession.currentSshUserHost = this.currentSshUserHost;
    }
  }

  private restoreSessionState(session: TerminalSession): void {
    this.commandHistory = [...session.commandHistory];
    this.currentWorkingDirectory = session.currentWorkingDirectory;
    this.gitBranch = session.gitBranch;
    this.isSshSessionActive = session.isSshSessionActive;
    this.currentSshUserHost = session.currentSshUserHost;
    
    // Reset UI state
    this.currentCommand = '';
    this.commandHistoryIndex = -1;
    this.isProcessing = false;
    this.showSuggestions = false;
    
    // Update directory display
    this.getCurrentDirectory();
  }

  getActiveSession(): TerminalSession | undefined {
    return this.terminalSessions.find(s => s.id === this.activeSessionId);
  }

  // Event handlers for tab renaming
  startRenaming(event: Event): void {
    const target = event.target as HTMLElement;
    if (target) {
      target.contentEditable = 'true';
      target.focus();
    }
  }

  finishRenaming(event: Event, session: TerminalSession): void {
    const target = event.target as HTMLElement;
    if (target) {
      target.contentEditable = 'false';
      const newName = target.textContent?.trim() || session.name;
      this.renameSession(session.id, newName);
      // Restore the display text in case textContent was modified
      target.textContent = session.name;
    }
  }

  handleEnterKey(event: Event): void {
    const keyboardEvent = event as KeyboardEvent;
    const target = keyboardEvent.target as HTMLElement;
    if (target) {
      target.blur();
      keyboardEvent.preventDefault();
    }
  }
}
