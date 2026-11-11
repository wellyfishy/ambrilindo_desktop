/**
 * Referee (wasit & juri) helpers shared by the desktop Wasit & Juri channel and the web
 * Tatami Manager: seat rules, team → bout panel, conflicts, candidate ranking, photo
 * file-name matching and WebP conversion.
 */
import type { BoutPanel, BoutWasit, WasitItem, WasitTatamiAssignment } from "$lib/types";

export interface Seat {
  key: string;
  label: string;
  short: string;
}

/** Seats of a tatami team (desktop panel tab and web "Tim Tatami") */
export const TEAM_SEATS: Seat[] = [
  { key: "tatami_manager", label: "Tatami Manager", short: "TM" },
  { key: "referee", label: "Wasit (Referee)", short: "WASIT" },
  { key: "judge_1", label: "Juri 1", short: "J1" },
  { key: "judge_2", label: "Juri 2", short: "J2" },
  { key: "judge_3", label: "Juri 3", short: "J3" },
  { key: "judge_4", label: "Juri 4", short: "J4" },
  { key: "judge_5", label: "Juri 5", short: "J5" },
  { key: "judge_6", label: "Juri 6", short: "J6" },
  { key: "kansa", label: "Kansa", short: "KANSA" },
  { key: "score_supervisor", label: "Score Supervisor", short: "SS" },
  { key: "vr_wasit", label: "Wasit VR", short: "VR" },
];

export function seatLabel(key: string): string {
  return TEAM_SEATS.find((s) => s.key === key)?.label ?? (key === "pool" ? "Cadangan" : key);
}

export function isKataCategory(name: string | null | undefined): boolean {
  return /\bKATA\b/i.test(name || "");
}

/** Judge counts an operator can pick per kind of bout */
export const KUMITE_JUDGE_OPTIONS = [2, 4] as const;
export const KATA_JUDGE_OPTIONS = [3, 5, 7] as const;

/**
 * Judges of one bout: its own setting, else the tatami's default, else (kata) the
 * control panel's 3/5/7 setting; kumite 4 and kata 5 when nothing is set.
 */
export function boutJudgeCount(panel: Pick<BoutPanel, "category" | "judges" | "tatami_kumite_judges" | "tatami_kata_judges"> | null, liveKataJudges?: number | null): number {
  const kata = isKataCategory(panel?.category);
  const allowed: readonly number[] = kata ? KATA_JUDGE_OPTIONS : KUMITE_JUDGE_OPTIONS;
  const candidates = [panel?.judges, kata ? panel?.tatami_kata_judges : panel?.tatami_kumite_judges, kata ? liveKataJudges : null];
  for (const n of candidates) if (n && allowed.includes(n)) return n;
  return kata ? 5 : 4;
}

/**
 * Seats of one bout. Kumite: referee + 2 or 4 judges + kansa + score supervisor.
 * Kata: 3, 5 or 7 judges.
 */
export function boutSeats(category: string | null | undefined, judges?: number): Seat[] {
  const judge = (i: number): Seat => ({ key: `judge_${i}`, label: `Juri ${i}`, short: `J${i}` });
  if (isKataCategory(category)) {
    const n = judges && (KATA_JUDGE_OPTIONS as readonly number[]).includes(judges) ? judges : 5;
    return Array.from({ length: n }, (_, i) => judge(i + 1));
  }
  const n = judges === 2 ? 2 : 4;
  return [
    { key: "referee", label: "Wasit (Referee)", short: "WASIT" },
    ...Array.from({ length: n }, (_, i) => judge(i + 1)),
    { key: "kansa", label: "Kansa", short: "KANSA" },
    { key: "score_supervisor", label: "Score Supervisor", short: "SS" },
  ];
}

export type SeatHolder = Pick<BoutWasit, "wasit_id" | "nama_wasit" | "perguruan_id" | "nama_perguruan" | "kab_kota" | "lisensi" | "foto">;

function fromTeam(t: WasitTatamiAssignment): SeatHolder {
  return {
    wasit_id: t.wasit_id,
    nama_wasit: t.nama_wasit,
    perguruan_id: t.perguruan_id,
    nama_perguruan: t.nama_perguruan,
    kab_kota: t.kab_kota,
    lisensi: t.lisensi,
    foto: t.foto ?? null,
  };
}

