export interface ChatHistory {
  message: string;
  response: string;
  timestamp: Date;
  isCommand?: boolean;
  codeBlocks?: { code: string; language: string }[];
}
