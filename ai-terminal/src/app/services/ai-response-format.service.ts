import { Injectable } from '@angular/core';

export interface ParsedCommandPart {
  command: string;
  fullText: string;
}

export interface ExtractedCodeBlock {
  code: string;
  language: string;
}

export interface ExtractCodeBlocksResult {
  formattedText: string;
  codeBlocks: ExtractedCodeBlock[];
}

@Injectable({
  providedIn: 'root'
})
export class AiResponseFormatService {
  parseCommandFromResponse(response: string): ParsedCommandPart[] {
    const results: ParsedCommandPart[] = [];
    let lastIndex = 0;
    const tripleCommandRegex = /```([^`]+)```/g;
    let match: RegExpExecArray | null;

    while ((match = tripleCommandRegex.exec(response)) !== null) {
      const textBefore = response.slice(lastIndex, match.index);

      if (textBefore) {
        const processedText = this.processSingleBackticks(textBefore);
        if (processedText) {
          results.push({ command: '', fullText: processedText });
        }
      }

      results.push({
        command: match[1].trim(),
        fullText: match[0]
      });

      lastIndex = match.index + match[0].length;

      const nextChars = response.slice(lastIndex, lastIndex + 4);
      if (nextChars === '\\n') {
        results.push({ command: '', fullText: '\n' });
        lastIndex += 4;
      }
    }

    const textAfter = response.slice(lastIndex);
    if (textAfter) {
      const processedText = this.processSingleBackticks(textAfter);
      if (processedText) {
        results.push({ command: '', fullText: processedText });
      }
    }

    return results;
  }

  extractCodeBlocks(text: string): ExtractCodeBlocksResult {
    const codeBlocks: ExtractedCodeBlock[] = [];
    const commandParts = this.parseCommandFromResponse(text);

    if (commandParts.length > 0) {
      const formattedParts = commandParts.map((part) => {
        if (part.command) {
          codeBlocks.push({
            code: part.command,
            language: 'command'
          });
          return `<code-block-${codeBlocks.length - 1}></code-block-${codeBlocks.length - 1}>`;
        }
        return part.fullText;
      });

      return {
        formattedText: formattedParts.join(''),
        codeBlocks
      };
    }

    if (text.trim().startsWith('```') && text.trim().endsWith('```')) {
      const trimmedText = text.trim();
      const content = trimmedText.slice(3, -3).trim();
      if (content) {
        const lines = content.split('\n');
        let code: string;
        let language = 'text';

        if (lines.length > 1 && !lines[0].includes(' ') && lines[0].length < 20) {
          language = lines[0];
          code = lines.slice(1).join('\n').trim();
        } else {
          code = content;
        }

        codeBlocks.push({ code, language });
        return { formattedText: '<code-block-0></code-block-0>', codeBlocks };
      }
    }

    if (text.length < 100 && !text.includes('\n') && !text.includes('```')) {
      codeBlocks.push({
        code: text.trim(),
        language: 'command'
      });

      return { formattedText: '<code-block-0></code-block-0>', codeBlocks };
    }

    const codeBlockRegex = /```([\w-]*)?(?:\s*\n)?([\s\S]*?)```/gm;
    const formattedText = text.replace(codeBlockRegex, (language, code) => {
      if (!code || !code.trim()) {
        return '';
      }

      const trimmedCode = code.trim();
      const index = codeBlocks.length;
      codeBlocks.push({
        code: trimmedCode,
        language: language ? language.trim() : 'text'
      });

      return `<code-block-${index}></code-block-${index}>`;
    });

    return { formattedText, codeBlocks };
  }

  isSimpleCommand(code: string): boolean {
    if (!code) {
      return false;
    }

    const cleanCode = code.replace(/```/g, '').trim();

    if (cleanCode.length < 100 && !cleanCode.includes('\n')) {
      if (cleanCode.split(' ').length <= 5) {
        return true;
      }
    }

    const isSimple =
      !cleanCode.includes('\n') &&
      !cleanCode.includes('|') &&
      !cleanCode.includes('>') &&
      !cleanCode.includes('<') &&
      !cleanCode.includes('=') &&
      cleanCode.length < 80;

    const isCommonCommand =
      cleanCode.startsWith('ls') ||
      cleanCode.startsWith('cd') ||
      cleanCode.startsWith('mkdir') ||
      cleanCode.startsWith('rm') ||
      cleanCode.startsWith('cp') ||
      cleanCode.startsWith('mv') ||
      cleanCode.startsWith('cat') ||
      cleanCode.startsWith('grep') ||
      cleanCode.startsWith('find') ||
      cleanCode.startsWith('echo');

    return isSimple && (isCommonCommand || cleanCode.split(' ').length <= 3);
  }

  isCodeBlockPlaceholder(text: string): boolean {
    const exactMatch = /^<code-block-\d+><\/code-block-\d+>$/.test(text);
    if (exactMatch) {
      return true;
    }
    return text.trim().startsWith('<code-block-') && text.trim().includes('>');
  }

  getCodeBlockIndex(placeholder: string): number {
    let match = placeholder.match(/<code-block-(\d+)><\/code-block-\d+>/);
    if (!match) {
      match = placeholder.match(/<code-block-(\d+)>/);
    }
    return match ? parseInt(match[1], 10) : -1;
  }

  getCommandExplanation(code: string): string | null {
    if (!code) {
      return null;
    }

    const parts = code.split(':');
    if (parts.length > 1 && parts[1].trim()) {
      return parts.slice(1).join(':').trim();
    }

    return null;
  }

  transformCodeForDisplay(code: string): string {
    if (!code) {
      return '';
    }

    let cleanCode = code.replace(/```/g, '').trim();
    const colonIndex = cleanCode.indexOf(':');
    if (colonIndex > -1) {
      cleanCode = cleanCode.substring(0, colonIndex).trim();
    }

    return cleanCode;
  }

  private processSingleBackticks(text: string): string {
    return text.replace(/`([^`]+)`/g, '<b>$1</b>');
  }
}
