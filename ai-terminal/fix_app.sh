#!/bin/bash

echo "=== AITerminal Fix Tool ==="
echo "This script will fix common issues with the AITerminal application."
echo

# Check if the app exists
if [ ! -d "AITerminal.app" ]; then
    echo "Error: AITerminal.app not found in the current directory"
    exit 1
fi

echo "1. Fixing file permissions..."
chmod +x AITerminal.app/Contents/MacOS/AITerminal
chmod +x AITerminal.app/Contents/MacOS/standalone_launcher.sh
echo "File permissions fixed."

echo
echo "2. Removing quarantine attribute (if present)..."
xattr -d com.apple.quarantine AITerminal.app 2>/dev/null || echo "No quarantine attribute found"

echo
echo "3. Updating Info.plist..."
cat > AITerminal.app/Contents/Info.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>standalone_launcher.sh</string>
    <key>CFBundleIdentifier</key>
    <string>com.aiterminal.app</string>
    <key>CFBundleName</key>
    <string>AITerminal</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>LSUIElement</key>
    <false/>
    <key>NSAppleEventsUsageDescription</key>
    <string>AITerminal needs to access the terminal to execute commands.</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2023 AITerminal. All rights reserved.</string>
</dict>
</plist>
EOF
echo "Info.plist updated."

echo
echo "4. Creating standalone launcher..."
cat > AITerminal.app/Contents/MacOS/standalone_launcher.sh << 'EOF'
#!/bin/bash

# Get the directory where the script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Run the actual binary directly
"$DIR/AITerminal"

# Exit with the same code as the application
exit $?
EOF
chmod +x AITerminal.app/Contents/MacOS/standalone_launcher.sh
echo "Standalone launcher created."

echo
echo "Fix complete. Try running the application again with:"
echo "open AITerminal.app"
echo
echo "If it still doesn't work, you can run it directly with:"
echo "AITerminal.app/Contents/MacOS/AITerminal" 