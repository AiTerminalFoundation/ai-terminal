#!/bin/bash

# Get the directory where the script is located
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Run the actual binary directly
"$DIR/AITerminal"

# Exit with the same code as the application
exit $?