/**
 * The panel a bout actually uses: its own panel when one was set, otherwise the tatami
 * team. Kata judges are taken from the team in order Referee, Juri 1, Juri 2, …
 */
export function effectivePanel(panel: BoutPanel, seats: Seat[]): { seats: Record<string, SeatHolder | null>; fromBout: boolean } {
  const out: Record<string, SeatHolder | null> = {};
  for (const s of seats) out[s.key] = null;
  if (panel.custom || panel.bout.length > 0) {
    for (const b of panel.bout) if (b.posisi in out) out[b.posisi] = b;
    return { seats: out, fromBout: true };
  }
  const team = panel.team.filter((t) => t.posisi !== "pool");
  if (isKataCategory(panel.category)) {
    const order = ["referee", "judge_1", "judge_2", "judge_3", "judge_4", "judge_5", "judge_6"];
    const holders = order.map((k) => team.find((t) => t.posisi === k)).filter((t): t is WasitTatamiAssignment => !!t);
    seats.forEach((s, i) => (out[s.key] = holders[i] ? fromTeam(holders[i]) : null));
  } else {
    for (const s of seats) {
      const t = team.find((x) => x.posisi === s.key);
      out[s.key] = t ? fromTeam(t) : null;
    }
  }
  return { seats: out, fromBout: false };
}

const norm = (s: string | null | undefined) =>
  (s || "")
    .toUpperCase()
    .replace(/^(KABUPATEN|KAB\.?|KOTA)\s+/, "")
    .replace(/[^A-Z0-9 ]/g, " ")
    .replace(/\s+/g, " ")
    .trim();

/** Same perguruan as an athlete, or the referee's kab/kota is an athlete's contingent. */
export function conflictReason(
  w: { nama_perguruan?: string | null; kab_kota?: string | null },
  panel: Pick<BoutPanel, "aka_perguruan" | "aka_utusan" | "ao_perguruan" | "ao_utusan"> | null
): string | null {
  if (!panel) return null;
  const perg = norm(w.nama_perguruan);
  const kota = norm(w.kab_kota);
  for (const [side, p, u] of [
    ["AKA", panel.aka_perguruan, panel.aka_utusan],
    ["AO", panel.ao_perguruan, panel.ao_utusan],
  ] as const) {
    if (perg && perg === norm(p)) return `Satu perguruan dengan ${side} (${w.nama_perguruan})`;
    const uk = norm(u);
    if (kota && uk && (uk === kota || uk.endsWith(" " + kota) || kota.endsWith(" " + uk))) return `Satu daerah dengan ${side} (${u})`;
  }
  return null;
}

export interface Candidate {
  wasit: WasitItem;
  rank: number;
  label: string;
  conflict: string | null;
}

/**
 * Picker order (as in Ambrilindo Local): this tatami's team ★, standby ★, on another
 * tatami (can be borrowed), then the same with a conflict ⚠. Inactive referees last.
 */
export function rankCandidates(
  wasits: WasitItem[],
  opts: { tatamiId: number | null; panel: BoutPanel | null; takenIds: Set<number> }
): Candidate[] {
  // get_wasits returns one row per tatami assignment: fold them per referee
  const byId = new Map<number, { w: WasitItem; tatamis: Set<number> }>();
  for (const w of wasits) {
    const e = byId.get(w.id) ?? { w, tatamis: new Set<number>() };
    if (w.assigned_tatami_id && w.assigned_posisi && w.assigned_posisi !== "pool") e.tatamis.add(w.assigned_tatami_id);
    byId.set(w.id, e);
  }
  const out: Candidate[] = [];
  for (const { w, tatamis } of byId.values()) {
    const conflict = conflictReason(w, opts.panel);
    let rank: number;
    let label: string;
    if (opts.tatamiId && tatamis.has(opts.tatamiId)) {
      rank = 0;
      label = "Tim tatami ini";
    } else if (tatamis.size === 0) {
      rank = 1;
      label = "Standby";
    } else {
      rank = 2;
      label = "Tatami lain (pinjam)";
    }
    if (conflict) rank += 3;
    if (!w.is_active) rank += 10;
    if (opts.takenIds.has(w.id)) {
      // picking one moves them from their seat: list after the free ones
      rank += 5;
      label = "Sudah di panel ini";
    }
    out.push({ wasit: w, rank, label, conflict });
  }
  return out.sort((a, b) => a.rank - b.rank || a.wasit.nama_wasit.localeCompare(b.wasit.nama_wasit));
}

