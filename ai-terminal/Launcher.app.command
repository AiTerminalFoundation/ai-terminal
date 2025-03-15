#!/bin/bash

# This is a launcher script that opens the ai-terminal in a proper Terminal window
# This helps solve the "Device not configured" error that can happen when
# launching terminal applications directly from Finder/Dock.

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Path to the binary
BINARY_PATH="$SCRIPT_DIR/AI-Terminal.app/Contents/MacOS/ai-terminal"

# Check if binary exists
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Could not find AI Terminal binary at $BINARY_PATH"
    echo "Please make sure the application is properly installed."
    echo "Press any key to exit..."
    read -n 1
    exit 1
fi

echo "Launching AI Terminal..."
echo ""
echo "If you see 'Device not configured' errors when launching the app directly,"
echo "you can always use this launcher script instead."
echo ""

# Execute the binary with proper terminal environment
"$BINARY_PATH" "$@"

echo ""
echo "AI Terminal has exited. Press any key to close this window..."
read -n 1
