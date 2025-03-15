#!/bin/bash

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if AI-Terminal.app exists
echo -e "${BLUE}Checking if AI-Terminal.app exists...${NC}"
if [ ! -d "AI-Terminal.app" ]; then
    echo -e "${RED}ERROR: AI-Terminal.app not found in current directory.${NC}"
    echo "Make sure you're running this script from the directory containing AI-Terminal.app"
    exit 1
else
    echo -e "${GREEN}Found AI-Terminal.app${NC}"
fi

# Check app bundle structure
echo -e "\n${BLUE}Checking app bundle structure...${NC}"
required_dirs=(
    "AI-Terminal.app/Contents"
    "AI-Terminal.app/Contents/MacOS"
    "AI-Terminal.app/Contents/Resources"
)

for dir in "${required_dirs[@]}"; do
    if [ -d "$dir" ]; then
        echo -e "${GREEN}✓ Found $dir${NC}"
    else
        echo -e "${RED}✗ Missing $dir${NC}"
    fi
done

# Check required files
echo -e "\n${BLUE}Checking required files...${NC}"
required_files=(
    "AI-Terminal.app/Contents/Info.plist"
    "AI-Terminal.app/Contents/PkgInfo"
    "AI-Terminal.app/Contents/MacOS/run.sh"
    "AI-Terminal.app/Contents/MacOS/ai-terminal"
)

for file in "${required_files[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓ Found $file${NC}"
    else
        echo -e "${RED}✗ Missing $file${NC}"
    fi
done

# Check permissions
echo -e "\n${BLUE}Checking file permissions...${NC}"
echo "run.sh permissions:"
ls -la AI-Terminal.app/Contents/MacOS/run.sh
echo "binary permissions:"
ls -la AI-Terminal.app/Contents/MacOS/ai-terminal

# Check if binary is executable
echo -e "\n${BLUE}Testing binary execution...${NC}"
if [ -x "AI-Terminal.app/Contents/MacOS/ai-terminal" ]; then
    echo -e "${GREEN}Binary is executable${NC}"
    
    # Try to run the binary directly
    echo "Attempting to run binary directly..."
    output=$(AI-Terminal.app/Contents/MacOS/ai-terminal --version 2>&1 || echo "Failed to run binary")
    echo "Output: $output"
else
    echo -e "${RED}Binary is not executable${NC}"
fi

# Check app signing
echo -e "\n${BLUE}Checking app signing...${NC}"
codesign -dv --verbose=4 AI-Terminal.app 2>&1

# Check for log file
echo -e "\n${BLUE}Checking for log files...${NC}"
LOG_FILE="$HOME/Library/Logs/AITerminal/launch.log"
if [ -f "$LOG_FILE" ]; then
    echo -e "${GREEN}Log file exists. Contents:${NC}"
    cat "$LOG_FILE"
else
    echo -e "${YELLOW}No log file found at $LOG_FILE${NC}"
fi

# Try launching the app
echo -e "\n${BLUE}Attempting to launch app...${NC}"
echo "Running: open ./AI-Terminal.app"
open ./AI-Terminal.app

echo -e "\n${YELLOW}If the app still doesn't open, try these steps:${NC}"
echo "1. Check for security restrictions: System Preferences > Security & Privacy"
echo "2. Check Console.app for any crash reports related to 'ai-terminal'"
echo "3. Try running the app from Terminal with: open -a AI-Terminal.app --args --verbose" 