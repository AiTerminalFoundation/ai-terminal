declare module '@tauri-apps/api' {
  export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
} 