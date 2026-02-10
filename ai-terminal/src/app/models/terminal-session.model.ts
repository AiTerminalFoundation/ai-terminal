import { CommandHistory } from './command-history.model';

export interface TerminalSession {
  id: string;
  name: string;
  commandHistory: CommandHistory[];
  currentWorkingDirectory: string;
  isActive: boolean;
  gitBranch: string;
  isSshSessionActive: boolean;
  currentSshUserHost: string | null;
}
