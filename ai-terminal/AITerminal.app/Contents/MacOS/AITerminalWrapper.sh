#!/bin/bash

# Enable error tracing for debugging
set -x

# Get the directory where the script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
echo "Script directory: $DIR"

# Check if the binary exists
if [ ! -f "$DIR/AITerminal" ]; then
    echo "Error: AITerminal binary not found at $DIR/AITerminal"
    exit 1
fi

# Check if the binary is executable
if [ ! -x "$DIR/AITerminal" ]; then
    echo "Error: AITerminal binary is not executable"
    chmod +x "$DIR/AITerminal"
    echo "Made AITerminal executable"
fi

# When launched as an app, we need to open a new Terminal window
# to properly handle terminal input/output
osascript <<EOF
tell application "Terminal"
    do script "'$DIR/AITerminal'"
    activate
end tell
EOF

exit 0 