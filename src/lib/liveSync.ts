import { emit as tauriEmit, listen as tauriListen } from "@tauri-apps/api/event";
import type { TatamiItem } from "./types";
import { getConnectionConfig, getHostBaseUrl, isHostUi } from "./api_client";

export interface TatamiLiveMatchPayload {
  tatami: number;
  matchId: number | null;
  baganId: number | null;
  category?: string;
  isTimerRunning?: boolean;
  isFinished?: boolean;
  isClosed?: boolean;
  [key: string]: any;
}

// In-memory active live matches state
// Key: tatamiNumber -> TatamiLiveMatchPayload
let activeLiveTatamis: Record<number, TatamiLiveMatchPayload> = {};

let liveBroadcastChannel: BroadcastChannel | null = null;
let wsClient: WebSocket | null = null;
let wsReconnectTimer: any = null;
let currentHostUrl: string = "";
let isInitialized = false;
let lastBroadcastPayload: TatamiLiveMatchPayload | null = null;

type LiveStateListener = (activeTatamis: Record<number, TatamiLiveMatchPayload>) => void;
const listeners = new Set<LiveStateListener>();

// Brackets changed somewhere (a result saved on any laptop, a drawing, a manual edit), or
// this window just reconnected and may have missed such a change.
const bracketListeners = new Set<() => void>();
let wsEverOpened = false;

function notifyBracketListeners() {
  for (const cb of bracketListeners) {
    try {
      cb();
    } catch (e) {
      console.warn("LiveSync bracket listener error:", e);
    }
  }
}

/** Runs `cb` when brackets change on any device, and after the LAN link comes back. */
export function subscribeBracketUpdates(cb: () => void): () => void {
  bracketListeners.add(cb);
  return () => bracketListeners.delete(cb);
}

// Referees changed on the host (roster, photo, tatami team, a bout's panel)
const refereeListeners = new Set<() => void>();
function notifyRefereeListeners() {
  for (const cb of refereeListeners) {
    try {
      cb();
    } catch (e) {
      console.warn("referee listener failed:", e);
    }
  }
}

/** Called when referees change anywhere on the host, and after a reconnect. */
export function subscribeRefereeUpdates(cb: () => void): () => void {
  refereeListeners.add(cb);
  return () => refereeListeners.delete(cb);
}

function notifyListeners() {
  for (const listener of listeners) {
    try {
      listener({ ...activeLiveTatamis });
    } catch (e) {
      console.warn("LiveSync listener error:", e);
    }
  }
}

/**
 * Applies a live state payload to the active map
 */
export function applyLiveMatchPayload(data: any) {
  if (!data) return;
  const tNum = Number(data.tatami);
  if (!tNum || isNaN(tNum)) return;

  const mId = data.matchId ? Number(data.matchId) : null;
  const bId = data.baganId ? Number(data.baganId) : null;
  const isFinished = !!(data.isFinished || data.isClosed);

  if (isFinished) {
    if (activeLiveTatamis[tNum]) {
      delete activeLiveTatamis[tNum];
      notifyListeners();
    }
  } else if (mId && mId > 0) {
    const next: TatamiLiveMatchPayload = {
      tatami: tNum,
      matchId: mId,
      baganId: bId,
      category: data.category || "",
      isTimerRunning: !!data.isTimerRunning,
      isFinished: false,
      isClosed: false,
    };
    const prev = activeLiveTatamis[tNum];
    activeLiveTatamis[tNum] = next;
    // Control panels send their state ~7x/second while the timer runs. Only wake
    // subscribers (sidebar, bracket view, tatami manager) when something they
    // display actually changed, not on every timer tick.
    if (
      !prev ||
      prev.matchId !== next.matchId ||
      prev.baganId !== next.baganId ||
      prev.category !== next.category ||
      prev.isTimerRunning !== next.isTimerRunning
    ) {
      notifyListeners();
    }
  }
}

