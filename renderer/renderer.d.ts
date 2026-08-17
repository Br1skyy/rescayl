export interface IElectronAPI {
  on: (command: string, func?: (...args: any[]) => void) => void;
  off: (command: string, func?: (...args: any[]) => void) => void;
  send: <T>(command: string, payload?: T) => void;
  invoke: <T = any>(command: string, payload?: any) => Promise<T>;
  platform: "mac" | "win" | "linux";
  getSystemInfo: () => Promise<{
    platform: string;
    release: string;
    arch: string;
    model: string;
    cpuCount: number;
  }>;
  getAppVersion: () => Promise<string>;
  getDroppedPaths: () => string[];
}

declare global {
  interface Window {
    electron: IElectronAPI;
  }
}