// ---- photo file name → referee --------------------------------------------------

export function nameKey(s: string): string {
  return s
    .toLowerCase()
    .replace(/\.[a-z0-9]+$/, "") // extension
    .replace(/[^a-z0-9]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function levenshtein(a: string, b: string): number {
  const dp = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let prev = dp[0];
    dp[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const tmp = dp[j];
      dp[j] = Math.min(dp[j] + 1, dp[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = tmp;
    }
  }
  return dp[b.length];
}

/** 0..1 similarity of a photo file name to a referee name */
export function nameScore(file: string, name: string): number {
  const f = nameKey(file);
  const n = nameKey(name);
  if (!f || !n) return 0;
  if (f === n) return 1;
  // "samchizal" → "samchizal huzain", "totok subuh" → "totok subuh subianto"
  if (n.startsWith(f + " ")) return 0.95;
  if (f.startsWith(n + " ")) return 0.9;
  const fw = f.split(" ");
  const nw = n.split(" ");
  // every word of the file name is (almost) a word of the name: "abdul latief" ~ "abdul latif"
  const wordsMatch = fw.every((x) => nw.some((y) => y === x || (x.length >= 4 && y.length >= 4 && levenshtein(x, y) <= 1)));
  if (wordsMatch) return 0.88 - 0.02 * (nw.length - fw.length);
  return 1 - levenshtein(f, n) / Math.max(f.length, n.length);
}

/**
 * 0..1 how well a typed search matches a referee name: full name or typo'd words
 * ("surya darma"), and word starts ("sury", "m rif").
 */
export function searchScore(query: string, name: string): number {
  const q = nameKey(query);
  const n = nameKey(name);
  if (!q || !n) return 0;
  const words = n.split(" ");
  const qw = q.split(" ");
  let s = nameScore(q, n);
  if (qw.every((x) => words.some((y) => y.startsWith(x)))) s = Math.max(s, 0.86 - 0.01 * (words.length - qw.length));
  if (n.includes(q)) s = Math.max(s, 0.8);
  return s;
}

export interface PhotoMatch {
  wasitId: number | null;
  score: number;
  /** a second referee scored almost as well */
  ambiguous: boolean;
}

export function matchPhoto(fileName: string, wasits: { id: number; nama_wasit: string }[]): PhotoMatch {
  const scored = wasits
    .map((w) => ({ id: w.id, s: nameScore(fileName, w.nama_wasit) }))
    .sort((a, b) => b.s - a.s);
  const best = scored[0];
  if (!best || best.s < 0.8) return { wasitId: null, score: best?.s ?? 0, ambiguous: false };
  const ambiguous = !!scored[1] && scored[1].s >= best.s - 0.03 && best.s < 1;
  return { wasitId: ambiguous ? null : best.id, score: best.s, ambiguous };
}

// ---- photo conversion ----------------------------------------------------------

export const PHOTO_MAX_SIDE = 600;

/**
 * Any browser-readable image → WebP data URL, longest side ≤ 600 px. A WebP that is
 * already small enough is kept as is.
 */
export async function photoToWebp(file: File, maxSide = PHOTO_MAX_SIDE, quality = 0.85): Promise<{ dataUrl: string; width: number; height: number }> {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, maxSide / Math.max(bitmap.width, bitmap.height));
  const width = Math.max(1, Math.round(bitmap.width * scale));
  const height = Math.max(1, Math.round(bitmap.height * scale));
  if (file.type === "image/webp" && scale === 1) {
    bitmap.close();
    return { dataUrl: await readAsDataUrl(file), width, height };
  }
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas tidak tersedia");
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, 0, 0, width, height);
  bitmap.close();
  const blob: Blob | null = await new Promise((r) => canvas.toBlob(r, "image/webp", quality));
  if (!blob || blob.type !== "image/webp") throw new Error("Browser tidak bisa membuat WebP");
  return { dataUrl: await readAsDataUrl(blob), width, height };
}

function readAsDataUrl(b: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(b);
  });
}

export function fileToBase64(file: File): Promise<string> {
  return readAsDataUrl(file);
}
