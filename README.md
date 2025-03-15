# AI Terminal

Your AI mate into your favourite terminal

## Overview

AI Terminal is a powerful command-line interface application that brings AI assistance directly to your terminal. It helps you with common tasks, provides information, and enhances your terminal experience with AI capabilities.

## Features

- AI-powered command suggestions
- Natural language processing for terminal commands
- Cross-platform support (macOS, Linux, Windows)
- Lightweight and fast performance

## Installation

### macOS

#### Using DMG Installer
1. Download the latest DMG file for your architecture (ARM64 for Apple Silicon, x86_64 for Intel) from the [Releases](https://github.com/yourusername/ai-terminal/releases) page
2. Open the DMG file
3. Drag the AI Terminal app to your Applications folder
4. Run AI Terminal from your Applications folder

#### Using Install Script
```bash
curl -sSL https://raw.githubusercontent.com/yourusername/ai-terminal/main/ai-terminal/install.sh | bash
```

### Linux

#### Using Debian Package
1. Download the latest .deb file from the [Releases](https://github.com/yourusername/ai-terminal/releases) page
2. Install using:
```bash
sudo dpkg -i ai-terminal.deb
```

#### Building from Source
```bash
git clone https://github.com/yourusername/ai-terminal.git
cd ai-terminal/ai-terminal
cargo build --release
sudo cp target/release/ai-terminal /usr/local/bin/
```

### Windows

1. Download the latest Windows zip file from the [Releases](https://github.com/yourusername/ai-terminal/releases) page
2. Extract the zip file to a location of your choice
3. Run the `run-ai-terminal.bat` file

## Usage

Simply type your query or command after launching AI Terminal:

```bash
ai-terminal "What's the weather like today?"
```

Or launch the interactive mode:

```bash
ai-terminal
```

## Building from Source

### Prerequisites
- Rust and Cargo (latest stable version)
- For macOS: Xcode Command Line Tools
- For Linux: build-essential, libssl-dev, pkg-config
- For Windows: Visual Studio Build Tools

### Build Steps
```bash
git clone https://github.com/yourusername/ai-terminal.git
cd ai-terminal/ai-terminal
cargo build --release
```

The compiled binary will be available at `target/release/ai-terminal`.

## Releases

We use GitHub Actions to automatically build and release packages for macOS (ARM64 and x86_64), Linux, and Windows when a new tag is pushed. The release workflow creates:

- DMG installers for macOS (both ARM64 and x86_64 architectures)
- DEB package for Linux
- ZIP archive for Windows

## License

[MIT License](LICENSE)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
