#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Open a new Terminal window and run the application
osascript <<EOF
tell application "Terminal"
    do script "'$DIR/ai-terminal'"
    activate
end tell
EOF 