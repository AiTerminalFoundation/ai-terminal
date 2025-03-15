#!/bin/bash

# Set the APP_BUNDLE environment variable
export APP_BUNDLE=1

# Get the directory where this script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Create a temporary script that will run our application
TEMP_SCRIPT=$(mktemp /tmp/ai-terminal-XXXXX.sh)

cat > "$TEMP_SCRIPT" << INNEREOF
#!/bin/bash
cd "$DIR"
"$DIR/ai-terminal"
echo "Press any key to exit..."
read -n 1
INNEREOF

chmod +x "$TEMP_SCRIPT"

# Launch Terminal.app with our script
open -a Terminal.app "$TEMP_SCRIPT"

# Exit with success
exit 0
