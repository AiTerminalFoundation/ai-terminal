import {
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
import { UnlistenFn } from '@tauri-apps/api/event';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { CommandHistory } from './models/command-history.model';
import { ChatHistory } from './models/chat-history.model';
import { TerminalSession } from './models/terminal-session.model';
import { TerminalTabComponent } from './components/terminal-tab/terminal-tab.component';
import { AiCommandService } from './services/ai-command.service';
import { AiResponseFormatService } from './services/ai-response-format.service';
import { OllamaConnectionService } from './services/ollama-connection.service';
import { TerminalEventListenerService } from './services/terminal-event-listener.service';
import { TerminalOutputService } from './services/terminal-output.service';
import { TerminalSessionService } from './services/terminal-session.service';
import { buildTerminalAssistantSystemPrompt } from './constants/ai.constants';
import {
  COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER,
  SSH_NEEDS_PASSWORD_MARKER
} from './constants/ssh.constants';

@Component({
  selector: 'app-root',
  imports: [CommonModule, FormsModule, TerminalTabComponent],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css'
})
export class AppComponent implements OnInit, OnDestroy {
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
  private readonly minAutocompleteInputLength = 2;
  private autocompleteDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  private autocompleteRequestToken: number = 0;

  // History search properties
  isHistorySearchActive: boolean = false;
  historySearchQuery: string = '';
  historySearchResults: { command: string, index: number, timestamp: Date }[] = [];
  selectedHistoryIndex: number = 0;

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
  @ViewChild('autocompleteContainer') autocompleteContainer!: ElementRef;
  private _shouldScroll = false;
  private scrollFramePending = false;
  get shouldScroll(): boolean {
    return this._shouldScroll;
  }

  set shouldScroll(value: boolean) {
    this._shouldScroll = value;
    if (value) {
      this.scheduleScrollToBottom();
    }
  }


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

  // Commit popup
  showCommitPopup: boolean = false;
  commitMessage: string = '';

  constructor(
    private sanitizer: DomSanitizer,
    private ngZone: NgZone,
    private elRef: ElementRef,
    private aiCommandService: AiCommandService,
    private aiResponseFormatService: AiResponseFormatService,
    private ollamaConnectionService: OllamaConnectionService,
    private terminalEventListenerService: TerminalEventListenerService,
    private terminalOutputService: TerminalOutputService,
    private terminalSessionService: TerminalSessionService
  ) { }

  getPlaceholder(): string {
    if (this.isHistorySearchActive) {
      return 'Type to search command history...';
    }
    if (this.isSudoPasswordPrompt) {
      return 'Sudo Password:';
    }
    if (this.isSSHPasswordPrompt) {
      return 'SSH Password:';
    }
    return '';
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
      const unlisteners = await this.terminalEventListenerService.registerListeners({
        onCommandOutput: async (outputLine: string) => {
          this.ngZone.run(() => {
            if (this.commandHistory.length > 0) {
              const currentCmdEntry = this.commandHistory[this.commandHistory.length - 1];

              const { lineToDisplay, newExpectingSshEchoState } = this.terminalOutputService.cleanOutputLine(
                outputLine,
                currentCmdEntry.command,
                this.isSshSessionActive,
                currentCmdEntry.expectingSshEcho || false
              );
              currentCmdEntry.expectingSshEcho = newExpectingSshEchoState;

              if (lineToDisplay === null) {
                return;
              }

              if (!currentCmdEntry.isStreaming) {
                currentCmdEntry.isStreaming = true;
                if (
                  currentCmdEntry.output.length === 1 &&
                  (currentCmdEntry.output[0] === "Processing..." ||
                    currentCmdEntry.output[0] === "Processing sudo command..." ||
                    currentCmdEntry.output[0] === "Command started. Output will stream in real-time." ||
                    currentCmdEntry.output[0] === "Sudo command started. Output will stream in real-time." ||
                    currentCmdEntry.output[0] === "Command sent to active SSH session.")
                ) {
                  currentCmdEntry.output = [];
                }
              }

              const skipLine =
                lineToDisplay === "Command started. Output will stream in real-time." ||
                lineToDisplay === "Sudo command started. Output will stream in real-time." ||
                (currentCmdEntry.command.startsWith('sudo ') && outputLine.includes("[sudo] password for"));

              if (!skipLine) {
                currentCmdEntry.output.push(lineToDisplay);
                this.shouldScroll = true;
              }
            }
          });
        },
        onCommandError: async (payload: string) => {
          this.ngZone.run(() => {
            if (this.commandHistory.length > 0) {
              const currentCmdEntry = this.commandHistory[this.commandHistory.length - 1];
              currentCmdEntry.output.push(payload);
              this.shouldScroll = true;
            }
          });
        },
        onCommandEnd: async (payload: string) => {
          await this.ngZone.run(async () => {
            if (this.commandHistory.length > 0) {
              const currentCmdEntry = this.commandHistory[this.commandHistory.length - 1];
              currentCmdEntry.isComplete = true;
              currentCmdEntry.isStreaming = false;

              currentCmdEntry.success = payload === "Command completed successfully.";

              // Refresh local prompt metadata after every command, so tab title and git branch
              // stay in sync even when commands affect cwd/branch indirectly.
              if (!this.isSshSessionActive) {
                await this.getCurrentDirectory();
              }

              this.isProcessing = false;
              this.shouldScroll = true;
            }
          });
        },
        onCommandForwardedToSsh: async () => {
          this.ngZone.run(() => {
            this.isProcessing = false;
          });
        },
        onSshPreExecPasswordRequest: async (originalCommandFromEvent: string) => {
          this.ngZone.run(() => {
            const sshPromptEntry: CommandHistory = {
              command: originalCommandFromEvent,
              output: [`SSH Password for ${this.extractUserHostFromSshCommand(originalCommandFromEvent)}:`],
              timestamp: new Date(),
              isComplete: false,
              isStreaming: false
            };
            this.commandHistory.push(sshPromptEntry);

            this.originalSSHCommand = originalCommandFromEvent;
            this.isSSHPasswordPrompt = true;
            this.passwordValue = '';
            this.displayValue = '';
            this.currentCommand = '';

            this.shouldScroll = true;
            this.isProcessing = false;
            this.focusTerminalInput();
          });
        },
        onRemoteDirectoryUpdated: async (newRemotePath: string) => {
          this.ngZone.run(() => {
            if (this.isSshSessionActive) {
              if (this.currentSshUserHost) {
                this.currentWorkingDirectory = `${this.currentSshUserHost}:${newRemotePath}`;
              } else {
                this.currentWorkingDirectory = newRemotePath;
              }
              this.gitBranch = '';
              this.syncActiveSessionState();
            }
          });
        },
        onSshSessionStarted: async () => {
          await this.ngZone.run(async () => {
            this.isSshSessionActive = true;
            await this.getCurrentDirectory();
            this.gitBranch = '';
            this.isProcessing = false;
          });
        },
        onSshSessionEnded: async () => {
          await this.ngZone.run(async () => {
            this.isSshSessionActive = false;
            this.currentSshUserHost = null;
            await this.getCurrentDirectory();
          });
        }
      });

      this.unlistenFunctions.push(...unlisteners);
    } catch (error) {
      console.error('Failed to set up event listeners:', error);
    }
  }

  ngOnDestroy() {
    this.clearAutocompleteDebounce();
    // Clean up all event listeners
    for (const unlisten of this.unlistenFunctions) {
      unlisten();
    }
  }

  private scheduleScrollToBottom() {
    if (!this.shouldScroll || this.scrollFramePending) {
      return;
    }

    this.scrollFramePending = true;
    requestAnimationFrame(() => {
      this.scrollToBottom();
      this.shouldScroll = false;
      this.scrollFramePending = false;
    });
  }

  scrollToBottom() {
    try {
      if (!this.outputAreaRef?.nativeElement) {
        return;
      }

      const outputArea = this.outputAreaRef.nativeElement;
      // Force a reflow to ensure the content height is updated
      void outputArea.offsetHeight;
      // Scroll to the maximum possible position
      outputArea.scrollTop = outputArea.scrollHeight;
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

      this.syncActiveSessionState();
    } catch (error) {
      console.error('Failed to get current directory:', error);
      this.currentWorkingDirectory = this.isSshSessionActive ? "remote:error" : "local:error";
      this.gitBranch = ''; // Clear git branch on error too
      this.syncActiveSessionState();
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
    // Handle ESC key to close popups or cancel actions
    if (event.key === 'Escape') {
      if (this.isHistorySearchActive) {
        this.exitHistorySearch(false);
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (this.showSuggestions) {
        this.showSuggestions = false;
        event.preventDefault();
        event.stopPropagation();
        this.focusTerminalInput();
        return;
      }
    }

    // Handle Ctrl+R to activate history search
    if (event.ctrlKey && event.key === 'r' && !this.isProcessing) {
      event.preventDefault();
      event.stopPropagation();
      this.activateHistorySearch();
      return;
    }

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
    const requestToken = ++this.autocompleteRequestToken;
    try {
      const trimmedCommand = this.currentCommand.trim();
      const isCdCommand = trimmedCommand === 'cd' || trimmedCommand.startsWith('cd ');

      // Don't show suggestions for empty input unless it's a cd command with no args
      if (trimmedCommand.length === 0 && !isCdCommand) {
        this.autocompleteSuggestions = [];
        this.showSuggestions = false;
        return;
      }

      // Avoid backend autocomplete calls for very short inputs.
      // Keep cd-related autocomplete available for directory navigation.
      if (!isCdCommand && trimmedCommand.length < this.minAutocompleteInputLength) {
        this.autocompleteSuggestions = [];
        this.showSuggestions = false;
        return;
      }

      // Get autocomplete suggestions from backend
      this.autocompleteSuggestions = await invoke<string[]>("autocomplete", {
        input: this.currentCommand,
        sessionId: this.activeSessionId
      });

      // Ignore stale responses from older requests.
      if (requestToken !== this.autocompleteRequestToken) {
        return;
      }

      this.showSuggestions = this.autocompleteSuggestions.length > 0;

      // Reset selection index
      this.selectedSuggestionIndex = -1;
    } catch (error) {
      // Ignore stale errors from older requests.
      if (requestToken !== this.autocompleteRequestToken) {
        return;
      }
      console.error('Failed to get autocomplete suggestions:', error);
      this.autocompleteSuggestions = [];
      this.showSuggestions = false;
    }
  }

  private clearAutocompleteDebounce(): void {
    if (this.autocompleteDebounceTimer) {
      clearTimeout(this.autocompleteDebounceTimer);
      this.autocompleteDebounceTimer = null;
    }
  }

  private cancelPendingAutocomplete(): void {
    this.clearAutocompleteDebounce();
    // Invalidate in-flight async responses from previous input states.
    this.autocompleteRequestToken++;
  }

  private scheduleAutocomplete(): void {
    this.clearAutocompleteDebounce();
    this.autocompleteDebounceTimer = setTimeout(() => {
      this.autocompleteDebounceTimer = null;
      this.requestAutocomplete();
    }, 200);
  }

  applySuggestion(suggestion: string): void {
    this.cancelPendingAutocomplete();
    const parts = this.currentCommand.trim().split(' ');

    if (parts.length > 1 || parts[0] === 'cd') {
      const command = parts[0];
      this.currentCommand = `${command} ${suggestion}`;
    } else {
      this.currentCommand = suggestion;
    }

    // Hide suggestions
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
    // Handle history search mode
    if (this.isHistorySearchActive) {
      // Handle special keys in history search mode
      if (event.key === 'Enter') {
        this.exitHistorySearch(true);
        event.preventDefault();
        return;
      }

      if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
        event.preventDefault();
        if (event.key === 'ArrowUp' && this.selectedHistoryIndex > 0) {
          this.selectedHistoryIndex--;
          this.updateHistorySearchDisplay();
        } else if (event.key === 'ArrowDown' && this.selectedHistoryIndex < this.historySearchResults.length - 1) {
          this.selectedHistoryIndex++;
          this.updateHistorySearchDisplay();
        }
        return;
      }

      // Handle character input for search
      if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey) {
        event.preventDefault();
        this.historySearchQuery += event.key;
        this.selectedHistoryIndex = 0;
        this.performHistorySearch(this.historySearchQuery);
        return;
      }

      // Handle backspace
      if (event.key === 'Backspace') {
        event.preventDefault();
        if (this.historySearchQuery.length > 0) {
          this.historySearchQuery = this.historySearchQuery.slice(0, -1);
          this.selectedHistoryIndex = 0;
          this.performHistorySearch(this.historySearchQuery);
        }
        return;
      }

      // For any other special key, prevent default
      event.preventDefault();
      return;
    }

    // Hide suggestions when pressing Esc
    if (event.key === 'Escape') {
      this.cancelPendingAutocomplete();
      this.showSuggestions = false;
      event.preventDefault();
      return;
    }

    // Handle arrow keys for command history or suggestion navigation
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      if (this.showSuggestions && this.autocompleteSuggestions.length > 0) {
        event.preventDefault();
        this.navigateToSuggestion(event.key === 'ArrowUp' ? 'up' : 'down');
      } else {
        event.preventDefault();
        this.navigateCommandHistory(event.key === 'ArrowUp' ? 'up' : 'down');
      }
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

    // Handle Tab for autocomplete
    if (event.key === 'Tab') {
      event.preventDefault();

      if (this.showSuggestions && this.autocompleteSuggestions.length > 0) {
        // If suggestions are already showing, navigate them
        this.navigateToSuggestion('down');
      } else {
        // Otherwise, request new suggestions
        await this.requestAutocomplete();
      }
      return;
    }

    // Handle Enter key for command execution
    if (event.key === 'Enter') {
      // If suggestions are visible and one is selected, apply it and prevent execution
      if (this.showSuggestions && this.selectedSuggestionIndex >= 0) {
        event.preventDefault();
        this.applySuggestion(this.autocompleteSuggestions[this.selectedSuggestionIndex]);
        this.showSuggestions = false;
        this.focusTerminalInput();
        return;
      }

      // Don't hide suggestions if a suggestion is selected (global handler will handle this case)
      if (!(this.showSuggestions && this.selectedSuggestionIndex >= 0)) {
        this.showSuggestions = false;
      }
    }

    // Execute command on Enter - only if no suggestions are visible or selected
    if (event.key === 'Enter' && !event.shiftKey && this.currentCommand.trim()) {
      this.cancelPendingAutocomplete();
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
            if (result === SSH_NEEDS_PASSWORD_MARKER) {
              // The 'ssh_pre_exec_password_request' event listener will handle:
              // - Creating the CommandHistory entry with the password prompt.
              // - Setting up isSSHPasswordPrompt, originalSSHCommand, etc.
              // - Setting isProcessing = false to allow password input.
              // So, no CommandHistory entry is created here for this specific case.
              // isProcessing will be set to false by the listener.
            } else if (result === COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER) {
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

              // If the command isn't one that typically streams (like an immediate error message),
              // or if it's a success that doesn't stream (less common for SSH connect),
              // isProcessing should be false. The command_end event is the primary way to set this.
              // For now, optimistically set to false if no streaming indicators.
              if (!result || (!result.includes("Output will stream") && !result.includes(COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER))) {
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
        if (result === COMMAND_FORWARDED_TO_ACTIVE_SSH_MARKER) {
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

  // Add a new method to parse commands from AI responses
  parseCommandFromResponse(response: string): { command: string, fullText: string }[] {
    return this.aiResponseFormatService.parseCommandFromResponse(response);
  }

  // Extract code blocks from response text
  extractCodeBlocks(text: string): { formattedText: string, codeBlocks: { code: string, language: string }[] } {
    return this.aiResponseFormatService.extractCodeBlocks(text);
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
    return this.aiResponseFormatService.isSimpleCommand(code);
  }

  // Helper method to directly call Ollama API from frontend
  async callOllamaDirectly(question: string, model: string): Promise<string> {
    try {
      // Get the current operating system
      const os = navigator.platform.toLowerCase().includes('mac') ?
        'macOS' : 'Linux';

      const systemPrompt = buildTerminalAssistantSystemPrompt(os);

      // Combine the system prompt with the user's question
      const combinedPrompt = `${systemPrompt}\n\nUser: ${question}`;

      const requestBody = {
        model: model,
        prompt: combinedPrompt,
        stream: false
      };

      // Use relative path with proxy instead of absolute URL
      const apiEndpoint = this.useProxy ? '/api/generate' : `${this.ollamaApiHost}/api/generate`;

      // Call Ollama directly
      const response = await fetch(apiEndpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(requestBody)
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Ollama API error: ${response.status} - ${errorText}`);
      }

      const data = await response.json();

      if (!data.response) {
        console.error('Unexpected response format:', data);
        return 'Error: Unexpected response format from Ollama';
      }

      return data.response;
    } catch (error: any) {

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

  // Helper method to determine if a chat history entry is a command response
  isCommandResponse(entry: ChatHistory): boolean {
    return !!entry.isCommand;
  }

  // Code specific functions
  isCodeBlockPlaceholder(text: string): boolean {
    return this.aiResponseFormatService.isCodeBlockPlaceholder(text);
  }

  getCodeBlockIndex(placeholder: string): number {
    return this.aiResponseFormatService.getCodeBlockIndex(placeholder);
  }

  // Handle AI commands starting with /
  async handleAICommand(command: string): Promise<string> {
    return this.aiCommandService.handleAICommand(command, {
      currentLLMModel: this.currentLLMModel,
      ollamaApiHost: this.ollamaApiHost,
      setCurrentLLMModel: (model: string) => {
        this.currentLLMModel = model;
      },
      setOllamaApiHost: (host: string) => {
        this.ollamaApiHost = host;
      },
      clearChatHistory: () => {
        this.chatHistory = [];
      },
      testOllamaConnection: () => {
        void this.testOllamaConnection();
      },
      retryOllamaConnection: async () => this.retryOllamaConnection()
    });
  }

  // Method to get command explanation from code block
  getCommandExplanation(code: string): string | null {
    return this.aiResponseFormatService.getCommandExplanation(code);
  }

  // Update transformCodeForDisplay to handle explanations
  transformCodeForDisplay(code: string): string {
    return this.aiResponseFormatService.transformCodeForDisplay(code);
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

    // Skip input handling during history search mode since executeCommand handles it
    if (this.isHistorySearchActive) {
      return;
    }

    // When in password mode, don't do autocomplete
    if (this.isSudoPasswordPrompt || this.isSSHPasswordPrompt) {
      return;
    }

    // When not processing, fetch suggestions
    if (!this.isProcessing) {
      this.scheduleAutocomplete();
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

    const numSuggestions = this.autocompleteSuggestions.length;

    if (direction === 'down') {
      this.selectedSuggestionIndex = (this.selectedSuggestionIndex + 1) % numSuggestions;
    } else { // direction === 'up'
      if (this.selectedSuggestionIndex <= 0) { // Handles -1 (none selected) and 0 (first selected)
        this.selectedSuggestionIndex = numSuggestions - 1;
      } else {
        this.selectedSuggestionIndex = this.selectedSuggestionIndex - 1;
      }
    }

    setTimeout(() => this.scrollSuggestionIntoView(), 0);
  }

  private scrollSuggestionIntoView(): void {
    if (this.autocompleteContainer?.nativeElement) {
      const selectedElement = this.autocompleteContainer.nativeElement.querySelector('.autocomplete-item.selected');
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest' });
      }
    }
  }

  // Navigate through command history with arrow keys
  navigateCommandHistory(direction: 'up' | 'down'): void {
    const history = this.commandHistory.map(h => h.command);
    if (history.length === 0) {
      return;
    }

    if (direction === 'up') {
      // If not navigating history yet, start from the last command
      if (this.commandHistoryIndex === -1) {
        this.commandHistoryIndex = history.length - 1;
      } else {
        // Move up in history (if not at the beginning)
        this.commandHistoryIndex = Math.max(0, this.commandHistoryIndex - 1);
      }

      // Set the current command to the historical command
      this.currentCommand = history[this.commandHistoryIndex];
    } else if (direction === 'down') {
      // If already at the end of history, do nothing
      if (this.commandHistoryIndex === -1) {
        return;
      }

      // Move down in history
      this.commandHistoryIndex++;

      // If we went past the end of history, clear input and reset index
      if (this.commandHistoryIndex >= history.length) {
        this.currentCommand = '';
        this.commandHistoryIndex = -1;
      } else {
        // Otherwise set to the command at current index
        this.currentCommand = history[this.commandHistoryIndex];
      }
    }

    // Make sure the terminal input maintains focus
    this.focusTerminalInput();
  }

  // Activate history search mode
  activateHistorySearch(): void {
    this.isHistorySearchActive = true;
    this.historySearchQuery = '';
    this.historySearchResults = [];
    this.selectedHistoryIndex = 0;
    // Set the input to show search prompt
    this.currentCommand = "(reverse-i-search)`': ";
    this.focusTerminalInput();
  }

  // Perform fuzzy search on command history
  performHistorySearch(query: string): void {
    if (!query) {
      this.historySearchResults = [];
      this.selectedHistoryIndex = 0;
      this.updateHistorySearchDisplay();
      return;
    }

    // Search through all command history (excluding empty commands)
    const validCommands = this.commandHistory
      .filter(entry => entry.command && entry.command.trim().length > 0)
      .map(entry => entry.command.trim());

    // Remove duplicates while preserving order
    const uniqueCommands = [...new Set(validCommands)];

    this.historySearchResults = uniqueCommands
      .map((command, index) => {
        const originalEntry = this.commandHistory.find(entry => entry.command === command);
        const score = this.fuzzyMatch(command.toLowerCase(), query.toLowerCase());
        return {
          command: command,
          index: index,
          timestamp: originalEntry?.timestamp || new Date(),
          score: score
        };
      })
      .filter(result => result.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 10) // Limit to top 10 results
      .map(({ command, index, timestamp }) => ({ command, index, timestamp }));

    // Reset selection to first result
    this.selectedHistoryIndex = 0;

    // Update the display
    this.updateHistorySearchDisplay();
  }

  // Simple fuzzy matching algorithm
  fuzzyMatch(text: string, query: string): number {
    let score = 0;
    let textIndex = 0;

    for (let i = 0; i < query.length; i++) {
      const char = query[i];
      const foundIndex = text.indexOf(char, textIndex);

      if (foundIndex === -1) {
        return 0; // Character not found
      }

      // Give higher score for consecutive matches
      if (foundIndex === textIndex) {
        score += 2;
      } else {
        score += 1;
      }

      textIndex = foundIndex + 1;
    }

    // Bonus for shorter strings (more relevant)
    score += (50 / text.length);

    // Bonus for exact substring match
    if (text.includes(query)) {
      score += 10;
    }

    return score;
  }

  // Update the display with current search result
  updateHistorySearchDisplay(): void {
    const searchPrefix = "(reverse-i-search)`";
    const searchSuffix = "': ";

    if (this.historySearchResults.length > 0 &&
      this.selectedHistoryIndex >= 0 &&
      this.selectedHistoryIndex < this.historySearchResults.length) {
      const result = this.historySearchResults[this.selectedHistoryIndex];
      this.currentCommand = `${searchPrefix}${this.historySearchQuery}${searchSuffix}${result.command}`;
    } else {
      this.currentCommand = `${searchPrefix}${this.historySearchQuery}${searchSuffix}`;
    }
  }

  // Exit history search mode
  exitHistorySearch(acceptCommand: boolean = false): void {
    if (acceptCommand && this.historySearchResults.length > 0 && this.selectedHistoryIndex < this.historySearchResults.length) {
      // Set the selected command
      this.currentCommand = this.historySearchResults[this.selectedHistoryIndex].command;
    } else {
      // Clear the search prompt
      this.currentCommand = '';
    }

    this.isHistorySearchActive = false;
    this.historySearchQuery = '';
    this.historySearchResults = [];
    this.selectedHistoryIndex = 0;
  }

  // Load command history from localStorage
  loadCommandHistory(): void {
    this.commandHistory = [];
  }

  // Test the Ollama connection
  async testOllamaConnection(): Promise<void> {
    await this.ollamaConnectionService.testOllamaConnection({
      ollamaApiHost: this.ollamaApiHost,
      currentLLMModel: this.currentLLMModel,
      setCurrentLLMModel: (model: string) => {
        this.currentLLMModel = model;
      },
      addChatEntry: (entry: ChatHistory) => {
        this.chatHistory.push(entry);
      }
    });
  }

  // Method to retry Ollama connection
  async retryOllamaConnection(): Promise<void> {
    await this.ollamaConnectionService.retryOllamaConnection({
      ollamaApiHost: this.ollamaApiHost,
      currentLLMModel: this.currentLLMModel,
      setCurrentLLMModel: (model: string) => {
        this.currentLLMModel = model;
      },
      addChatEntry: (entry: ChatHistory) => {
        this.chatHistory.push(entry);
      }
    });
  }

  // Check if a specific model exists in Ollama
  async checkModelExists(modelName: string): Promise<boolean> {
    return this.ollamaConnectionService.checkModelExists(modelName, {
      ollamaApiHost: this.ollamaApiHost,
      currentLLMModel: this.currentLLMModel,
      setCurrentLLMModel: (model: string) => {
        this.currentLLMModel = model;
      },
      addChatEntry: (entry: ChatHistory) => {
        this.chatHistory.push(entry);
      }
    });
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

  // Helper to extract user@host from ssh command for a nicer prompt
  extractUserHostFromSshCommand(sshCommand: string): string {
    const parts = sshCommand.trim().split(/\\s+/);
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
    const { sessions, sessionId, shouldActivate } = this.terminalSessionService.createNewSession(
      this.terminalSessions,
      name,
      setAsActive
    );
    this.terminalSessions = sessions;

    if (shouldActivate) {
      this.switchToSession(sessionId);
    }

    return sessionId;
  }

  switchToSession(sessionId: string): void {
    // Save current session state
    if (this.activeSessionId) {
      this.saveCurrentSessionState();
    }

    const { sessions, targetSession } = this.terminalSessionService.switchToSession(
      this.terminalSessions,
      sessionId
    );
    this.terminalSessions = sessions;

    if (!targetSession) {
      return;
    }

    this.activeSessionId = sessionId;

    // Restore session state
    this.restoreSessionState(targetSession);
  }

  closeSession(sessionId: string): void {
    const { sessions, nextActiveSessionId } = this.terminalSessionService.closeSession(
      this.terminalSessions,
      sessionId
    );
    this.terminalSessions = sessions;

    if (nextActiveSessionId) {
      this.switchToSession(nextActiveSessionId);
    }
  }

  renameSession(sessionId: string, newName: string): void {
    this.terminalSessions = this.terminalSessionService.renameSession(
      this.terminalSessions,
      sessionId,
      newName
    );
  }

  private saveCurrentSessionState(): void {
    // Exit history search mode before saving state
    if (this.isHistorySearchActive) {
      this.exitHistorySearch(false);
    }

    this.terminalSessions = this.terminalSessionService.saveCurrentSessionState(
      this.terminalSessions,
      this.activeSessionId,
      {
        commandHistory: this.commandHistory,
        currentWorkingDirectory: this.currentWorkingDirectory,
        gitBranch: this.gitBranch,
        isSshSessionActive: this.isSshSessionActive,
        currentSshUserHost: this.currentSshUserHost
      }
    );
  }

  private syncActiveSessionState(): void {
    if (!this.activeSessionId) {
      return;
    }

    this.terminalSessions = this.terminalSessionService.saveCurrentSessionState(
      this.terminalSessions,
      this.activeSessionId,
      {
        commandHistory: this.commandHistory,
        currentWorkingDirectory: this.currentWorkingDirectory,
        gitBranch: this.gitBranch,
        isSshSessionActive: this.isSshSessionActive,
        currentSshUserHost: this.currentSshUserHost
      }
    );
  }

  private restoreSessionState(session: TerminalSession): void {
    const state = this.terminalSessionService.restoreSessionState(session);
    this.commandHistory = state.commandHistory;
    this.currentWorkingDirectory = state.currentWorkingDirectory;
    this.gitBranch = state.gitBranch;
    this.isSshSessionActive = state.isSshSessionActive;
    this.currentSshUserHost = state.currentSshUserHost;

    // Reset UI state
    this.currentCommand = '';
    this.commandHistoryIndex = -1;
    this.isProcessing = false;
    this.showSuggestions = false;

    // Update directory display
    this.getCurrentDirectory();
  }

  getActiveSession(): TerminalSession | undefined {
    return this.terminalSessionService.getActiveSession(this.terminalSessions, this.activeSessionId);
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
      const renamedSession = this.terminalSessions.find((s) => s.id === session.id);
      target.textContent = renamedSession?.name || session.name;
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

  trackBySessionId(_: number, session: TerminalSession): string {
    return session.id;
  }

  trackByCommandEntry(_: number, entry: CommandHistory): number {
    return entry.timestamp.getTime();
  }

  trackByOutputLine(index: number, line: string): string {
    return `${index}-${line}`;
  }

  trackByAutocompleteSuggestion(index: number, suggestion: string): string {
    return `${index}-${suggestion}`;
  }

  trackByChatEntry(index: number, entry: ChatHistory): string {
    return `${entry.timestamp.getTime()}-${index}`;
  }

  trackByResponseSegment(index: number, segment: string): string {
    return `${index}-${segment}`;
  }
}
