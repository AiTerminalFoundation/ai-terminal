#!/usr/bin/swift

import Foundation
import AppKit

// Get the path to the AITerminal binary
let scriptPath = CommandLine.arguments[0]
let scriptDir = URL(fileURLWithPath: scriptPath).deletingLastPathComponent().path
let terminalPath = "\(scriptDir)/AITerminal"

// Create a process to run the terminal application
let process = Process()
process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
process.arguments = ["-a", "Terminal", terminalPath]

do {
    try process.run()
    print("Launched AITerminal in a new Terminal window")
    exit(0)
} catch {
    print("Error launching AITerminal: \(error)")
    exit(1)
} 