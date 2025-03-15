#!/bin/bash

# Compile the Rust application
cargo build --release

# Create the app bundle structure
mkdir -p "AI Terminal.app/Contents/MacOS"
mkdir -p "AI Terminal.app/Contents/Resources"

# Create the run.sh script
cat > "AI Terminal.app/Contents/MacOS/run.sh" << 'EOF'
#!/bin/bash

# Set the APP_BUNDLE environment variable
export APP_BUNDLE=1

# Get the directory where this script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Run the actual binary
"$DIR/ai-terminal"

# Exit with the same status as the binary
exit $?
EOF

# Make the script executable
chmod +x "AI Terminal.app/Contents/MacOS/run.sh"

# Copy the binary
cp target/release/ai-terminal "AI Terminal.app/Contents/MacOS/"

# Copy the Info.plist
cp ai-terminal/Info.plist "AI Terminal.app/Contents/"

# If you have an icon, copy it
# cp path/to/your/AppIcon.icns "AI Terminal.app/Contents/Resources/"

echo "App bundle created: AI Terminal.app"