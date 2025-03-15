#!/usr/bin/osascript

-- Get the path to the script
set scriptPath to (do shell script "echo $0")

-- Get the directory containing the script
set scriptDir to do shell script "dirname " & quoted form of scriptPath

-- Get the path to the AITerminal binary
set terminalPath to scriptDir & "/AITerminal"

-- Launch the terminal application in a new Terminal window
tell application "Terminal"
    do script quoted form of terminalPath
    activate
end tell 