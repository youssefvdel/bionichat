// Tauri bridge: the app exposes window.__TAURI__ (withGlobalTauri).
// Same commands and events the vanilla dashboard used. Backend untouched.

interface TauriCore {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

interface TauriEvent {
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>;
}

interface TauriGlobal {
  core: TauriCore;
  event: TauriEvent;
}

declare global {
  interface Window {
    __TAURI__?: TauriGlobal;
  }
}

export function bridge(): TauriGlobal | null {
  return window.__TAURI__ ?? null;
}

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const b = bridge();
  if (!b) throw new Error(`Tauri bridge not found. Open the app, not the file.`);
  return b.core.invoke<T>(command, args);
}

