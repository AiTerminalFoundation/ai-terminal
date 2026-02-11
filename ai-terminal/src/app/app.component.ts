import {
  AfterViewInit,
  Component,
  ElementRef,
  HostListener,
  OnDestroy,
  OnInit,
  ViewChild
} from '@angular/core';
import { CommonModule } from '@angular/common';
import { invoke } from "@tauri-apps/api/core";
import { FormsModule } from '@angular/forms';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { CommandHistory } from './models/command-history.model';
import { ChatHistory } from './models/chat-history.model';
import { TerminalSession } from './models/terminal-session.model';
import { TerminalTabComponent } from './components/terminal-tab/terminal-tab.component';
import { IconComponent } from './components/icon/icon.component';
import { AiCommandService } from './services/ai-command.service';
import { AiResponseFormatService } from './services/ai-response-format.service';
import { OllamaConnectionService } from './services/ollama-connection.service';
import { TerminalSessionService } from './services/terminal-session.service';
import { buildTerminalAssistantSystemPrompt } from './constants/ai.constants';

@Component({
  selector: 'app-root',
  imports: [CommonModule, FormsModule, TerminalTabComponent, IconComponent],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css'
})
export class AppComponent implements OnInit, AfterViewInit, OnDestroy {
  // Terminal sessions
  terminalSessions: TerminalSession[] = [];
  activeSessionId: string = '';

  // Terminal properties
  commandHistory: CommandHistory[] = [];
  currentWorkingDirectory: string = '~';
  gitBranch: string = '';

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
  @ViewChild('terminalContainer') terminalContainerRef!: ElementRef<HTMLDivElement>;
  private terminal: Terminal | null = null;
  private fitAddon: FitAddon | null = null;
  private ptySessions = new Set<string>();
  private ptyBufferBySession = new Map<string, string>();
  private ptyStartupBufferBySession = new Map<string, string>();
  private ptyStartupSettledSessions = new Set<string>();
  private ptyStartupTimers = new Map<string, ReturnType<typeof setTimeout>>();
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


  // New property for useProxy
  useProxy: boolean = false;
  isSshSessionActive: boolean = false;
  currentSshUserHost: string | null = null;

  constructor(
    private sanitizer: DomSanitizer,
    private aiCommandService: AiCommandService,
    private aiResponseFormatService: AiResponseFormatService,
    private ollamaConnectionService: OllamaConnectionService,
    private terminalSessionService: TerminalSessionService
  ) { }


  // Public method to sanitize HTML content
  public sanitizeHtml(html: string): SafeHtml {
    return this.sanitizer.bypassSecurityTrustHtml(html);
  }

  async ngOnInit() {
    // Initialize first terminal session
    this.createNewSession('Terminal 1', true);

    // Clean any existing code blocks to ensure no backticks are displayed
    this.sanitizeAllCodeBlocks();

    // Test the Ollama connection
    await this.testOllamaConnection();
  }

  ngAfterViewInit(): void {
    this.initializeInteractiveTerminal();
    this.resizeInteractiveTerminal();
  }

  private initializeInteractiveTerminal(): void {
    if (!this.terminalContainerRef?.nativeElement || this.terminal) {
      return;
    }

    this.fitAddon = new FitAddon();
    this.terminal = new Terminal({
      fontFamily: '"JetBrains Mono", "Menlo", "Monaco", "Courier New", monospace',
      fontSize: 12,
      fontWeight: '300',
      fontWeightBold: '500',
      cursorBlink: true,
      cursorStyle: 'bar',
      cursorWidth: 1,
      scrollback: 5000,
      convertEol: false,
      theme: {
        background: '#0f1115',
        foreground: '#e6edf3',
        cursor: '#7aa2f7'
      }
    });
    this.terminal.loadAddon(this.fitAddon);
    this.terminal.open(this.terminalContainerRef.nativeElement);
    this.fitAddon.fit();

    this.terminal.onData((data: string) => {
      if (!this.activeSessionId) {
        return;
      }
      invoke<void>('pty_write', { sessionId: this.activeSessionId, data })
        .catch((error) => {
          console.error('Failed to write to PTY:', error);
        });
    });

    this.terminal.onResize(({ cols, rows }) => {
      if (!this.activeSessionId) {
        return;
      }
      invoke<void>('pty_resize', { sessionId: this.activeSessionId, cols, rows })
        .catch((error) => {
          console.error('Failed to resize PTY:', error);
        });
    });

    void this.registerPtyListeners();
    if (this.activeSessionId) {
      void this.ensurePtySession(this.activeSessionId);
    }
  }

