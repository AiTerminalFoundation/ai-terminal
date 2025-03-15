#!/bin/bash

# Check if running as root
if [ "$EUID" -eq 0 ]; then
  echo "Please do not run as root"
  exit 1
fi

# Set variables
APP_NAME="AITerminal.app"
DEST_DIR="/Applications"

# Check if the app exists
if [ ! -d "$APP_NAME" ]; then
  echo "Error: $APP_NAME not found in the current directory"
  exit 1
fi

# Copy the app to Applications folder
echo "Installing $APP_NAME to $DEST_DIR..."
cp -R "$APP_NAME" "$DEST_DIR"

# Check if the copy was successful
if [ $? -eq 0 ]; then
  echo "Installation successful!"
  echo "You can now run AITerminal from your Applications folder"
  echo "Note: The first time you run it, you may need to right-click and select 'Open'"
else
  echo "Installation failed. Please try again or manually copy the app to your Applications folder."
fi 