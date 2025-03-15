# AI Terminal

A simple terminal emulator built in Rust with a clean, minimalist UI.

## Features

- Single tab interface for executing terminal commands
- Command history displayed in the output area
- Full support for standard terminal commands
- Simple, intuitive interface
- Standalone application that can run outside of a regular terminal

## Requirements

- Rust and Cargo (latest stable version recommended)
- macOS 10.13 or later

## Installation

### Building from Source

1. Clone this repository:
   ```
   git clone https://github.com/yourusername/ai-terminal.git
   cd ai-terminal
   ```

2. Build the project:
   ```
   cargo build --release
   ```

3. Run the terminal directly:
   ```
   cargo run --release
   ```

### Using the Standalone Application

1. Copy the `AI-Terminal.app` to your Applications folder
2. Double-click the application to launch it
3. A new Terminal window will open with the AI Terminal interface

## Usage

- Type commands in the input area at the bottom
- Press Enter to execute commands
- Command output appears in the output area above
- Use arrow keys to navigate within the input field
- Press Esc to exit the application

## Controls

- `Enter`: Execute the current command
- `Backspace`: Delete the character before the cursor
- `Delete`: Delete the character at the cursor position
- `Left/Right Arrow Keys`: Move the cursor within the input field
- `Esc`: Exit the application

## Dependencies

- [crossterm](https://github.com/crossterm-rs/crossterm): Terminal manipulation library
- [ratatui](https://github.com/ratatui-org/ratatui): Terminal UI library

## License

MIT 