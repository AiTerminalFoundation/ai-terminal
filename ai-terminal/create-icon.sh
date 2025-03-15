#!/bin/bash
set -e

# Check if input file is provided
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <path-to-1024x1024-png>"
    exit 1
fi

INPUT_FILE="$1"
ICONSET_NAME="AppIcon.iconset"
OUTPUT_FILE="AppIcon.icns"

# Create iconset directory
mkdir -p "$ICONSET_NAME"

# Generate different icon sizes
sips -z 16 16     "$INPUT_FILE" --out "$ICONSET_NAME/icon_16x16.png"
sips -z 32 32     "$INPUT_FILE" --out "$ICONSET_NAME/icon_16x16@2x.png"
sips -z 32 32     "$INPUT_FILE" --out "$ICONSET_NAME/icon_32x32.png"
sips -z 64 64     "$INPUT_FILE" --out "$ICONSET_NAME/icon_32x32@2x.png"
sips -z 128 128   "$INPUT_FILE" --out "$ICONSET_NAME/icon_128x128.png"
sips -z 256 256   "$INPUT_FILE" --out "$ICONSET_NAME/icon_128x128@2x.png"
sips -z 256 256   "$INPUT_FILE" --out "$ICONSET_NAME/icon_256x256.png"
sips -z 512 512   "$INPUT_FILE" --out "$ICONSET_NAME/icon_256x256@2x.png"
sips -z 512 512   "$INPUT_FILE" --out "$ICONSET_NAME/icon_512x512.png"
sips -z 1024 1024 "$INPUT_FILE" --out "$ICONSET_NAME/icon_512x512@2x.png"

# Convert iconset to icns
iconutil -c icns "$ICONSET_NAME"

# Move the icon to the Resources directory
mkdir -p AI-Terminal.app/Contents/Resources
cp "$OUTPUT_FILE" AI-Terminal.app/Contents/Resources/

# Clean up
rm -rf "$ICONSET_NAME"

echo "Icon created at $OUTPUT_FILE and copied to AI-Terminal.app/Contents/Resources/" 