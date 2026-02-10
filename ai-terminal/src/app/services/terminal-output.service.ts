import { Injectable } from '@angular/core';

export interface CleanOutputResult {
  lineToDisplay: string | null;
  newExpectingSshEchoState: boolean;
}

@Injectable({
  providedIn: 'root'
})
export class TerminalOutputService {
  stripAnsiCodes(text: string): string {
    return text.replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, '');
  }

  stripTerminalTitle(text: string): string {
    return text.replace(/\x1b\]0;.*\x07/g, '');
  }

  cleanOutputLine(
    rawLine: string,
    commandText: string,
    isSsh: boolean,
    isCurrentlyExpectingEcho: boolean
  ): CleanOutputResult {
    let cleanedLine = this.stripAnsiCodes(rawLine);
    cleanedLine = this.stripTerminalTitle(cleanedLine);

    let finalLineToDisplay: string | null = cleanedLine;
    let updatedExpectingEcho = isCurrentlyExpectingEcho;

    if (isSsh) {
      const trimmedCleanedLine = cleanedLine.trim();
      const trimmedCommandText = commandText.trim();

      if (isCurrentlyExpectingEcho) {
        if (trimmedCommandText.startsWith('cd ') && cleanedLine.includes('__REMOTE_CD_PWD_MARKER_')) {
          finalLineToDisplay = null;
          updatedExpectingEcho = false;
          return { lineToDisplay: finalLineToDisplay, newExpectingSshEchoState: updatedExpectingEcho };
        }

        const commandStartIndex = trimmedCleanedLine.indexOf(trimmedCommandText);
        if (commandStartIndex !== -1) {
          finalLineToDisplay = null;
          updatedExpectingEcho = false;
          return { lineToDisplay: finalLineToDisplay, newExpectingSshEchoState: updatedExpectingEcho };
        }

        if (trimmedCleanedLine !== '') {
          updatedExpectingEcho = false;
        }
      }

      const promptRegex = /^([\w\W]*?([\w.-]+@[\w.-]+(:\s?[\/\w\.~-]+)?)\s*)?([\$#%])\s*$/;
      if (promptRegex.test(trimmedCleanedLine)) {
        const potentialPromptOnly = trimmedCleanedLine.replace(promptRegex, '').trim();
        if (potentialPromptOnly === '') {
          finalLineToDisplay = null;
        }
      }
    }

    if (finalLineToDisplay !== null && finalLineToDisplay.trim() === '' && rawLine.trim() !== '') {
      finalLineToDisplay = null;
    }

    return { lineToDisplay: finalLineToDisplay, newExpectingSshEchoState: updatedExpectingEcho };
  }
}
