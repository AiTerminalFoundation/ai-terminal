#!/bin/bash

# Build the release version
echo "Building AI Terminal..."
cargo build --release

# Copy the binary to the application bundle
echo "Creating application bundle..."
mkdir -p AI-Terminal.app/Contents/{MacOS,Resources}
cp target/release/ai-terminal AI-Terminal.app/Contents/MacOS/
cp run.sh AI-Terminal.app/Contents/MacOS/
cp Info.plist AI-Terminal.app/Contents/

# Make sure the run script is executable
chmod +x AI-Terminal.app/Contents/MacOS/run.sh

# Copy to Applications folder if requested
echo ""
echo "AI Terminal has been built successfully!"
echo ""
read -p "Would you like to install AI Terminal to your Applications folder? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]
then
    echo "Installing to Applications folder..."
    cp -R AI-Terminal.app /Applications/
    echo "Installation complete! You can now run AI Terminal from your Applications folder."
else
    echo "You can manually copy AI-Terminal.app to your Applications folder later."
fi

echo ""
echo "Thank you for using AI Terminal!" 