  private async registerPtyListeners(): Promise<void> {
    const unlistenPtyOutput = await listen('pty_output', (event) => {
      const payload = event.payload as { sessionId: string; data: string };

      if (!this.ptyStartupSettledSessions.has(payload.sessionId)) {
        // Buffer all data during the startup phase (not written to xterm).
        const existing = this.ptyStartupBufferBySession.get(payload.sessionId) || '';
        this.ptyStartupBufferBySession.set(payload.sessionId, existing + payload.data);

        // Reset the settle timer on every chunk so we wait for a quiet period.
        const existingTimer = this.ptyStartupTimers.get(payload.sessionId);
        if (existingTimer) {
          clearTimeout(existingTimer);
        }
        this.ptyStartupTimers.set(
          payload.sessionId,
          setTimeout(() => this.settlePtyStartup(payload.sessionId), 300)
        );
        return;
      }

      // Post-startup: write directly to xterm and the replay buffer.
      const previous = this.ptyBufferBySession.get(payload.sessionId) || '';
      this.ptyBufferBySession.set(payload.sessionId, previous + payload.data);

      if (payload.sessionId === this.activeSessionId && this.terminal) {
        this.terminal.write(payload.data);
      }
    });

    const unlistenPtyExit = await listen('pty_exit', (event) => {
      const payload = event.payload as { sessionId: string; success: boolean };
      this.ptySessions.delete(payload.sessionId);
      this.ptyStartupBufferBySession.delete(payload.sessionId);
      this.ptyStartupSettledSessions.delete(payload.sessionId);
      const timer = this.ptyStartupTimers.get(payload.sessionId);
      if (timer) {
        clearTimeout(timer);
        this.ptyStartupTimers.delete(payload.sessionId);
      }
    });

    this.unlistenFunctions.push(unlistenPtyOutput, unlistenPtyExit);
  }

  /**
   * Called once the PTY startup phase is considered settled (no new output
   * for 300 ms). Discards any noisy startup data, resets xterm, and asks the
   * shell to redraw a clean prompt via Ctrl-L.
   */
  private settlePtyStartup(sessionId: string): void {
    this.ptyStartupBufferBySession.delete(sessionId);
    this.ptyStartupTimers.delete(sessionId);
    this.ptyStartupSettledSessions.add(sessionId);

    // Reset the replay buffer so tab-switching won't replay startup noise.
    this.ptyBufferBySession.set(sessionId, '');

    if (sessionId === this.activeSessionId && this.terminal) {
      this.terminal.reset();
    }

    // Send Ctrl-L (form-feed) to the shell; bash/zsh interpret this as
    // "clear screen and redraw prompt", giving us a clean terminal.
    invoke<void>('pty_write', { sessionId, data: '\x0c' }).catch(() => {});
  }

  private async ensurePtySession(sessionId: string): Promise<void> {
    if (!this.terminal || this.ptySessions.has(sessionId)) {
      return;
    }

    const cols = this.terminal.cols || 80;
    const rows = this.terminal.rows || 24;
    await invoke<void>('pty_create_session', { sessionId, cols, rows });
    this.ptySessions.add(sessionId);
    if (!this.ptyBufferBySession.has(sessionId)) {
      this.ptyBufferBySession.set(sessionId, '');
    }
    this.ptyStartupBufferBySession.set(sessionId, '');
    this.ptyStartupSettledSessions.delete(sessionId);
  }

  private renderActivePtyBuffer(): void {
    if (!this.terminal || !this.activeSessionId) {
      return;
    }

    const sessionBuffer = this.ptyBufferBySession.get(this.activeSessionId) || '';
    this.terminal.reset();
    if (sessionBuffer.length > 0) {
      this.terminal.write(sessionBuffer);
    }
  }

  private resizeInteractiveTerminal(): void {
    if (!this.terminal || !this.fitAddon) {
      return;
    }
    this.fitAddon.fit();
    const cols = this.terminal.cols || 80;
    const rows = this.terminal.rows || 24;
    if (!this.activeSessionId) {
      return;
    }
    invoke<void>('pty_resize', { sessionId: this.activeSessionId, cols, rows })
      .catch((error) => {
        console.error('Failed to resize active PTY:', error);
      });
  }

  ngOnDestroy() {
    for (const sessionId of this.ptySessions) {
      invoke<void>('pty_close_session', { sessionId }).catch(() => {
        // Ignore close failures during shutdown.
      });
    }
    // Clear any pending startup timers.
    for (const timer of this.ptyStartupTimers.values()) {
      clearTimeout(timer);
    }
    this.ptyStartupTimers.clear();
    this.terminal?.dispose();
    this.fitAddon = null;
    this.terminal = null;
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

  private scrollToBottom() {
    document.querySelectorAll('.output-area').forEach((area) => {
      const outputArea = area as HTMLElement;
      outputArea.scrollTop = outputArea.scrollHeight;
    });
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
      this.resizeInteractiveTerminal();
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
      this.resizeInteractiveTerminal();
    }
  }

  @HostListener('document:mouseup')
  onMouseUp() {
    this.isResizing = false;
    this.resizeInteractiveTerminal();
  }

  @HostListener('document:touchend')
  onTouchEnd() {
    this.isResizing = false;
    this.resizeInteractiveTerminal();
  }

  @HostListener('window:resize')
  onWindowResize() {
    this.resizeInteractiveTerminal();
  }

