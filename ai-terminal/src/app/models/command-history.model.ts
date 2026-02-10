export interface CommandHistory {
  command: string;
  output: string[];
  timestamp: Date;
  isComplete: boolean;
  isStreaming?: boolean;
  success?: boolean;
  expectingSshEcho?: boolean;
}
