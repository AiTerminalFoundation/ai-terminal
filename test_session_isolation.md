# Terminal Session Isolation Test

## Overview
This document outlines how to test the multiple terminal sessions feature with independent working directories.

## Test Steps

1. **Launch the Application**
   - The application should start with one terminal tab named "Terminal 1"
   - The current working directory should be displayed in the prompt

2. **Create Multiple Sessions**
   - Click the "+" button to create a new terminal tab
   - You should now have "Terminal 1" and "Terminal 2"

3. **Test Directory Independence**
   - In Terminal 1:
     ```bash
     cd /tmp
     pwd
     ```
     This should show `/tmp`

   - Switch to Terminal 2 (click on the tab)
   - In Terminal 2:
     ```bash
     pwd
     ```
     This should show the original directory (likely your home directory), NOT `/tmp`

   - In Terminal 2:
     ```bash
     cd /var
     pwd
     ```
     This should show `/var`

   - Switch back to Terminal 1
   - In Terminal 1:
     ```bash
     pwd
     ```
     This should still show `/tmp`, proving the sessions are isolated

4. **Test Session Persistence**
   - Create multiple directories in different sessions
   - Switch between tabs multiple times
   - Each session should maintain its own working directory

## Expected Results
- Each terminal tab maintains its own independent working directory
- Changing directory in one tab does not affect other tabs
- Session state is preserved when switching between tabs
- Git branch information is session-specific
- SSH sessions are isolated per tab

## Success Criteria
✅ Multiple terminal tabs can be created
✅ Each tab has an independent working directory
✅ Directory changes in one tab don't affect others
✅ Session switching preserves state
✅ UI properly shows active tab and allows tab management
