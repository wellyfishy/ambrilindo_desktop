// Kumite judge sticks (mouse) scoring, computed entirely on the operator's laptop.
//
// Each of the 4 judges holds a controller whose buttons are mapped (by the
// mouse-to-keyboard tool) to the letters a–x. The control panel window receives
// those keystrokes; nothing goes to the server except the resulting banner.
// Ported from Ambrilindo Local `backend/static/mouse.js`.

export type Side = "aka" | "ao";
export type Level = 1 | 2 | 3;

export interface StickKey {
  judge: 1 | 2 | 3 | 4;
  side: Side;
  level: Level;
}

/** Letter → judge / corner / point value (same layout as Ambrilindo Local) */
export const KEY_MAP: Record<string, StickKey> = {
  a: { judge: 1, side: "aka", level: 1 }, b: { judge: 1, side: "aka", level: 2 }, c: { judge: 1, side: "aka", level: 3 },
  d: { judge: 1, side: "ao", level: 1 }, e: { judge: 1, side: "ao", level: 2 }, f: { judge: 1, side: "ao", level: 3 },
  g: { judge: 2, side: "aka", level: 1 }, h: { judge: 2, side: "aka", level: 2 }, i: { judge: 2, side: "aka", level: 3 },
  j: { judge: 2, side: "ao", level: 1 }, k: { judge: 2, side: "ao", level: 2 }, l: { judge: 2, side: "ao", level: 3 },
  n: { judge: 3, side: "aka", level: 1 }, m: { judge: 3, side: "aka", level: 2 }, o: { judge: 3, side: "aka", level: 3 },
  p: { judge: 3, side: "ao", level: 1 }, q: { judge: 3, side: "ao", level: 2 }, r: { judge: 3, side: "ao", level: 3 },
  s: { judge: 4, side: "aka", level: 1 }, t: { judge: 4, side: "aka", level: 2 }, u: { judge: 4, side: "aka", level: 3 },
  v: { judge: 4, side: "ao", level: 1 }, w: { judge: 4, side: "ao", level: 2 }, x: { judge: 4, side: "ao", level: 3 },
};

/** Letters per judge in display order: AKA 1,2,3 then AO 1,2,3 */
export const JUDGE_KEYS: string[][] = [
  ["a", "b", "c", "d", "e", "f"],
  ["g", "h", "i", "j", "k", "l"],
  ["n", "m", "o", "p", "q", "r"],
  ["s", "t", "u", "v", "w", "x"],
];

export const LEVEL_LABEL: Record<Level, [string, string]> = {
  1: ["1 - POINT", "YUKO"],
  2: ["2 - POINT", "WAZA-ARI"],
  3: ["3 - POINT", "IPPON"],
};

export const LEVEL_SHORT: Record<Level, string> = { 1: "YUKO", 2: "WAZA", 3: "IPPON" };

/** How long one press counts (ms) */
export const VOTE_TTL_MS = 3000;
/** Gap after which the next press starts a new exchange (ms) */
export const INCIDENT_GAP_MS = 4000;

/**
 * Point awarded to one corner from the levels its judges pressed.
 * - The highest value pressed by at least 2 judges wins: (3,1,1) → 1, (2,2,3) → 2.
 * - If no value is shared but at least 2 judges pressed for this corner, the
 *   highest one wins: (1,3) → 3, (2,1) → 2.
 * - A single judge gives nothing.
 */
export function evaluateSide(levels: number[]): 0 | Level {
  if (levels.length < 2) return 0;
  for (const lv of [3, 2, 1] as Level[]) {
    if (levels.filter((l) => l === lv).length >= 2) return lv;
  }
  return Math.max(...levels) as Level;
}

export interface SideResult {
  level: 0 | Level;
  /** number of judges who pressed for this corner */
  judges: number;
}

export interface StickState {
  /** Active vote per judge (index 0 = J1), null when none */
  votes: ({ side: Side; level: Level; key: string } | null)[];
  aka: SideResult;
  ao: SideResult;
  /** Hantei tally: judges currently voting for each corner */
  hantei: { aka: number; ao: number };
  /** true when this update comes from a vote expiring, not from a press */
  isDecay: boolean;
  /** Increments when a press arrives after INCIDENT_GAP_MS of silence */
  incidentId: number;
  /** Letter of the press that caused this update (null on decay) */
  lastKey: string | null;
}

/** One exchange in the monitor's history (updated in place while it lasts) */
export interface StickHistoryRow {
  incidentId: number;
  time: string;
  votes: StickState["votes"];
  result: string;
  tone: "aka" | "ao" | "seri" | "none";
}

export class StickTracker {
  private votes: ({ side: Side; level: Level; key: string; expiresAt: number } | null)[] = [null, null, null, null];
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastPressAt = 0;
  private incidentId = 0;
  /** Hantei: votes stay until reset instead of expiring after VOTE_TTL_MS */
  private hold = false;

  constructor(private onChange: (state: StickState) => void) {}

  /** Feed one keystroke. Returns false when the letter is not a judge button. */
  press(rawKey: string): boolean {
    const key = rawKey.toLowerCase();
    const info = KEY_MAP[key];
    if (!info) return false;
    const now = Date.now();
    if (now - this.lastPressAt > INCIDENT_GAP_MS) this.incidentId += 1;
    this.lastPressAt = now;
    // One active vote per judge: a new press replaces the judge's previous one
    this.votes[info.judge - 1] = {
      side: info.side,
      level: info.level,
      key,
      expiresAt: this.hold ? Number.POSITIVE_INFINITY : now + VOTE_TTL_MS,
    };
    this.emit(false, key);
    this.schedule();
    return true;
  }

  /** Hantei mode: each judge's last vote is kept (no expiry) until reset() */
  setHold(on: boolean) {
    if (this.hold === on) return;
    this.hold = on;
    this.reset();
  }

  reset() {
    this.votes = [null, null, null, null];
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.lastPressAt = 0;
  }

  dispose() {
    this.reset();
  }

  private schedule() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    const pending = this.votes.filter((v) => v !== null && Number.isFinite(v.expiresAt)).map((v) => v!.expiresAt);
    if (pending.length === 0) return;
    const wait = Math.max(10, Math.min(...pending) - Date.now());
    this.timer = setTimeout(() => {
      this.timer = null;
      const now = Date.now();
      this.votes = this.votes.map((v) => (v && v.expiresAt <= now ? null : v));
      this.emit(true, null);
      this.schedule();
    }, wait);
  }

  private emit(isDecay: boolean, lastKey: string | null) {
    const akaLevels: number[] = [];
    const aoLevels: number[] = [];
    for (const v of this.votes) {
      if (!v) continue;
      (v.side === "aka" ? akaLevels : aoLevels).push(v.level);
    }
    this.onChange({
      votes: this.votes.map((v) => (v ? { side: v.side, level: v.level, key: v.key } : null)),
      aka: { level: evaluateSide(akaLevels), judges: akaLevels.length },
      ao: { level: evaluateSide(aoLevels), judges: aoLevels.length },
      hantei: { aka: akaLevels.length, ao: aoLevels.length },
      isDecay,
      incidentId: this.incidentId,
      lastKey,
    });
  }
}
