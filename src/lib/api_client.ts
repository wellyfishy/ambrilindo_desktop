import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { AppMode } from "./types";

export interface AppConnectionState {
  mode: AppMode;
  hostUrl: string;
}

function getInitialMode(): AppMode {
  if (typeof window !== "undefined" && window.sessionStorage) {
    const saved = window.sessionStorage.getItem("ambrilindo_app_mode");
    if (saved === "client" || saved === "host" || saved === "solo") {
      return saved as AppMode;
    }
  }
  return "solo";
}

function getInitialHostUrl(): string {
  if (typeof window !== "undefined" && window.sessionStorage) {
    const saved = window.sessionStorage.getItem("ambrilindo_host_url");
    if (saved) return saved;
  }
  return "http://127.0.0.1:8000";
}

export const connectionState: AppConnectionState = {
  mode: getInitialMode(),
  hostUrl: getInitialHostUrl(),
};

export function setConnectionConfig(mode: AppMode, hostUrl?: string) {
  connectionState.mode = mode;
  if (hostUrl) {
    let clean = hostUrl.trim().replace(/\/$/, "");
    if (!clean.startsWith("http://") && !clean.startsWith("https://")) {
      clean = "http://" + clean;
    }
    connectionState.hostUrl = clean;
  }
  if (typeof window !== "undefined" && window.sessionStorage) {
    window.sessionStorage.setItem("ambrilindo_app_mode", connectionState.mode);
    window.sessionStorage.setItem("ambrilindo_host_url", connectionState.hostUrl);
  }
}

export function getConnectionConfig(): AppConnectionState {
  return { ...connectionState };
}

export function isClientMode(): boolean {
  if (connectionState.mode === "client") return true;
  if (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) return true;
  return false;
}

/**
 * Universal IPC & REST transport client.
 * In Solo / Host mode with Tauri available: invokes local Rust IPC commands.
 * In Client mode or browser mode: proxies requests over HTTP to Axum server at hostUrl.
 */
export async function apiCall<T = any>(
  command: string,
  args: Record<string, any> = {}
): Promise<T> {
  // Special handling for media URLs on client machines
  if (command === "get_media_data_url" && isClientMode()) {
    const rel = args?.relativePath || args?.relative_path || "";
    if (!rel) return null as T;
    const clean = rel.replace(/^[/\\]+/, "");
    return `${connectionState.hostUrl}/media/${clean}` as T;
  }

  // If local Tauri IPC is active and not running as LAN client, invoke directly
  if (!isClientMode() && typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
    return await tauriInvoke<T>(command, args);
  }

  // Client / LAN mode: proxy request via HTTP to Axum server
  const base = connectionState.hostUrl.replace(/\/$/, "");
  const url = `${base}/api/command/${command}`;

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(args || {}),
  });

  if (!res.ok) {
    let errMsg = `HTTP ${res.status}: ${res.statusText}`;
    try {
      const errJson = await res.json();
      if (errJson.message) {
        errMsg = errJson.message;
      }
    } catch {}
    throw new Error(errMsg);
  }

  const json = await res.json();
  if (json.status === "error") {
    throw new Error(json.message || "Unknown remote host error");
  }

  return json.data as T;
}

// Export as both apiCall and invoke drop-in replacement
export { apiCall as invoke };
