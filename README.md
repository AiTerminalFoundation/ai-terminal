# AI Terminal

A Rust-based terminal application with integrated AI capabilities.
<img width="1207" alt="image" src="https://github.com/user-attachments/assets/703832ae-360d-4bc5-9d8b-339a282d05ff" />


## Features

- Modern UI built with Tauri, using html, css and Typescript
- Integrated AI assistance
- Cross-platform support for macOS and Linux

## Requirements

- Rust 1.72 or newer
- Cargo (Rust's package manager)
- For Linux builds: GTK3 development libraries

## Development Setup

1. Clone the repository:
   ```
   git clone https://github.com/your-username/ai-terminal.git
   cd ai-terminal
   ```

2. Build and run the project:
   ```
   cd ai-terminal
   cargo tauri dev
   ```

## Quick Guide to Using Ollama to Download `macsdeve/BetterBash3` Model

### Linux

1. **Install Ollama**

Open your terminal and run:

```bash
curl -fsSL https://ollama.com/install.sh | sh
```

2. **Download the Model**

Run the following command:

```bash
ollama pull macsdeve/BetterBash3
```

### macOS

1. **Download Ollama**

- Visit [Ollama download page](https://ollama.com/download/mac).
- Click **Download for macOS**.

2. **Install Ollama**

- Open the downloaded `.zip` file from your `Downloads` folder.
- Drag the `Ollama.app` into your `Applications` folder.
- Open `Ollama.app` and follow any prompts.

3. **Download the Model**

Open Terminal and execute:

```bash
ollama pull macsdeve/BetterBash3
```

## Additional Notes

- Make sure you have enough disk space.
- See more details on the [official Ollama website](https://ollama.com).





## License

[MIT License](LICENSE)
