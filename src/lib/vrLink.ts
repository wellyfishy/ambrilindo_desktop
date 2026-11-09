// Shared link for the Video Review tablets (coach page and VR referee page).
//
// The operator's control panel owns the VR state (open requests per corner,
// which cards are used up, scores) and sends it in every live state. These
// tablets only draw that state and send commands:
//   coach   → request (side + value 1/2/3) or cancel
//   referee → decision (accepted / rejected / minai)
// Commands go to the host server (reaches the control panel on any device) and,
// on the same machine, over BroadcastChannel too; the control panel ignores the
// duplicate.

import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
import { getHostBaseUrl } from "$lib/api_client";

export type VrSide = "aka" | "ao";
export type VrStatus = "in_progress" | "accepted" | "rejected" | "minai";

export interface VrRequest {
  side: VrSide;
  status: VrStatus;
  /** value asked for (in progress) or awarded (accepted) */
  points?: number;
  timestamp: number;
}

export interface VrCorner {
  name: string;
  club: string;
  score: number;
  /** VR card already used up (hangus) */
  cardUsed: boolean;
}

export interface VrView {
  tatami: number;
  matchId: number | null;
  category: string;
  round: string;
  matchNumber: string;
  timerMs: number;
  isTimerRunning: boolean;
  /** this match uses Video Review (kumite category with VR) */
  vrEnabled: boolean;
  isKata: boolean;
  aka: VrCorner;
  ao: VrCorner;
  requests: { aka: VrRequest | null; ao: VrRequest | null };
  /** true once any live state from the control panel has arrived */
  hasData: boolean;
}

export const POINT_NAME: Record<number, string> = { 1: "YUKO", 2: "WAZA-ARI", 3: "IPPON" };

export function emptyView(tatami: number): VrView {
  return {
    tatami,
    matchId: null,
    category: "",
    round: "",
    matchNumber: "",
    timerMs: 0,
    isTimerRunning: false,
    vrEnabled: false,
    isKata: false,
    aka: { name: "AKA", club: "-", score: 0, cardUsed: false },
    ao: { name: "AO", club: "-", score: 0, cardUsed: false },
    requests: { aka: null, ao: null },
    hasData: false,
  };
}

function readRequest(raw: any, side: VrSide): VrRequest | null {
  if (!raw || typeof raw !== "object") return null;
  const status = String(raw.status || "") as VrStatus;
  if (!["in_progress", "accepted", "rejected", "minai"].includes(status)) return null;
  const points = Number(raw.points) || undefined;
  return { side, status, points, timestamp: Number(raw.timestamp) || 0 };
}

/** Merge one live state message (full or partial) into the view */
export function mergeLiveState(view: VrView, data: any): VrView {
  if (!data || typeof data !== "object") return view;
  const next: VrView = {
    ...view,
    aka: { ...view.aka },
    ao: { ...view.ao },
    requests: { ...view.requests },
    hasData: true,
  };
  if (data.matchId !== undefined) next.matchId = data.matchId === null ? null : Number(data.matchId);
  if (data.category) next.category = String(data.category);
  if (data.round) next.round = String(data.round);
  if (data.matchNumber !== undefined) next.matchNumber = String(data.matchNumber);
  if (data.timerMs !== undefined) next.timerMs = Number(data.timerMs) || 0;
  if (data.isTimerRunning !== undefined) next.isTimerRunning = !!data.isTimerRunning;
  if (data.vrEnabled !== undefined) next.vrEnabled = !!data.vrEnabled;
  if (data.isKata !== undefined) next.isKata = !!data.isKata;
  for (const side of ["aka", "ao"] as const) {
    const c = data[side];
    if (c && typeof c === "object") {
      if (c.name) next[side].name = String(c.name);
      if (c.contingent !== undefined) next[side].club = String(c.contingent || "-");
      if (c.score !== undefined) next[side].score = Number(c.score) || 0;
      if (c.vr !== undefined) next[side].cardUsed = !!c.vr;
    }
  }
  if (data.vrChallenges && typeof data.vrChallenges === "object") {
    next.requests = {
      aka: readRequest(data.vrChallenges.aka, "aka"),
      ao: readRequest(data.vrChallenges.ao, "ao"),
    };
  } else if (data.vrChallenge !== undefined) {
    // Older control panel: a single request
    const single = data.vrChallenge;
    const side: VrSide = single?.side === "ao" ? "ao" : "aka";
    next.requests = { aka: null, ao: null };
    if (single) next.requests[side] = readRequest(single, side);
  }
  return next;
}