/**
 * Initializes the unified live synchronization client (Tauri IPC, BroadcastChannel, and LAN WebSocket).
 */
export function initLiveSync(hostUrl?: string) {
  if (typeof window === "undefined") return;

  if (hostUrl) {
    currentHostUrl = hostUrl;
  } else {
    currentHostUrl = getConnectionConfig().hostUrl;
  }

  // 1. Setup local BroadcastChannel (same browser tab / multi-window)
  if (window.BroadcastChannel && !liveBroadcastChannel) {
    try {
      liveBroadcastChannel = new BroadcastChannel("ambrilindo_live_sync");
      liveBroadcastChannel.onmessage = (event) => {
        if (event.data) {
          applyLiveMatchPayload(event.data);
        }
      };
    } catch (_) {}
  }

  // 2. Setup local Tauri IPC listener
  if (!isInitialized && (window as any).__TAURI_INTERNALS__) {
    tauriListen("tatami_live_state", (event: any) => {
      applyLiveMatchPayload(event.payload);
    }).catch(() => {});
  }

  // 3. Connect / Reconnect LAN WebSocket to Axum Server
  connectWebSocket();

  isInitialized = true;
}

function connectWebSocket() {
  if (typeof window === "undefined") return;

  if (wsReconnectTimer) {
    clearTimeout(wsReconnectTimer);
    wsReconnectTimer = null;
  }

  if (wsClient) {
    if (wsClient.readyState === WebSocket.OPEN || wsClient.readyState === WebSocket.CONNECTING) {
      return;
    }
    try {
      wsClient.close();
    } catch (_) {}
    wsClient = null;
  }

  try {
    const rawHost = (currentHostUrl || getConnectionConfig().hostUrl || "http://127.0.0.1:8000")
      .replace(/^http:\/\//, "")
      .replace(/^https:\/\//, "")
      .replace(/\/$/, "");

    const wsUrl = `ws://${rawHost}/ws/bracket/0`;
    wsClient = new WebSocket(wsUrl);

    wsClient.onopen = () => {
      // After a reconnect, refresh whatever may have changed while the link was down.
      if (wsEverOpened) {
        notifyBracketListeners();
        notifyRefereeListeners();
      }
      wsEverOpened = true;
      if (lastBroadcastPayload && wsClient && wsClient.readyState === WebSocket.OPEN) {
        try {
          wsClient.send(
            JSON.stringify({
              type: "tatami_live_state",
              origin: isHostUi() ? "host" : undefined,
              payload: lastBroadcastPayload,
            })
          );
        } catch (_) {}
      }
    };

    wsClient.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (msg.type === "tatami_live_state") {
          applyLiveMatchPayload(msg.data || msg.payload || msg);
        } else if (msg.type === "bracket_updated") {
          notifyListeners();
          notifyBracketListeners();
        } else if (msg.type === "referee_updated") {
          notifyRefereeListeners();
        } else if (msg.tatami !== undefined) {
          applyLiveMatchPayload(msg);
        }
      } catch (_) {}
    };

    wsClient.onclose = () => {
      wsClient = null;
      wsReconnectTimer = setTimeout(() => {
        connectWebSocket();
      }, 3000);
    };

    wsClient.onerror = () => {
      try {
        wsClient?.close();
      } catch (_) {}
    };
  } catch (_) {
    wsReconnectTimer = setTimeout(() => {
      connectWebSocket();
    }, 5000);
  }
}

/**
 * Universal broadcast function. Sends match live state to:
 * 1. Tauri local IPC
 * 2. BroadcastChannel (same laptop / dev mode)
 * 3. Axum WebSocket relay (reaches all other laptops on the LAN!)
 */
