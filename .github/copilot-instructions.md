# GitHub Copilot Instructions for AI Terminal

## Project Description

AI Terminal is a cross-platform terminal application built with Tauri and Angular, featuring integrated AI capabilities for natural language command interpretation. The application provides a modern desktop experience that combines traditional terminal functionality with an AI assistant powered by Ollama.

Key features include:
- Natural language command interpretation
- Integrated AI assistant for terminal commands
- Command history and auto-completion
- Modern, responsive UI built with Angular
- Cross-platform native application using Tauri
- Local AI processing with Ollama

## Technologies and Stack

### Frontend Technologies
- **Angular 19.x**: Component-based web framework
  - Use TypeScript for all component and service implementations
  - Follow Angular style guide and best practices
  - Leverage RxJS for reactive programming patterns
- **TypeScript 5.8.x**: Strongly typed JavaScript
  - Use strict type checking
  - Define interfaces for data models
  - Avoid `any` types when possible
- **RxJS 7.8.x**: Reactive Extensions for JavaScript
  - Use Observables for asynchronous operations
  - Properly unsubscribe from subscriptions to prevent memory leaks

### Backend Technologies
- **Tauri 2.x**: Rust-based desktop application framework
  - Provides native OS integration
  - Handles shell command execution
  - Manages system-level operations
- **Rust**: Systems programming language
  - Use safe Rust practices
  - Handle errors with Result types
  - Follow Rust naming conventions (snake_case for functions/variables)
- **Cargo**: Rust build system and package manager

### AI Integration
- **Ollama**: Local LLM runtime
  - Runs AI models locally for privacy
  - Uses HTTP API for communication
- **macsdeve/BetterBash3**: Specialized terminal command model
  - Provides natural language to command translation
  - Offers context-aware terminal assistance

### Build and Development Tools
- **Node.js 18+**: JavaScript runtime
- **npm**: Package management and script running
- **Angular CLI 19.x**: Development server and build tools
- **Tauri CLI 2.x**: Application bundling and development

## Platform Support

The application supports multiple platforms with native builds:

### macOS
- Native macOS application bundle (.app)
- Available via Homebrew Cask
- Supports macOS-specific features (private API access)
- Installation: `brew tap AiTerminalFoundation/ai-terminal && brew install --cask ai-terminal`

### Windows
- Native Windows executable (.exe)
- Windows Installer package (.msi)
- Full Windows platform support

### Linux (Secondary)
- Debian package (.deb) for Debian-based distributions
- Can be built for other distributions

## Testing the Platform

### Development Environment Setup

1. **Prerequisites:**
   - Node.js 18 or later
   - Rust toolchain (rustc, cargo)
   - Ollama (optional, for AI features)

2. **Clone and Setup:**
   ```bash
   git clone https://github.com/AiTerminalFoundation/ai-terminal.git
   cd ai-terminal/ai-terminal
   npm install
   ```

3. **Run Development Server:**
   ```bash
   npm run tauri dev
   ```
   This starts both the Angular dev server and Tauri in development mode with hot-reload.

### Testing Workflow

- **Frontend Development:** `npm run start` - Run Angular dev server with proxy
- **Full Application:** `npm run tauri dev` - Run complete Tauri application
- **Build Verification:** `npm run build` - Build Angular for production
- **Production Build:** `npm run tauri build` - Create platform-specific installer

### AI Feature Testing

1. **Install Ollama:**
   - macOS: `brew install ollama`
   - Linux: `curl -fsSL https://ollama.com/install.sh | sh`
   - Windows: Download from ollama.ai

2. **Download AI Model:**
   ```bash
   ollama pull macsdeve/BetterBash3
   ```

3. **Verify AI Integration:**
   - Launch the application
   - Try natural language commands
   - Check AI assistant responses

## Code Conventions

### Angular/TypeScript
- Use Angular component decorators and lifecycle hooks
- Implement OnDestroy for cleanup in components with subscriptions
- Use dependency injection for services
- Keep components focused and single-responsibility
- Use async pipe in templates when possible

### Rust/Tauri
- Use snake_case for function and variable names
- Use PascalCase for types and structs
- Handle errors explicitly with Result and Option types
- Use Tauri's command system for frontend-backend communication
- Implement proper error handling for Tauri commands

### File Organization
- Angular components in `/ai-terminal/src/app/`
- Tauri backend code in `/ai-terminal/src-tauri/src/`
- Shared types and interfaces should be well-documented
- Keep configuration files in their conventional locations

## Important Notes for Copilot

- This is a **hybrid application**: Angular frontend + Rust backend via Tauri
- Always consider **cross-platform compatibility** (macOS and Windows primary targets)
- **AI integration** is a core feature - be mindful of Ollama API calls
- **Security**: Handle shell command execution carefully to prevent injection
- **Performance**: Keep the UI responsive; use async operations for heavy tasks
- **Error handling**: Provide meaningful error messages to users
- Test changes with `npm run tauri dev` before considering them complete
- The application runs locally - no server-side deployment needed

## Common Tasks

### Adding a new Angular component:
```bash
cd ai-terminal
ng generate component component-name
```

### Adding a Tauri command (Rust):
1. Define command in `src-tauri/src/lib.rs` or appropriate module
2. Register command in `tauri::Builder` setup
3. Invoke from Angular using `@tauri-apps/api`

### Updating dependencies:
- Frontend: `npm update` in `/ai-terminal/`
- Backend: `cargo update` in `/ai-terminal/src-tauri/`

### Debugging:
- Use browser DevTools for Angular debugging
- Use Rust logging or println! for Tauri debugging
- Check console output when running `npm run tauri dev`
