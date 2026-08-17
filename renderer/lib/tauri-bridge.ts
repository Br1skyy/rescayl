import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";

type Handler = (...args: any[]) => void;

// File paths received from Tauri's native drag-drop handling. The DOM
// `File` objects in the webview do not expose a real filesystem path.
let droppedPaths: string[] = [];

function initDragDrop() {
  getCurrentWebview()
    .onDragDropEvent((event) => {
      if (event.payload.type === "drop") {
        droppedPaths = event.payload.paths;
      }
    })
    .catch((error) => {
      console.error("Rescayl drag-drop init error:", error);
    });
}

// Tauri's `listen` is async and resolves to an unlisten function; store the
// promise so `off` can unregister a specific handler once it is ready.
const listeners = new Map<
  string,
  Array<{ handler: Handler; unlisten: Promise<UnlistenFn> }>
>();

function detectPlatform(): "mac" | "win" | "linux" {
  const ua = navigator.userAgent;
  if (/Macintosh|Mac OS X|Darwin/i.test(ua)) return "mac";
  if (/Windows/i.test(ua)) return "win";
  return "linux";
}

// Only install the bridge in a real browser environment. Next.js evaluates
// this module during static export (SSR), where `window` does not exist.
if (typeof window !== "undefined") {
  const tauriBridge = {
    send: (command: string, payload?: any) => {
      invoke("command", { command, payload }).catch((error) => {
        console.error("Rescayl bridge send error:", command, error);
      });
    },
    on: (command: string, handler: Handler) => {
      const unlisten = listen(command, (event) =>
        handler(event, event.payload),
      ).catch((error) => {
        console.error("Rescayl bridge listen error:", command, error);
      });
      const entry = {
        handler,
        unlisten: unlisten as unknown as Promise<UnlistenFn>,
      };
      const existing = listeners.get(command) ?? [];
      existing.push(entry);
      listeners.set(command, existing);
    },
    off: (command: string, handler: Handler) => {
      const existing = listeners.get(command) ?? [];
      const index = existing.findIndex((entry) => entry.handler === handler);
      if (index !== -1) {
        const [removed] = existing.splice(index, 1);
        removed.unlisten.then((unlisten) => unlisten());
      }
      if (existing.length === 0) listeners.delete(command);
    },
    invoke: (command: string, payload?: any) =>
      invoke("command", { command, payload }),
    platform: detectPlatform(),
    getSystemInfo: () =>
      invoke<{
        platform: string;
        release: string;
        arch: string;
        model: string;
        cpuCount: number;
      }>("get_system_info"),
    getAppVersion: () => invoke<string>("get_app_version"),
    getDroppedPaths: () => droppedPaths,
  };

  initDragDrop();

  if (!window.electron) {
    (window as any).electron = tauriBridge;
  }
}