export function broadcastLiveMatch(payload: TatamiLiveMatchPayload) {
  lastBroadcastPayload = payload;
  // Update local memory state immediately
  applyLiveMatchPayload(payload);

  // 1. Emit via local Tauri IPC
  if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
    tauriEmit("tatami_live_state", payload).catch(() => {});
  }

  // 2. Broadcast via local BroadcastChannel
  if (liveBroadcastChannel) {
    try {
      liveBroadcastChannel.postMessage(payload);
    } catch (_) {}
  }

  // 3. Send over WebSocket to Axum Server so all other laptops receive it
  if (wsClient && wsClient.readyState === WebSocket.OPEN) {
    try {
      wsClient.send(
        JSON.stringify({
          type: "tatami_live_state",
          // The host app already delivered this to its own windows over Tauri IPC;
          // tell the relay not to re-emit it there.
          origin: isHostUi() ? "host" : undefined,
          payload,
        })
      );
    } catch (e) {
      console.warn("LiveSync WS send failed:", e);
    }
  }
}

/**
 * Checks whether a specific match ID is currently running on any Tatami
 */
export function isMatchLive(matchId: number | null | undefined, tatamis?: TatamiItem[]): boolean {
  if (!matchId) return false;

  // 1. Check live in-memory state
  for (const item of Object.values(activeLiveTatamis)) {
    if (item.matchId === matchId && !item.isFinished && !item.isClosed) {
      return true;
    }
  }

  // 2. Check tatamis prop (only if detail_bagan_id matches and not finished)
  if (tatamis && tatamis.length > 0) {
    return tatamis.some((t) => t.detail_bagan_id === matchId);
  }

  return false;
}

/**
 * Gets the running tatami number for a given match ID, or null
 */
export function getRunningTatamiForMatch(matchId: number | null | undefined, tatamis?: TatamiItem[]): number | null {
  if (!matchId) return null;

  for (const item of Object.values(activeLiveTatamis)) {
    if (item.matchId === matchId && !item.isFinished && !item.isClosed) {
      return item.tatami;
    }
  }

  if (tatamis && tatamis.length > 0) {
    const t = tatamis.find((x) => x.detail_bagan_id === matchId);
    if (t && t.tatami_number) return t.tatami_number;
  }

  return null;
}

/**
 * Gets array of active running tatami numbers for a specific Bagan ID
 */
export function getLiveTatamisForBagan(baganId: number | null | undefined, tatamis?: TatamiItem[]): number[] {
  if (!baganId) return [];
  const set = new Set<number>();

  // 1. In-memory active live state
  for (const item of Object.values(activeLiveTatamis)) {
    if (item.baganId === baganId && !item.isFinished && !item.isClosed && item.tatami) {
      set.add(item.tatami);
    }
  }

  // 2. Tatamis prop from DB (if bagan_id and detail_bagan_id are set)
  if (tatamis && tatamis.length > 0) {
    for (const t of tatamis) {
      if (t.bagan_id === baganId && t.detail_bagan_id && t.tatami_number) {
        set.add(t.tatami_number);
      }
    }
  }

  return Array.from(set).sort((a, b) => a - b);
}

/**
 * Subscribe to real-time live sync changes
 */
export function subscribeLiveSync(callback: LiveStateListener): () => void {
  listeners.add(callback);
  // Initial fire with current state
  callback({ ...activeLiveTatamis });
  return () => {
    listeners.delete(callback);
  };
}

export interface TatamiSocket {
  send: (data: Record<string, any>) => void;
  close: () => void;
}

/**
 * Opens a reconnecting WebSocket to the host for one tatami
 * (`/ws/<role>/<tatami>`). The host relays to it only that tatami's traffic plus
 * global messages. `onOpen` runs after every (re)connect.
 */
export interface TatamiSocketOptions {
  /**
   * Watchdog for half-open connections (e.g. Wi-Fi dropped without a close):
   * after this many ms without any message, `onStale` runs (ask for fresh state);
   * if nothing arrives within 5 s after that, the socket is dropped and reopened.
   */
  staleMs?: number;
  onStale?: (socket: TatamiSocket) => void;
}

