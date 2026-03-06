import { Injectable } from '@angular/core';

export interface AiCommandContext {
  currentLLMModel: string;
  ollamaApiHost: string;
  setCurrentLLMModel: (model: string) => void;
  setOllamaApiHost: (host: string) => void;
  clearChatHistory: () => void;
  testOllamaConnection: () => void;
  retryOllamaConnection: () => Promise<void>;
}

@Injectable({
  providedIn: 'root'
})
export class AiCommandService {
  async handleAICommand(command: string, context: AiCommandContext): Promise<string> {
    const parts = command.split(' ');
    const cmd = parts[0].toLowerCase();

    switch (cmd) {
      case '/help':
        return `
Available commands:
/help - Show this help message
/models - List available models
/model [name] - Show current model or switch to a different model
/host [url] - Show current API host or set a new one
/retry - Retry connection to Ollama API
/clear - Clear the AI chat history`;

      case '/models':
        try {
          const response = await fetch(`${context.ollamaApiHost}/api/tags`);

          if (!response.ok) {
            throw new Error(`Ollama API error: ${response.status}`);
          }

          const data = await response.json();
          let result = 'Available models:\n';
          for (const model of data.models) {
            result += `- ${model.name} (${model.size} bytes)\n`;
          }
          return result;
        } catch (error) {
          return `Error: Failed to get models from Ollama API: ${error}`;
        }

      case '/model':
        if (parts.length > 1) {
          const modelName = parts[1];
          try {
            context.setCurrentLLMModel(modelName);
            return `Switched to model: ${modelName}`;
          } catch (error) {
            return `Error: Failed to switch model: ${error}`;
          }
        }
        return `Current model: ${context.currentLLMModel}`;

      case '/host':
        if (parts.length > 1) {
          const hostUrl = parts.slice(1).join(' ');
          try {
            context.setOllamaApiHost(hostUrl);
            setTimeout(() => context.testOllamaConnection(), 100);
            return `Changed Ollama API host to: ${hostUrl}`;
          } catch (error) {
            return `Error: Failed to set host: ${error}`;
          }
        }
        return `Current Ollama API host: ${context.ollamaApiHost}`;

      case '/retry':
        setTimeout(() => {
          void context.retryOllamaConnection();
        }, 100);
        return 'Attempting to reconnect to Ollama API...';

      case '/clear':
        context.clearChatHistory();
        return 'AI chat history cleared';

      default:
        return `Unknown command: ${cmd}. Type /help for available commands.`;
    }
  }
}