export type LinkStatus = "connecting" | "live" | "offline";

export interface VrLink {
  close: () => void;
  /** ask the control panel for its current state now */
  refresh: () => void;
}

/**
 * Keeps a tablet in sync with its tatami: WebSocket to the host (reconnects on
 * its own and asks for the latest state each time), plus BroadcastChannel when
 * the page runs on the host machine, plus an HTTP fetch at start.
 */
export function connectVrLink(
  role: string,
  tatami: number,
  onState: (data: any) => void,
  onStatus: (status: LinkStatus) => void
): VrLink {
  let socket: TatamiSocket | null = null;
  let bc: BroadcastChannel | null = null;

  const offlineOr = (s: LinkStatus): LinkStatus =>
    typeof navigator !== "undefined" && navigator.onLine === false ? "offline" : s;

  async function fetchHttp() {
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/tatami/${tatami}/live-state?_t=${Date.now()}`, { cache: "no-store" });
      if (res.ok) {
        const json = await res.json();
        if (json?.data) onState(json.data);
      }
    } catch (_) {}
  }

  const ask = (s: TatamiSocket) => s.send({ type: "request_tatami_state", tatami });

  socket = connectTatamiSocket(
    role,
    tatami,
    (msg) => {
      onStatus("live");
      if (msg?.type === "tatami_live_state") {
        const data = msg.payload || msg.data;
        if (data && (data.tatami === undefined || Number(data.tatami) === tatami)) onState(data);
      }
    },
    (s) => {
      onStatus("live");
      // Every (re)connect: get the latest state straight away
      ask(s);
    },
    () => onStatus(offlineOr("connecting")),
    { staleMs: 15000, onStale: ask }
  );

  if (typeof BroadcastChannel !== "undefined") {
    bc = new BroadcastChannel("ambrilindo_live_sync");
    bc.onmessage = (ev) => {
      const data = ev.data;
      if (!data || typeof data !== "object") return;
      if (data.tatami !== undefined && Number(data.tatami) !== tatami) return;
      // Live state from the control panel on this machine (VR commands are ignored:
      // their result arrives in the next live state)
      if (data.type === undefined || data.type === "tatami_live_state") {
        onState(data.payload || data);
      }
    };
  }

  onStatus(offlineOr("connecting"));
  fetchHttp();

  return {
    close() {
      socket?.close();
      bc?.close();
    },
    refresh() {
      if (socket) ask(socket);
      fetchHttp();
    },
  };
}

function postLocal(msg: any) {
  if (typeof BroadcastChannel === "undefined") return;
  try {
    const ch = new BroadcastChannel("ambrilindo_live_sync");
    ch.postMessage(msg);
    ch.close();
  } catch (_) {}
}

async function postServer(path: string, body: any) {
  try {
    await fetch(`${getHostBaseUrl()}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  } catch (_) {}
}

/** Coach: ask for a review of `points` (1/2/3) for a corner */
export function sendVrRequest(tatami: number, side: VrSide, points: number) {
  const reason = `${POINT_NAME[points] ?? ""} (${points} POINT)`;
  postLocal({ type: "vr_challenge_started", tatami, side, points, reason, timestamp: Date.now() });
  return postServer("/api/vr/challenge", { tatami, side, points, reason });
}

/** Coach cancel, or the referee's decision */
export function sendVrDecision(
  tatami: number,
  side: VrSide,
  decision: "accepted" | "rejected" | "minai" | "cancelled",
  points?: number
) {
  postLocal({ type: "vr_challenge_resolved", tatami, side, decision, points, timestamp: Date.now() });
  return postServer("/api/vr/resolve", { tatami, side, decision, points });
}

export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
}
