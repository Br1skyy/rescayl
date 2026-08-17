import { convertFileSrc } from "@tauri-apps/api/core";

// Tauri v2 blocks `file://` URLs in the webview. Local files must be served
// through the `asset://` protocol, which is enabled in tauri.conf.json
// (`app.security.assetProtocol`).
export function fileAssetUrl(filePath: string): string {
  if (typeof window === "undefined" || !filePath) {
    return "";
  }
  return convertFileSrc(filePath);
}