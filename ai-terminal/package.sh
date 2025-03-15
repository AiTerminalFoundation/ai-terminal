#!/bin/bash

# Build the release version
echo "Building AI Terminal..."
cargo build --release

# Create the application bundle
echo "Creating application bundle..."
mkdir -p AI-Terminal.app/Contents/{MacOS,Resources}
cp target/release/ai-terminal AI-Terminal.app/Contents/MacOS/
cp run.sh AI-Terminal.app/Contents/MacOS/
cp Info.plist AI-Terminal.app/Contents/

# Make sure the run script is executable
chmod +x AI-Terminal.app/Contents/MacOS/run.sh

# Create a DMG file
echo "Creating DMG file..."
mkdir -p dist
hdiutil create -volname "AI Terminal" -srcfolder AI-Terminal.app -ov -format UDZO dist/AI-Terminal.dmg

echo ""
echo "Package created at dist/AI-Terminal.dmg"
echo "You can distribute this DMG file to others." 