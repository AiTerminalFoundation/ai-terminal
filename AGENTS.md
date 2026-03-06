# AI Terminal - Agent Configuration

## Project Description

AI Terminal is a modern, cross-platform terminal application that combines the power of traditional command-line interfaces with integrated AI capabilities. Built with Tauri and Angular, it provides a native desktop experience with natural language command interpretation, an integrated AI assistant, and a sleek modern UI.

The application leverages Ollama for AI features, allowing users to interact with terminal commands using natural language and receive intelligent assistance for command-line tasks.

## Technologies Used

### Frontend
- **Angular 19.x**: Modern web framework for building the user interface
- **TypeScript 5.8.x**: Type-safe JavaScript for development
- **RxJS 7.8.x**: Reactive programming for handling asynchronous operations

### Backend
- **Tauri 2.x**: Rust-based framework for building native desktop applications
- **Rust**: Systems programming language providing security and performance
- **Cargo**: Rust's package manager and build system

### AI Integration
- **Ollama**: Local AI runtime for running language models
- **macsdeve/BetterBash3**: Specialized model for terminal command assistance

### Development Tools
- **Node.js 18+**: JavaScript runtime for development tooling
- **npm**: Package manager for JavaScript dependencies
- **Angular CLI 19.x**: Command-line interface for Angular development

## Platform Support

AI Terminal is designed to work across multiple platforms:

### macOS
- Full support for macOS systems
- Available via Homebrew installation
- Native macOS application bundle
- Installation: `brew tap AiTerminalFoundation/ai-terminal && brew install --cask ai-terminal`

### Windows
- Full support for Windows systems
- Native Windows executable
- Can be built and run using the standard development setup

### Linux
- Full support for Linux distributions
- Available as `.deb` package for Debian-based systems
- Can be built from source on other distributions

## Testing the Platform

### Prerequisites

Before testing, ensure you have the following installed:

1. **Node.js 18 or later**
2. **Rust and Cargo** (Rust toolchain)
3. **Ollama** (for AI features):
   - macOS: `brew install ollama`
   - Linux: `curl -fsSL https://ollama.com/install.sh | sh`
   - Or download from [ollama.ai](https://ollama.ai/)

### Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/AiTerminalFoundation/ai-terminal.git
   cd ai-terminal
   ```

2. **Navigate to the project directory:**
   ```bash
   cd ai-terminal
   ```

3. **Install dependencies:**
   ```bash
   npm install
   ```

4. **Run the application in development mode:**
   ```bash
   npm run tauri dev
   ```
   
   This command will:
   - Build the Angular frontend
   - Compile the Rust backend
   - Launch the application in development mode with hot-reload

### Setting Up AI Features

To use the AI capabilities, you need to download the AI model:

1. **Ensure Ollama is running:**
   ```bash
   ollama serve
   ```

2. **Download the BetterBash3 model:**
   ```bash
   ollama pull macsdeve/BetterBash3
   ```

### Running Tests

- **Frontend tests:** Standard Angular testing setup with Jasmine and Karma
- **Build verification:** `npm run build` to verify the Angular build
- **Development mode:** `npm run tauri dev` for interactive testing

### Building for Production

To create a production build:
```bash
npm run tauri build
```

This will create platform-specific installers in `src-tauri/target/release/bundle/`.

## Project Structure

- `/ai-terminal/` - Main application directory
  - `/src/` - Angular frontend source code
  - `/src-tauri/` - Tauri backend (Rust) source code
  - `package.json` - Node.js dependencies and scripts
  - `angular.json` - Angular configuration
  - `tauri.conf.json` - Tauri application configuration
- `/FineTuned/` - AI model fine-tuning resources
- `README.md` - Project documentation
- `requirements.txt` - Python dependencies (if applicable)

## Contributing

When working on this project:
1. Follow the existing code style and conventions
2. Test on both macOS and Windows when possible
3. Ensure the application builds successfully with `npm run tauri dev`
4. Update documentation for any new features or changes
5. Consider the impact on AI integration features
