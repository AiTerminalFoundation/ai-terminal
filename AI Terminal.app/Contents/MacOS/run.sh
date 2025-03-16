#!/bin/bash

# Set the APP_BUNDLE environment variable
export APP_BUNDLE=1

# Get the directory where this script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Run the actual binary directly (no Terminal.app needed)
"$DIR/ai-terminal"

# Exit with the same status as the binary
exit $?
