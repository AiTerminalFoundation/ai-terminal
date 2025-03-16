#!/bin/bash

# Exit on error
set -e

echo "Building AI Terminal macOS App..."

# Change to the ai-terminal directory where Cargo.toml is located
cd ai-terminal

# Compile the Rust application
echo "Compiling Rust application..."
cargo build --release

# Create the app bundle structure
echo "Creating app bundle structure..."
APP_NAME="../AI Terminal.app"
rm -rf "$APP_NAME"
mkdir -p "$APP_NAME/Contents/MacOS"
mkdir -p "$APP_NAME/Contents/Resources"

# Create the launcher script
echo "Creating launcher script..."
cat > "$APP_NAME/Contents/MacOS/run.sh" << 'EOF'
#!/bin/bash

# Set the APP_BUNDLE environment variable
export APP_BUNDLE=1

# Get the directory where this script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Run the actual binary directly (no Terminal.app needed)
"$DIR/ai-terminal"

# Exit with the same status as the binary
exit $?
EOF

# Make the script executable
chmod +x "$APP_NAME/Contents/MacOS/run.sh"

# Copy the binary
echo "Copying binary..."
cp target/release/ai-terminal "$APP_NAME/Contents/MacOS/"

# Copy the Info.plist
echo "Creating Info.plist..."
cat > "$APP_NAME/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>run.sh</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.ai-terminal</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>AI Terminal</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.10</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2023 Your Company. All rights reserved.</string>
    <key>LSBackgroundOnly</key>
    <false/>
    <key>LSUIElement</key>
    <false/>
    <key>NSAppleEventsUsageDescription</key>
    <string>AI Terminal needs to access system features.</string>
</dict>
</plist>
EOF

# Create a placeholder icon if it doesn't exist
if [ ! -f "../AppIcon.icns" ]; then
    echo "Creating placeholder icon..."
    # This is just a placeholder - you should replace with a real icon
    touch "$APP_NAME/Contents/Resources/AppIcon.icns"
else
    cp "../AppIcon.icns" "$APP_NAME/Contents/Resources/"
fi

echo "App bundle created: $APP_NAME"
echo "You can now run it by double-clicking on it in Finder." 