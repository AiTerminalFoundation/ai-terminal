import { Component, HostListener, OnInit, ViewChild, ElementRef, AfterViewChecked, OnDestroy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterOutlet } from '@angular/router';
import { invoke } from "@tauri-apps/api/core";
import { FormsModule } from '@angular/forms';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

interface CommandHistory {
  command: string;
  output: string[];
  timestamp: Date;
  isComplete: boolean;
  isStreaming?: boolean;
  success?: boolean;
}

interface ChatHistory {
  message: string;
  response: string;
  timestamp: Date;
  isCommand?: boolean; // Flag to indicate if this is a command message
  codeBlocks?: { code: string, language: string }[]; // Store extracted code blocks
}

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [CommonModule, RouterOutlet, FormsModule],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css'
})
export class AppComponent implements OnInit, AfterViewChecked, OnDestroy {
  // Terminal properties
  commandHistory: CommandHistory[] = [];
  currentCommand: string = '';
  isProcessing: boolean = false;
  currentWorkingDirectory: string = '~';
  commandHistoryIndex: number = -1; // Current position in command history navigation

  // Autocomplete properties
  autocompleteSuggestions: string[] = [];
  showSuggestions: boolean = false;
  selectedSuggestionIndex: number = -1;
  lastTabPressTime: number = 0;

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

  // Add new property for sudo password handling
  isSudoPasswordMode: boolean = false;
  sudoCommandToExecute: string = '';

  async ngOnInit() {
    // Load saved command history
    this.loadCommandHistory();

    // Get initial working directory
    this.getCurrentDirectory();

    // Clean any existing code blocks to ensure no backticks are displayed
    this.sanitizeAllCodeBlocks();

    // Test the Ollama connection
    this.testOllamaConnection();

    // Set up event listeners for command output streaming
    try {
      // Listen for command output
      const unlisten1 = await listen('command_output', (event) => {
        if (this.commandHistory.length > 0) {
          const currentCommand = this.commandHistory[this.commandHistory.length - 1];

          // Mark command as streaming
          if (!currentCommand.isStreaming) {
            currentCommand.isStreaming = true;
            // Remove the "Processing..." indicator
            if (currentCommand.output.length === 1 && currentCommand.output[0] === "Processing...") {
              currentCommand.output = [];
            }
          }

          currentCommand.output.push(event.payload as string);
          this.shouldScroll = true;
        }
      });

      // Listen for command errors
      const unlisten2 = await listen('command_error', (event) => {
        if (this.commandHistory.length > 0) {
          const currentCommand = this.commandHistory[this.commandHistory.length - 1];
          currentCommand.output.push(event.payload as string);
          this.shouldScroll = true;
        }
      });

      // Listen for command completion
      const unlisten3 = await listen('command_end', async (event) => {
        if (this.commandHistory.length > 0) {
          const currentCommand = this.commandHistory[this.commandHistory.length - 1];
          currentCommand.isComplete = true;
          currentCommand.isStreaming = false;

          // Set success flag based on the exit message
          const message = event.payload as string;
          currentCommand.success = message === "Command completed successfully.";

          // Save command history when a command completes
          this.saveCommandHistory();

          // Handle directory updates for cd commands
          const commandText = currentCommand.command.trim();
          const isCdCommand = commandText === 'cd' || commandText.startsWith('cd ');

          if (isCdCommand) {
            // For CD commands, update the directory immediately
            await this.getCurrentDirectory();
          }

          this.isProcessing = false;
          this.shouldScroll = true;
        }
      });

      this.unlistenFunctions.push(unlisten1, unlisten2, unlisten3);
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
      this.outputAreaRef.nativeElement.scrollTop = this.outputAreaRef.nativeElement.scrollHeight;
    } catch (err) {
      console.error('Error scrolling to bottom:', err);
    }
  }

