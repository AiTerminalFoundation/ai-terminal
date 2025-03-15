#!/bin/bash

echo "=== AITerminal Diagnostic Tool ==="
echo "This script will help diagnose issues with the AITerminal application."
echo

# Check if the app exists
if [ ! -d "AITerminal.app" ]; then
    echo "Error: AITerminal.app not found in the current directory"
    exit 1
fi

echo "1. Checking application structure..."
ls -la AITerminal.app/Contents/
ls -la AITerminal.app/Contents/MacOS/
ls -la AITerminal.app/Contents/Resources/

echo
echo "2. Checking file permissions..."
if [ -x "AITerminal.app/Contents/MacOS/AITerminalWrapper.sh" ]; then
    echo "Wrapper script is executable: OK"
else
    echo "Wrapper script is not executable: FIXING"
    chmod +x AITerminal.app/Contents/MacOS/AITerminalWrapper.sh
fi

if [ -x "AITerminal.app/Contents/MacOS/AITerminal" ]; then
    echo "Binary is executable: OK"
else
    echo "Binary is not executable: FIXING"
    chmod +x AITerminal.app/Contents/MacOS/AITerminal
fi

echo
echo "3. Checking extended attributes..."
xattr -l AITerminal.app

echo
echo "4. Removing quarantine attribute (if present)..."
xattr -d com.apple.quarantine AITerminal.app 2>/dev/null || echo "No quarantine attribute found"

echo
echo "5. Testing wrapper script..."
echo "Running: AITerminal.app/Contents/MacOS/AITerminalWrapper.sh"
AITerminal.app/Contents/MacOS/AITerminalWrapper.sh &
WRAPPER_PID=$!
sleep 2
kill $WRAPPER_PID 2>/dev/null || echo "Process already terminated"

echo
echo "6. Rebuilding application bundle..."
echo "Copying binary to application bundle..."
cp -f target/release/ai-terminal AITerminal.app/Contents/MacOS/AITerminal
chmod +x AITerminal.app/Contents/MacOS/AITerminal

echo
echo "7. Checking Info.plist..."
plutil -lint AITerminal.app/Contents/Info.plist || echo "plutil not available, skipping plist validation"

echo
echo "Diagnostic complete. Try running the application again."
echo "If it still doesn't work, try running it from the terminal with:"
echo "open AITerminal.app"
echo "or"
echo "AITerminal.app/Contents/MacOS/AITerminalWrapper.sh" 