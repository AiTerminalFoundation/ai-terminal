# AI Terminal

A terminal application with an integrated AI assistant.

## Features

- Terminal emulator with command execution
- Command status tracking (success/failure)
- Integrated AI assistant panel
- Resizable panels
- Scrollable output

## Installation

### macOS

1. Copy the `AITerminal.app` to your Applications folder
2. Right-click on the app and select "Open" (this is required the first time to bypass Gatekeeper)
3. If you get a security warning, go to System Preferences > Security & Privacy and click "Open Anyway"

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/ai-terminal.git
cd ai-terminal

# Build the release version
cargo build --release

# The binary will be available at target/release/ai-terminal
```

## Usage

- Type commands in the terminal panel (left side)
- Press Enter to execute commands
- Command output will be displayed with appropriate status colors
- Use Alt+Left/Right to resize panels
- Use Alt+Up/Down or PageUp/PageDown to scroll through output
- Click on a panel to focus it
- Drag the divider between panels to resize them

## Keyboard Shortcuts

- `Enter`: Execute command
- `Alt+Left/Right`: Resize panels
- `Alt+Up/Down`: Scroll through output
- `PageUp/PageDown`: Scroll through output
- `Esc`: Exit the application

## License

MIT 