  async getCurrentDirectory() {
    try {
      // Use parallel requests to get both values if we don't have homePath yet
      if (!this.homePath) {
        const [result, homePath] = await Promise.all([
          invoke<string>("get_working_directory"),
          invoke<string>("get_home_directory")
        ]);

        this.homePath = homePath;

        // Replace home directory path with ~
        if (result.startsWith(homePath)) {
          this.currentWorkingDirectory = '~' + result.substring(homePath.length);
        } else {
          this.currentWorkingDirectory = result.trim();
        }
      } else {
        // If we already have the home path, just get the current directory
        const result = await invoke<string>("get_working_directory");

        // Replace home directory path with ~
        if (result.startsWith(this.homePath)) {
          this.currentWorkingDirectory = '~' + result.substring(this.homePath.length);
        } else {
          this.currentWorkingDirectory = result.trim();
        }
      }
    } catch (error) {
      console.error('Failed to get current directory:', error);
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
    currentCommand.output.push("\n^C - Terminating process...");
    currentCommand.isComplete = true;
    currentCommand.isStreaming = false;
    currentCommand.success = false;
    this.shouldScroll = true;

    // Force immediate UI update
    await new Promise(resolve => setTimeout(resolve, 0));

    try {
      // Call the backend to terminate the command
      const result = await Promise.race([
        invoke<string>("terminate_command"),
        // Use a longer timeout for macOS since process termination can take longer
        new Promise<string>(resolve =>
          setTimeout(() => resolve("Termination timed out, but UI is responsive"), 2000)
        )
      ]);

      currentCommand.output.push(`\n${result}`);
    } catch (error) {
      console.error('Failed to terminate command:', error);
      currentCommand.output.push("\nError terminating command, but UI is responsive");
    } finally {
      // Ensure UI state is consistent
      this.isProcessing = false;
      currentCommand.isComplete = true;
      currentCommand.isStreaming = false;
      this.shouldScroll = true;
    }
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
      const suggestions = await invoke<string[]>("autocomplete", {
        input: this.currentCommand
      });

      this.autocompleteSuggestions = suggestions;

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
      // Exit sudo password mode if active
      if (this.isSudoPasswordMode) {
        this.exitSudoPasswordMode();
        return;
      }

      this.showSuggestions = false;
      event.preventDefault();
      return;
    }

    // If we're in sudo password mode, handle password entry
    if (this.isSudoPasswordMode && event.key === 'Enter') {
      event.preventDefault();

      // Get the password from the input field
      const password = this.currentCommand;

      // Reset input field and exit sudo password mode
      this.currentCommand = '';
      this.isSudoPasswordMode = false;

      // Execute the sudo command with the password
      const sudoCommand = this.sudoCommandToExecute.substring(5); // Remove 'sudo ' prefix

      // Create a secure command, handling the password safely
      const quotedPassword = password.replace(/'/g, "'\\''"); // Escape single quotes properly
      const secureCommand = `echo '${quotedPassword}' | sudo -S ${sudoCommand}`;

      // Add original command to history
      const commandEntry: CommandHistory = {
        command: this.sudoCommandToExecute,
        output: [],
        timestamp: new Date(),
        isComplete: false
      };
      this.commandHistory.push(commandEntry);
      this.shouldScroll = true;

      // Save updated command history
      this.saveCommandHistory();

      // Execute the command with the password
      this.executeCommandInternal(secureCommand);

      return;
    }

    // Handle arrow keys for command history navigation when no suggestions are visible
    if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && !this.showSuggestions) {
      // Skip history navigation in sudo password mode
      if (this.isSudoPasswordMode) {
        return;
      }

      event.preventDefault();
      this.navigateCommandHistory(event.key === 'ArrowUp' ? 'up' : 'down');
      return;
    }

    // Tab completion - show suggestions (skip in sudo password mode)
    if (event.key === 'Tab' && !this.isSudoPasswordMode) {
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

    // Skip autocomplete in sudo password mode
    if (!this.isSudoPasswordMode && event.key !== 'ArrowLeft' && event.key !== 'ArrowRight' &&
      this.currentCommand.trim().length >= 1 && !this.isProcessing) {
      this.requestAutocomplete();
    }

    // Hide suggestions when pressing Enter to execute command
    if (event.key === 'Enter') {
      // Don't hide suggestions if a suggestion is selected (global handler will handle this case)
      if (!(this.showSuggestions && this.selectedSuggestionIndex >= 0)) {
        this.showSuggestions = false;
      }
    }

    // Execute command on Enter - only if no suggestions are visible or selected
    if (event.key === 'Enter' && !event.shiftKey && this.currentCommand.trim() && !this.isSudoPasswordMode) {
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

      // Check if this is a sudo command that might need a password
      if (commandToSend.startsWith('sudo ') && commandToSend !== 'sudo -k' && !commandToSend.includes('-S')) {
        try {
          // Try a quick check to see if we need a password
          const passwordNeeded = await this.checkSudoPasswordNeeded();

          if (passwordNeeded) {
            // Enter sudo password mode instead of using a browser prompt
            this.enterSudoPasswordMode(commandToSend);
            return;
          }
        } catch (error) {
          console.error("Error checking sudo status:", error);
          // If there's any error in the check, proceed with normal command execution
        }
      }

      // If we get here, either it's not a sudo command or sudo doesn't need a password

      // Add command to history with empty output array
      const commandEntry: CommandHistory = {
        command: commandToSend,
        output: [], // Start with an empty array instead of "Processing..."
        timestamp: new Date(),
        isComplete: false
      };
      this.commandHistory.push(commandEntry);
      this.shouldScroll = true;

      // Save updated command history
      this.saveCommandHistory();

      // Clear input immediately
      this.currentCommand = '';

      // Reset command history navigation index
      this.commandHistoryIndex = -1;

      // For cd commands, update directory proactively
      const isCdCommand = commandToSend === 'cd' || commandToSend.startsWith('cd ');
      if (isCdCommand) {
        // Update directory immediately to reduce perceived lag
        // Will be refreshed again when command completes
        setTimeout(() => this.getCurrentDirectory(), 50);
      }

      // Execute the command
      this.executeCommandInternal(commandToSend);
    }
  }

  // Helper method to execute a command via Tauri
  private async executeCommandInternal(commandToSend: string): Promise<void> {
    try {
      // Find the command entry for this command (the last one added)
      const commandEntry = this.commandHistory[this.commandHistory.length - 1];

      // Execute command using Tauri
      // For non-streaming commands, the result will be returned directly
      // For streaming commands, the events will update the output
      const result = await invoke<string>("execute_command", { command: commandToSend });

      // If the result is not empty, add it to the output
      if (result && result.trim() !== "") {
        commandEntry.output.push(result);
      }

      // Note: We don't mark the command as complete here
      // The command_end event will do that for us
    } catch (error) {
      // Find the command entry
      const commandEntry = this.commandHistory[this.commandHistory.length - 1];

      if (commandEntry) {
        commandEntry.output = [`Error: ${error}`];
        commandEntry.isComplete = true;
        commandEntry.success = false; // Explicitly mark as failed
      }
      this.isProcessing = false;
    }
  }

  // Check if sudo needs a password for the current user
  private async checkSudoPasswordNeeded(): Promise<boolean> {
    try {
      // Run a test command with sudo -n which will fail if a password is needed
      // -n option ensures that sudo won't prompt for password
      await invoke<string>("execute_command", { command: "sudo -n true" });

      // If we get here, the command succeeded, no password needed
      return false;
    } catch (error) {
      // Command failed, which means sudo needs a password
      return true;
    }
  }

  // Extract code blocks from response text
  extractCodeBlocks(text: string): { formattedText: string, codeBlocks: { code: string, language: string }[] } {
    const codeBlocks: { code: string, language: string }[] = [];

    // Special handling for command responses (single line enclosed in triple backticks)
    const command = this.parseCommandFromResponse(text);
    if (command) {
      console.log("Found command:", command);
      // Create a code block for the command
      codeBlocks.push({
        code: command,
        language: 'command'
      });

      // Return only the code block placeholder
      return { formattedText: `<code-block-0></code-block-0>`, codeBlocks };
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
    let formattedText = text.replace(codeBlockRegex, (match, language, code) => {
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
      When providing terminal commands, ensure they are compatible with ${os}. 
      When asked for a command, respond with ONLY the command in this format: \`\`\`command\`\`\`
      The command should be a single line without any explanation or additional text.`;

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
          // The default model doesn't exist and we've already tried to auto-switch
          response = "Error: The model could not be found. Please check available models with /models and select one with /model [name].";
        } else {
          // Call Ollama directly
          response = await this.callOllamaDirectly(this.currentQuestion, this.currentLLMModel);

          // Check if the response contains a command we can execute
          const command = this.parseCommandFromResponse(response);
          if (command) {
            console.log("Detected command in response:", command);
            // If this is a direct shell command question, we can enhance the UI by marking it as a command
            if (this.currentQuestion.toLowerCase().includes("command") ||
              this.currentQuestion.toLowerCase().includes("how do i") ||
              this.currentQuestion.toLowerCase().startsWith("show me") ||
              this.currentQuestion.toLowerCase().startsWith("execute")) {
              chatEntry.isCommand = true;
            }
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

  // Add helper method to check for errors in command output
  hasErrors(entry: CommandHistory): boolean {
    // If success is undefined, fall back to checking for "Error:" in output
    if (entry.success === undefined) {
      return entry.output.some(line => line.startsWith('Error:'));
    }
    return !entry.success;
  }

  isRealErrorLine(line: string, commandFailed: boolean): boolean {
    // Only mark as error if:
    // 1. The command actually failed (non-zero exit)
    // 2. The line starts with "Error:"
    return commandFailed && line.startsWith('Error:');
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
/retry - Retry connection to Ollama API`;

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

  // Transform code before display to ensure no backticks or unwanted formatting
  transformCodeForDisplay(code: string): string {
    if (!code) return '';

    // First, check if the code still has backticks around it
    let cleanCode = code.trim();

    // Remove any backticks entirely from the code (at beginning, end, or middle)
    cleanCode = cleanCode.replace(/```/g, '').trim();

    // Remove any language identifiers commonly found at the beginning of code blocks
    if (cleanCode.startsWith('bash') ||
      cleanCode.startsWith('shell') ||
      cleanCode.startsWith('sh') ||
      cleanCode.startsWith('command')) {
      // Split by newline and remove the first line if it's just a language identifier
      const lines = cleanCode.split('\n');
      if (lines.length > 1 && lines[0].length < 20 && !lines[0].includes(' ')) {
        cleanCode = lines.slice(1).join('\n').trim();
      }
    }

    // For command responses, we want to preserve the entire command string
    return cleanCode;
  }

  // Additional debug function
  logCodeBlock(codeBlock: any): void {
    console.log('Code block:', codeBlock);
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

    // Skip further processing if in sudo password mode
    if (this.isSudoPasswordMode) {
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
            message: " System",
            response: "Connected to Ollama API, but no models are available. Please install models with 'ollama pull <model>'.",
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

  // Add a new method to parse commands from AI responses
  parseCommandFromResponse(response: string): string | null {
    // First, check if the response is just plain text (no formatting)
    if (!response.includes('```') && response.trim().length < 100 && !response.includes('\n')) {
      // This might be a plain command without any formatting
      return response.trim();
    }

    // Second, try to match the most common format with triple backticks
    // This handles formats like: ```ls -l``` or ```command: ls -l``` or ```bash\nls -l```
    const commandMatch = response.match(/```(?:command|bash|shell|sh)?[:\s]?\s*([^`\n]+)(?:\n|```)/);

    if (commandMatch && commandMatch[1]) {
      // Return the command without surrounding backticks and whitespace
      return commandMatch[1].trim();
    }

    // Third approach: look for the entire response being just a command in backticks
    if (response.trim().startsWith('```') && response.trim().endsWith('```')) {
      const content = response.trim().slice(3, -3).trim();
      // If the content doesn't have multiple lines and is reasonably short, treat as command
      if (!content.includes('\n') && content.length < 100) {
        return content;
      }
    }

    // If no command found in the expected format, return null
    return null;
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

  // Add methods to handle sudo password mode
  enterSudoPasswordMode(sudoCommand: string): void {
    this.isSudoPasswordMode = true;
    this.sudoCommandToExecute = sudoCommand;
    this.currentCommand = ''; // Clear the input field
    this.isProcessing = false; // Allow input

    // Add a message to command history to indicate password is required
    const passwordPromptEntry: CommandHistory = {
      command: sudoCommand,
      output: ['[sudo] password required'],
      timestamp: new Date(),
      isComplete: true,
      success: true
    };
    this.commandHistory.push(passwordPromptEntry);
    this.shouldScroll = true;
  }

  exitSudoPasswordMode(): void {
    this.isSudoPasswordMode = false;
    this.sudoCommandToExecute = '';
    this.currentCommand = '';
    this.isProcessing = false;
  }
}
