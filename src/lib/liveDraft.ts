/**
 * Draft of a running bout's live score (points, penalties, senshu, VR, timer, kata, judges),
 * kept in this computer's localStorage on every change. A bout's result only reaches the DB on
 * "Simpan", so a control panel closed by accident or an app crash used to lose the running score:
 * reopening the bout restores the draft instead. Cleared once the bout is saved.
 */

export interface LiveDraft {
  savedAt: number;
  akaScore: number;
  aoScore: number;
  akaSenshu: boolean;
  aoSenshu: boolean;
  akaPenalties: Record<string, boolean>;
  aoPenalties: Record<string, boolean>;
  hasVr1: boolean;
  hasVr2: boolean;
  isHantei: boolean;
  matchWinner: string;
  timerMs: number;
  kataAkaName: string;
  kataAoName: string;
  akaJudgeScores: string[];
  aoJudgeScores: string[];
  boardAkaScore: number;
  boardAoScore: number;
  isScorePublished: boolean;
  manualFlags: { aka: number; ao: number } | null;
}

const PREFIX = "ambrilindo.liveDraft.";
/** A draft older than this is from another day: not restored. */
const MAX_AGE_MS = 12 * 60 * 60 * 1000;
/** Timer-only changes are written at most this often (a point or penalty is written at once). */
const TIMER_WRITE_MS = 1000;

let lastKey = "";
let lastBody = "";
let lastWrite = 0;

function key(matchId: number): string {
  return `${PREFIX}${matchId}`;
}

export function saveLiveDraft(matchId: number, draft: Omit<LiveDraft, "savedAt">): void {
  if (!matchId || matchId <= 0) return;
  const { timerMs, ...rest } = draft;
  const body = JSON.stringify(rest);
  const now = Date.now();
  const k = key(matchId);
  if (k === lastKey && body === lastBody && now - lastWrite < TIMER_WRITE_MS) return;
  try {
    localStorage.setItem(k, JSON.stringify({ ...draft, timerMs, savedAt: now }));
    lastKey = k;
    lastBody = body;
    lastWrite = now;
  } catch {
    // storage full or blocked: scoring goes on without a draft
  }
}

export function loadLiveDraft(matchId: number): LiveDraft | null {
  if (!matchId || matchId <= 0) return null;
  try {
    const raw = localStorage.getItem(key(matchId));
    if (!raw) return null;
    const d = JSON.parse(raw) as LiveDraft;
    if (!d || typeof d.savedAt !== "number" || Date.now() - d.savedAt > MAX_AGE_MS) {
      localStorage.removeItem(key(matchId));
      return null;
    }
    return d;
  } catch {
    return null;
  }
}

export function clearLiveDraft(matchId: number): void {
  if (!matchId || matchId <= 0) return;
  try {
    localStorage.removeItem(key(matchId));
  } catch {
    // ignore
  }
  if (lastKey === key(matchId)) {
    lastKey = "";
    lastBody = "";
  }
}

/** True when the draft holds anything beyond a fresh, untouched bout. */
export function draftHasProgress(d: LiveDraft, fullTimerMs: number): boolean {
  const pen = (p: Record<string, boolean>) => Object.values(p || {}).some(Boolean);
  return (
    d.akaScore !== 0 ||
    d.aoScore !== 0 ||
    d.akaSenshu ||
    d.aoSenshu ||
    pen(d.akaPenalties) ||
    pen(d.aoPenalties) ||
    d.isHantei ||
    !!d.matchWinner ||
    d.timerMs !== fullTimerMs ||
    [...(d.akaJudgeScores || []), ...(d.aoJudgeScores || [])].some((s) => (s || "").trim() !== "")
  );
}
