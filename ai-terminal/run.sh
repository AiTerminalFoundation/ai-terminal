#!/bin/bash

# Don't exit immediately on error (we want to handle errors gracefully)
set +e

# Get the directory where this script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Check if we're running in a proper terminal
if [ ! -t 1 ]; then
    # We're not in a terminal, so open Terminal.app and run the app there
    osascript <<EOF
        tell application "Terminal"
            activate
            do script "cd \"$DIR\" && \"$DIR/ai-terminal\""
        end tell
EOF
    exit 0
fi

# Create logs directory in user's home directory
LOG_DIR="$HOME/Library/Logs/AITerminal"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/launch.log"

# Log start time and script location
echo "$(date): Starting AI Terminal" > "$LOG_FILE"
echo "Script location: $0" >> "$LOG_FILE"

# Get the directory where this script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
echo "Script directory: $DIR" >> "$LOG_FILE"

# Check working directory and set it properly
echo "Current working directory: $(pwd)" >> "$LOG_FILE"
echo "Changing to application directory" >> "$LOG_FILE"
cd "$DIR"
echo "New working directory: $(pwd)" >> "$LOG_FILE"

# Print environment info
echo "TERM environment: $TERM" >> "$LOG_FILE"
echo "HOME environment: $HOME" >> "$LOG_FILE"
echo "PATH environment: $PATH" >> "$LOG_FILE"
echo "USER environment: $USER" >> "$LOG_FILE"

# Check if binary exists
if [ ! -f "./ai-terminal" ]; then
    echo "ERROR: Binary not found at $DIR/ai-terminal" >> "$LOG_FILE"
    osascript -e 'display dialog "AI Terminal binary not found. Please reinstall the application." buttons {"OK"} default button "OK" with icon stop with title "AI Terminal Error"'
    exit 1
fi

echo "Binary exists at: $DIR/ai-terminal" >> "$LOG_FILE"
echo "Binary permissions: $(ls -la "./ai-terminal")" >> "$LOG_FILE"

# Set proper permissions just to be sure
chmod +x "./ai-terminal"

# Ensure we have a proper TTY/PTY for terminal applications
# This helps with "Device not configured" errors
export TERM=xterm-256color

# Launch the actual binary with proper error handling
echo "Launching application with arguments: $@" >> "$LOG_FILE"
"./ai-terminal" "$@" 2>> "$LOG_FILE"
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo "Application exited with code $EXIT_CODE" >> "$LOG_FILE"
    
    # Check for specific device not configured error
    if grep -q "Device not configured" "$LOG_FILE"; then
        echo "Detected 'Device not configured' error. This is typically caused by terminal I/O issues." >> "$LOG_FILE"
        
        osascript -e 'display dialog "AI Terminal encountered an error accessing terminal resources. This might be because the app needs to be run from Terminal instead of by clicking the icon. Try running from Terminal with:\n\nopen -a Terminal.app '"$DIR"'/ai-terminal" buttons {"OK"} default button "OK" with icon stop with title "AI Terminal Error"'
    else
        osascript -e "display dialog \"AI Terminal crashed with error code $EXIT_CODE. See log at $LOG_FILE for details.\" buttons {\"OK\"} default button \"OK\" with icon stop with title \"AI Terminal Error\""
    fi
    exit $EXIT_CODE
fi 