export function connectTatamiSocket(
  role: string,
  tatami: number,
  onMessage: (msg: any) => void,
  onOpen?: (socket: TatamiSocket) => void,
  /** Runs when the connection drops (it reconnects by itself); `ev` is missing for a dead link the watchdog dropped. */
  onClose?: (ev?: CloseEvent) => void,
  options: TatamiSocketOptions = {}
): TatamiSocket {
  let ws: WebSocket | null = null;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  let watchdog: ReturnType<typeof setInterval> | null = null;
  let closed = false;
  // Reconnect quickly first (0.5 s), then back off to 3 s
  let retryDelay = 500;
  let lastMessageAt = 0;
  let staleAskedAt = 0;

  const handle: TatamiSocket = {
    send(data) {
      if (ws && ws.readyState === WebSocket.OPEN) {
        try {
          ws.send(JSON.stringify(data));
        } catch (_) {}
      }
    },
    close() {
      closed = true;
      if (retryTimer) clearTimeout(retryTimer);
      if (watchdog) clearInterval(watchdog);
      if (typeof window !== "undefined") {
        window.removeEventListener("online", reconnectNow);
        document.removeEventListener("visibilitychange", onVisible);
      }
      try {
        ws?.close();
      } catch (_) {}
      ws = null;
    },
  };

  function scheduleRetry() {
    if (closed || retryTimer) return;
    retryTimer = setTimeout(() => {
      retryTimer = null;
      connect();
    }, retryDelay);
    retryDelay = Math.min(3000, retryDelay * 2);
  }

  /** Drop whatever we have and connect again right away */
  function reconnectNow() {
    if (closed) return;
    if (ws && ws.readyState === WebSocket.OPEN) return;
    if (retryTimer) {
      clearTimeout(retryTimer);
      retryTimer = null;
    }
    retryDelay = 500;
    try {
      ws?.close();
    } catch (_) {}
    ws = null;
    connect();
  }

  // A tablet waking up from sleep: check the link immediately
  function onVisible() {
    if (document.visibilityState === "visible") reconnectNow();
  }

  function connect() {
    if (closed || typeof window === "undefined") return;
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
    const base = getHostBaseUrl().replace(/^http/, "ws");
    let sock: WebSocket;
    try {
      sock = new WebSocket(`${base}/ws/${role}/${tatami}`);
    } catch (_) {
      scheduleRetry();
      return;
    }
    ws = sock;
    sock.onopen = () => {
      retryDelay = 500;
      lastMessageAt = Date.now();
      staleAskedAt = 0;
      onOpen?.(handle);
    };
    sock.onmessage = (ev) => {
      lastMessageAt = Date.now();
      staleAskedAt = 0;
      try {
        onMessage(JSON.parse(ev.data));
      } catch (_) {}
    };
    sock.onclose = (ev) => {
      if (ws !== sock) return; // an older socket we already replaced
      ws = null;
      if (!closed) {
        onClose?.(ev);
        scheduleRetry();
      }
    };
    sock.onerror = () => {
      try {
        sock.close();
      } catch (_) {}
    };
  }

  if (typeof window !== "undefined") {
    window.addEventListener("online", reconnectNow);
    document.addEventListener("visibilitychange", onVisible);
    if (options.staleMs && options.staleMs > 0) {
      const staleMs = options.staleMs;
      watchdog = setInterval(() => {
        if (closed || !ws || ws.readyState !== WebSocket.OPEN) return;
        const quiet = Date.now() - lastMessageAt;
        if (quiet < staleMs) return;
        if (!staleAskedAt) {
          staleAskedAt = Date.now();
          options.onStale?.(handle);
        } else if (Date.now() - staleAskedAt > 5000) {
          // Asked for fresh state and still heard nothing: the link is dead
          staleAskedAt = 0;
          const dead = ws;
          ws = null;
          try {
            dead.close();
          } catch (_) {}
          onClose?.();
          retryDelay = 500;
          connect();
        }
      }, 1000);
    }
  }

  connect();
  return handle;
}
