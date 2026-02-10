import { Injectable } from '@angular/core';
import { CommandHistory } from '../models/command-history.model';
import { TerminalSession } from '../models/terminal-session.model';

export interface TerminalRuntimeState {
  commandHistory: CommandHistory[];
  currentWorkingDirectory: string;
  gitBranch: string;
  isSshSessionActive: boolean;
  currentSshUserHost: string | null;
}

@Injectable({
  providedIn: 'root'
})
export class TerminalSessionService {
  createNewSession(
    sessions: TerminalSession[],
    name?: string,
    setAsActive: boolean = false
  ): { sessions: TerminalSession[]; sessionId: string; shouldActivate: boolean } {
    const sessionId = `session-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const sessionName = name || `Terminal ${sessions.length + 1}`;

    const newSession: TerminalSession = {
      id: sessionId,
      name: sessionName,
      commandHistory: [],
      currentWorkingDirectory: '~',
      isActive: false,
      gitBranch: '',
      isSshSessionActive: false,
      currentSshUserHost: null
    };

    const nextSessions = [...sessions, newSession];
    const shouldActivate = setAsActive || nextSessions.length === 1;
    return { sessions: nextSessions, sessionId, shouldActivate };
  }

  switchToSession(
    sessions: TerminalSession[],
    sessionId: string
  ): { sessions: TerminalSession[]; targetSession?: TerminalSession } {
    const targetSession = sessions.find((session) => session.id === sessionId);
    if (!targetSession) {
      return { sessions };
    }

    const nextSessions = sessions.map((session) => ({
      ...session,
      isActive: session.id === sessionId
    }));

    const activeSession = nextSessions.find((session) => session.id === sessionId);
    return { sessions: nextSessions, targetSession: activeSession };
  }

  closeSession(
    sessions: TerminalSession[],
    sessionId: string
  ): { sessions: TerminalSession[]; nextActiveSessionId?: string } {
    if (sessions.length <= 1) {
      return { sessions };
    }

    const sessionIndex = sessions.findIndex((session) => session.id === sessionId);
    if (sessionIndex === -1) {
      return { sessions };
    }

    const wasActive = sessions[sessionIndex].isActive;
    const nextSessions = sessions.filter((session) => session.id !== sessionId);

    if (!wasActive) {
      return { sessions: nextSessions };
    }

    const newActiveIndex = Math.max(0, sessionIndex - 1);
    const nextActiveSessionId = nextSessions[newActiveIndex]?.id;
    return { sessions: nextSessions, nextActiveSessionId };
  }

  renameSession(sessions: TerminalSession[], sessionId: string, newName: string): TerminalSession[] {
    const trimmedName = newName.trim();
    if (!trimmedName) {
      return sessions;
    }

    return sessions.map((session) =>
      session.id === sessionId ? { ...session, name: trimmedName } : session
    );
  }

  saveCurrentSessionState(
    sessions: TerminalSession[],
    activeSessionId: string,
    runtimeState: TerminalRuntimeState
  ): TerminalSession[] {
    if (!activeSessionId) {
      return sessions;
    }

    return sessions.map((session) =>
      session.id === activeSessionId
        ? {
            ...session,
            commandHistory: [...runtimeState.commandHistory],
            currentWorkingDirectory: runtimeState.currentWorkingDirectory,
            gitBranch: runtimeState.gitBranch,
            isSshSessionActive: runtimeState.isSshSessionActive,
            currentSshUserHost: runtimeState.currentSshUserHost
          }
        : session
    );
  }

  restoreSessionState(session: TerminalSession): TerminalRuntimeState {
    return {
      commandHistory: [...session.commandHistory],
      currentWorkingDirectory: session.currentWorkingDirectory,
      gitBranch: session.gitBranch,
      isSshSessionActive: session.isSshSessionActive,
      currentSshUserHost: session.currentSshUserHost
    };
  }

  getActiveSession(sessions: TerminalSession[], activeSessionId: string): TerminalSession | undefined {
    return sessions.find((session) => session.id === activeSessionId);
  }
}