  // Handle key presses globally
  @HostListener('document:keydown', ['$event'])
  handleKeyboardEvent(_event: KeyboardEvent) {
    // Interactive mode handles keys directly through xterm onData.
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
  // Includes chat history and last 4 terminal commands for better context
  async callOllamaDirectly(question: string, model: string): Promise<string> {
    try {
      // Get the current operating system
      const os = navigator.platform.toLowerCase().includes('mac') ?
        'macOS' : 'Linux';

      const systemPrompt = buildTerminalAssistantSystemPrompt(os);

      // Build messages array for /api/chat (includes conversation context)
      const messages: { role: string; content: string }[] = [
        { role: 'system', content: systemPrompt }
      ];

      // Add previous chat history (exclude the current pending "Thinking..." entry)
      const completedChatHistory = this.chatHistory.slice(0, -1);
      for (const entry of completedChatHistory) {
        if (entry.response && entry.response !== 'Thinking...') {
          messages.push({ role: 'user', content: entry.message });
          messages.push({ role: 'assistant', content: entry.response });
        }
      }

      // Get current folder (try backend for fresher value, fallback to session state)
      let currentFolder = this.currentWorkingDirectory || '~';
      if (this.activeSessionId) {
        try {
          const cwd = await invoke<string>('get_working_directory', {
            sessionId: this.activeSessionId
          });
          if (cwd?.trim()) {
            currentFolder = cwd;
          }
        } catch {
          // Keep currentWorkingDirectory fallback
        }
      }

      // Build the current user message with context (folder, commands, question)
      const contextParts: string[] = [];
      contextParts.push(`Current folder: ${currentFolder}`);
      const lastCommands = this.commandHistory
        .filter(c => c.command?.trim())
        .slice(-4)
        .map(c => c.command.trim());
      if (lastCommands.length > 0) {
        contextParts.push(`Recent terminal commands (last ${lastCommands.length}):\n${lastCommands.map(c => `  $ ${c}`).join('\n')}`);
      }
      contextParts.push(`Current question: ${question}`);
      const userContent = contextParts.join('\n\n');
      messages.push({ role: 'user', content: userContent });

      const requestBody = {
        model: model,
        messages,
        stream: false
      };

      // Use /api/chat endpoint for proper conversation context
      const apiEndpoint = this.useProxy ? '/api/chat' : `${this.ollamaApiHost}/api/chat`;

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

      // /api/chat returns message.content (not response)
      const content = data.message?.content ?? data.response;
      if (!content) {
        console.error('Unexpected response format:', data);
        return 'Error: Unexpected response format from Ollama';
      }

      return content;
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

  // Helper method to focus the terminal textarea
  focusTerminalInput(): void {
    if (this.terminal) {
      this.terminal.focus();
      return;
    }
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

  // Method to copy code to terminal input (adds to prompt for editing, does not execute)
  sendCodeToTerminal(code: string): void {
    const command = this.transformCodeForDisplay(code);
    if (this.activeSessionId) {
      invoke<void>('pty_write', { sessionId: this.activeSessionId, data: command }).catch((error) => {
        console.error('Failed to send command to PTY:', error);
      });
    }
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
    const command = this.transformCodeForDisplay(code);
    const commandWithNewline = `${command}\n`;
    if (this.activeSessionId) {
      invoke<void>('pty_write', { sessionId: this.activeSessionId, data: commandWithNewline }).catch((error) => {
        console.error('Failed to execute command in PTY:', error);
      });
      // Track command in history for LLM context
      this.commandHistory.push({
        command,
        output: [],
        timestamp: new Date(),
        isComplete: true
      });
    }

    // Toggle to the terminal panel if we're on mobile
    if (window.innerWidth < 768) {
      this.isAIPanelVisible = false;
    }
  }

  toggleAIPanel(): void {
    this.isAIPanelVisible = !this.isAIPanelVisible;
    if (this.terminal && this.fitAddon) {
      setTimeout(() => this.fitAddon!.fit(), 0);
    }
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

    if (this.terminal) {
      void this.ensurePtySession(sessionId);
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
    if (this.terminal) {
      void this.ensurePtySession(sessionId).then(() => {
        this.renderActivePtyBuffer();
        this.resizeInteractiveTerminal();
      }).catch((error) => {
        console.error(`Failed to initialize PTY for ${sessionId}:`, error);
      });
    }
  }

  closeSession(sessionId: string): void {
    const { sessions, nextActiveSessionId } = this.terminalSessionService.closeSession(
      this.terminalSessions,
      sessionId
    );

    const sessionWasRemoved = sessions.length < this.terminalSessions.length;
    this.terminalSessions = sessions;

    if (sessionWasRemoved) {
      void invoke<void>('pty_close_session', { sessionId }).catch((error) => {
        console.error(`Failed to close PTY session ${sessionId}:`, error);
      });
      this.ptySessions.delete(sessionId);
      this.ptyBufferBySession.delete(sessionId);
      this.ptyStartupBufferBySession.delete(sessionId);
      this.ptyStartupSettledSessions.delete(sessionId);
    }

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
  }

  trackBySessionId(_: number, session: TerminalSession): string {
    return session.id;
  }

  trackByChatEntry(index: number, entry: ChatHistory): string {
    return `${entry.timestamp.getTime()}-${index}`;
  }

  trackByResponseSegment(index: number, segment: string): string {
    return `${index}-${segment}`;
  }
}
