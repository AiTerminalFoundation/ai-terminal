import { Injectable } from '@angular/core';
import { ChatHistory } from '../models/chat-history.model';

export interface OllamaConnectionContext {
  ollamaApiHost: string;
  currentLLMModel: string;
  setCurrentLLMModel: (model: string) => void;
  addChatEntry: (entry: ChatHistory) => void;
}

@Injectable({
  providedIn: 'root'
})
export class OllamaConnectionService {
  async testOllamaConnection(context: OllamaConnectionContext): Promise<void> {
    try {
      const response = await fetch(`${context.ollamaApiHost}/api/tags`, {
        method: 'GET'
      });

      if (response.ok) {
        const data = await response.json();
        if (data && data.models && data.models.length > 0) {
          const availableModels = data.models.map((m: any) => m.name).join(', ');
          const modelExists = data.models.some((m: any) => m.name === context.currentLLMModel);

          if (!modelExists && data.models.length > 0) {
            const selectedModel = data.models[0].name;
            context.setCurrentLLMModel(selectedModel);
            context.addChatEntry({
              message: ' System',
              response: `Connected to Ollama API. Available models: ${availableModels}
Using: ${selectedModel}`,
              timestamp: new Date(),
              isCommand: true
            });
          } else if (modelExists) {
            context.addChatEntry({
              message: ' System',
              response: `Connected to Ollama API. Using model: ${context.currentLLMModel}`,
              timestamp: new Date(),
              isCommand: true
            });
          }
        } else {
          context.addChatEntry({
            message: 'System',
            response: 'Connected to Ollama API, but no models are available. Please install models with "ollama pull <model>".',
            timestamp: new Date(),
            isCommand: true
          });
        }
      } else {
        console.error('Ollama connection test failed:', response.status);
        context.addChatEntry({
          message: 'System',
          response:
            'Could not connect to Ollama API. Please make sure Ollama is running on ' +
            context.ollamaApiHost +
            ' or change the host using /host command.',
          timestamp: new Date(),
          isCommand: true
        });
      }
    } catch (error) {
      console.error('Error testing Ollama connection:', error);
      context.addChatEntry({
        message: 'System',
        response:
          'Could not connect to Ollama API. Please make sure Ollama is running on ' +
          context.ollamaApiHost +
          ' or change the host using /host command.',
        timestamp: new Date(),
        isCommand: true
      });
    }
  }

  async retryOllamaConnection(context: OllamaConnectionContext): Promise<void> {
    context.addChatEntry({
      message: 'System',
      response: '🔄 Retrying connection to Ollama API...',
      timestamp: new Date(),
      isCommand: true
    });
    await this.testOllamaConnection(context);
  }

  async checkModelExists(modelName: string, context: OllamaConnectionContext): Promise<boolean> {
    try {
      const response = await fetch(`${context.ollamaApiHost}/api/tags`, {
        method: 'GET'
      });

      if (!response.ok) {
        console.error(`Failed to get models: ${response.status}`);
        return false;
      }

      const data = await response.json();

      if (!data.models || !Array.isArray(data.models)) {
        console.error('Unexpected response format when checking models:', data);
        return false;
      }

      const modelExists = data.models.some((m: any) => m.name === modelName);

      if (!modelExists && data.models.length > 0) {
        const fallbackModel = data.models[0].name;
        context.setCurrentLLMModel(fallbackModel);
        context.addChatEntry({
          message: 'System',
          response: `ℹModel '${modelName}' not found. Automatically switched to '${fallbackModel}'.`,
          timestamp: new Date(),
          isCommand: true
        });
        return true;
      }

      return modelExists;
    } catch (_error: any) {
      return false;
    }
  }
}
