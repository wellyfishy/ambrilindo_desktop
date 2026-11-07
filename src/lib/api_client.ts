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
  // A browser device (tablet, TV) opened this page from the host itself, e.g.
  // http://192.168.1.5:8000/scoreboard — the host is wherever the page came from.
  // (Port 1420 is the Vite dev server, which has no API.)
  if (
    typeof window !== "undefined" &&
    !(window as any).__TAURI_INTERNALS__ &&
    /^https?:$/.test(window.location.protocol) &&
    window.location.port !== "1420"
  ) {
    return window.location.origin;
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

function hasTauri(): boolean {
  return typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;
}

/** True for windows of the host/solo desktop app itself (not an operator tambahan). */
export function isHostUi(): boolean {
  return hasTauri() && !isClientMode();
}

/** Base URL of the Axum host this window should talk to over HTTP/WS. */
export function getHostBaseUrl(): string {
  return connectionState.hostUrl.replace(/\/$/, "");
}

/**
 * Adds `hostUrl=` to an app path when this window is an operator tambahan, so a
 * window it opens keeps talking to the host instead of falling back to local mode.
 */
export function withHostParam(path: string): string {
  if (!isClientMode()) return path;
  const sep = path.includes("?") ? "&" : "?";
  return `${path}${sep}hostUrl=${encodeURIComponent(getHostBaseUrl())}`;
}

/**
 * Commands that act on this machine's own windows and monitors. They always run on
 * the local desktop app, even for an operator tambahan; the window they open is
 * told the host URL so it talks to the host.
 */
const LOCAL_WINDOW_COMMANDS = new Set([
  "open_control_panel_window",
  "open_festival_control_panel",
  "close_control_panel_window",
  "project_scoreboard",
  "close_scoreboard",
  "get_available_monitors",
  "get_running_scoreboards",
]);

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
  if (!isClientMode() && hasTauri()) {
    return await tauriInvoke<T>(command, args);
  }

  // Operator tambahan with the desktop app: window/monitor commands run locally.
  if (hasTauri() && LOCAL_WINDOW_COMMANDS.has(command)) {
    if (command === "open_control_panel_window" && Number(args?.matchId) > 0) {
      // The host's DB records which match runs on which tatami.
      await remoteCall("notify_match_running", { matchId: args.matchId, tatami: args.tatami });
    }
    return await tauriInvoke<T>(command, { ...args, hostUrl: getHostBaseUrl() });
  }

  return remoteCall<T>(command, args);
}

/** Calls the host's universal RPC dispatcher over HTTP. */
async function remoteCall<T = any>(command: string, args: Record<string, any> = {}): Promise<T> {
  const base = getHostBaseUrl();
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
