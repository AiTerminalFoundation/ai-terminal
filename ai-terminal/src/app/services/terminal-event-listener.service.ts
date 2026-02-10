import { Injectable } from '@angular/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { SSH_PRE_EXEC_PASSWORD_EVENT } from '../constants/ssh.constants';

export interface TerminalEventHandlers {
  onCommandOutput: (payload: string) => void | Promise<void>;
  onCommandError: (payload: string) => void | Promise<void>;
  onCommandEnd: (payload: string) => void | Promise<void>;
  onCommandForwardedToSsh: () => void | Promise<void>;
  onSshPreExecPasswordRequest: (payload: string) => void | Promise<void>;
  onRemoteDirectoryUpdated: (payload: string) => void | Promise<void>;
  onSshSessionStarted: (payload: string) => void | Promise<void>;
  onSshSessionEnded: (payload: string) => void | Promise<void>;
}

@Injectable({
  providedIn: 'root'
})
export class TerminalEventListenerService {
  async registerListeners(handlers: TerminalEventHandlers): Promise<UnlistenFn[]> {
    const unlistenCommandOutput = await listen('command_output', async (event) => {
      await handlers.onCommandOutput(event.payload as string);
    });

    const unlistenCommandError = await listen('command_error', async (event) => {
      await handlers.onCommandError(event.payload as string);
    });

    const unlistenCommandEnd = await listen('command_end', async (event) => {
      await handlers.onCommandEnd(event.payload as string);
    });

    const unlistenCommandForwarded = await listen('command_forwarded_to_ssh', async () => {
      await handlers.onCommandForwardedToSsh();
    });

    const unlistenSshPrompt = await listen(SSH_PRE_EXEC_PASSWORD_EVENT, async (event) => {
      await handlers.onSshPreExecPasswordRequest(event.payload as string);
    });

    const unlistenRemoteDirectory = await listen('remote_directory_updated', async (event) => {
      await handlers.onRemoteDirectoryUpdated(event.payload as string);
    });

    const unlistenSshSessionStarted = await listen('ssh_session_started', async (event) => {
      await handlers.onSshSessionStarted(event.payload as string);
    });

    const unlistenSshSessionEnded = await listen('ssh_session_ended', async (event) => {
      await handlers.onSshSessionEnded(event.payload as string);
    });

    return [
      unlistenCommandOutput,
      unlistenCommandError,
      unlistenCommandEnd,
      unlistenCommandForwarded,
      unlistenSshPrompt,
      unlistenRemoteDirectory,
      unlistenSshSessionStarted,
      unlistenSshSessionEnded
    ];
  }
}
