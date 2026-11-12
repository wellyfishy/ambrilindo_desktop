/**
 * Timetable (jadwal pertandingan) time model and the auto-scheduler.
 *
 * One time model for everything: the grid, the printed roster, the result banner and
 * the scheduler all compute start/end times with `computeColumnTimes`, so the times the
 * scheduler plans with are exactly the times the operator sees.
 *
 * A tatami column runs top to bottom from the day's start time:
 * - a category takes its estimated duration;
 * - a text cell takes the minutes written in it ("JEDA 12 MENIT", "SESI FESTIVAL (90 MENIT)"),
 *   otherwise 60 for ISHOMA / ISTIRAHAT / BREAK, 15 for JEDA and 45 for anything else;
 * - a label row (whole-day break, e.g. "12:00 - 13:00") resumes at the end of its window,
 *   or after the window's length when the tatami reaches it late.
 * Only `custom_text` is stored for text cells, which is why durations live in the text.
 */
import type {
  AthleteConflictDetail,
  EnrichedTimetableCell,
  EnrichedTimetableRow,
  TimetableCategoryOption,
} from "$lib/types";

export const LUNCH_MINUTES = 60;
const JEDA_DEFAULT_MINUTES = 15;
const CUSTOM_DEFAULT_MINUTES = 45;
const LUNCH_FROM = 12 * 60;
/** A category that starts after this may not run past LUNCH_LATEST_END. */
const LUNCH_GUARD_FROM = 11 * 60;
const LUNCH_EARLIEST = 11 * 60 + 30;
const LUNCH_LATEST_END = 12 * 60 + 30;

export function parseTimeToMinutes(time: string): number {
  const parts = (time || "08:30").split(/[:.]/);
  const h = parseInt(parts[0], 10);
  const m = parseInt(parts[1], 10);
  return (Number.isFinite(h) ? h : 8) * 60 + (Number.isFinite(m) ? m : 0);
}

export function formatMinutesToTime(min: number): string {
  const h = Math.floor(min / 60) % 24;
  const m = Math.floor(min % 60);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

// ---------------------------------------------------------------------------
// Text cells (same keywords the backend uses to flag festival/break cells)
// ---------------------------------------------------------------------------

export function isFestivalText(text: string): boolean {
  return /FESTIVAL/i.test(text || "");
}

export function isJedaText(text: string): boolean {
  return /JEDA/i.test(text || "");
}

export function isLunchText(text: string): boolean {
  return /ISHOMA|ISTIRAHAT|BREAK/i.test(text || "");
}

export function isBreakText(text: string): boolean {
  return !isFestivalText(text) && (isLunchText(text) || isJedaText(text));
}

export function customTextMinutes(text: string): number {
  const m = (text || "").match(/(\d+)\s*MENIT/i);
  if (m) return parseInt(m[1], 10);
  if (isFestivalText(text)) return CUSTOM_DEFAULT_MINUTES;
  if (isLunchText(text)) return LUNCH_MINUTES;
  if (isJedaText(text)) return JEDA_DEFAULT_MINUTES;
  return CUSTOM_DEFAULT_MINUTES;
}

/** Athlete count written in a text cell, e.g. "FESTIVAL (24 Atlet)". */
export function customTextAthletes(text: string): number {
  const m = (text || "").match(/(\d+)\s*ATLET/i);
  return m ? parseInt(m[1], 10) : 0;
}

/** Bout count written in a text cell, e.g. "FESTIVAL (24 ATLET, 12 PARTAI, 50 MENIT)". */
export function customTextMatches(text: string): number {
  const m = (text || "").match(/(\d+)\s*PARTAI/i);
  return m ? parseInt(m[1], 10) : 0;
}

/**
 * Text of a roster-format festival session cell. Only custom_text is stored, so the numbers
 * the time model needs (athletes, bouts, minutes) are written into it; the grid and the print
 * show it as plain "FESTIVAL".
 */
export function festivalSessionText(athletes: number, matches: number, minutes: number): string {
  return `FESTIVAL (${athletes} ATLET, ${matches} PARTAI, ${minutes} MENIT)`;
}

/** Break window of a label row; "12:00 - 13:00" when its time label has none. */
export function labelRowWindow(row: Pick<EnrichedTimetableRow, "time_label">): { start: number; end: number } {
  const m = (row.time_label || "").match(/(\d{1,2})[:.](\d{2})\s*[-–—]\s*(\d{1,2})[:.](\d{2})/);
  if (m) {
    const start = +m[1] * 60 + +m[2];
    const end = +m[3] * 60 + +m[4];
    if (end > start) return { start, end };
  }
  return { start: LUNCH_FROM, end: LUNCH_FROM + LUNCH_MINUTES };
}

function labelRowResume(cur: number, row: EnrichedTimetableRow): number {
  const w = labelRowWindow(row);
  return cur <= w.start ? w.end : cur + (w.end - w.start);
}

/** Minutes a cell occupies on its tatami, or null for an empty cell. */
export function cellMinutes(cell: EnrichedTimetableCell | undefined | null): number | null {
  if (!cell) return null;
  if (cell.nomor_tanding_id) return Math.max(0, cell.est_duration_minutes || 0);
  if (cell.custom_text) return customTextMinutes(cell.custom_text);
  return null;
}

export interface ColumnTimes {
  /** Start/end minutes per row index (only rows where this tatami has something). */
  slots: Record<number, { start: number; end: number }>;
  finish: number;
  matches: number;
  athletes: number;
}

export function computeColumnTimes(rows: EnrichedTimetableRow[], tatamiId: number, startMin: number): ColumnTimes {
  const slots: ColumnTimes["slots"] = {};
  let cur = startMin;
  let matches = 0;
  let athletes = 0;
  rows.forEach((row, rIdx) => {
    if (row.row_type === "label") {
      cur = labelRowResume(cur, row);
      return;
    }
    const cell = row.cells.find((c) => c.tatami_id === tatamiId);
    const dur = cellMinutes(cell);
    if (!cell || dur === null) return;
    slots[rIdx] = { start: cur, end: cur + dur };
    cur += dur;
    if (cell.nomor_tanding_id) {
      matches += cell.matches || 0;
      // a final on its own plays the winners of the two pools: no new athletes
      if (cell.part !== "final") athletes += cell.atlet_count || 0;
    } else if (!cell.is_break) {
      const n = cell.atlet_count || customTextAthletes(cell.custom_text);
      athletes += n;
      matches += cell.matches || customTextMatches(cell.custom_text) || n;
    }
  });
  return { slots, finish: cur, matches, athletes };
}

// ---------------------------------------------------------------------------
// Athlete clashes
// ---------------------------------------------------------------------------

/** Clashes between cells of one session (same row, different tatami). */
export function rowConflicts(cells: EnrichedTimetableCell[], ntAthletes: Record<number, string[]>): AthleteConflictDetail[] {
  const out: AthleteConflictDetail[] = [];
  for (let i = 0; i < cells.length; i++) {
    for (let j = i + 1; j < cells.length; j++) {
      const a = cells[i];
      const b = cells[j];
      if (a.tatami_id === b.tatami_id || !a.nomor_tanding_id || !b.nomor_tanding_id) continue;
      if (a.nomor_tanding_id === b.nomor_tanding_id) continue; // pool atas + pool bawah of one category
      const setB = new Set(ntAthletes[b.nomor_tanding_id] || []);
      for (const ath of ntAthletes[a.nomor_tanding_id] || []) {
        if (setB.has(ath)) {
          out.push({
            athlete_name: ath,
            tatami1_number: a.tatami_number,
            category1_id: a.nomor_tanding_id,
            category1_name: a.nomor_tanding_nama || "",
            tatami2_number: b.tatami_number,
            category2_id: b.nomor_tanding_id,
            category2_name: b.nomor_tanding_nama || "",
          });
        }
      }
    }
  }
  return out;
}

export interface TimeClash {
  athlete: string;
  a: { tatamiNumber: number; categoryId: number; categoryName: string; start: number; end: number };
  b: { tatamiNumber: number; categoryId: number; categoryName: string; start: number; end: number };
}

/** Athletes due on two tatamis at overlapping times, using the grid's own times. */
export function findTimeClashes(
  rows: EnrichedTimetableRow[],
  tatamiIds: number[],
  startMin: number,
  ntAthletes: Record<number, string[]>
): TimeClash[] {
  const items: { cell: EnrichedTimetableCell; start: number; end: number }[] = [];
  for (const tid of tatamiIds) {
    const { slots } = computeColumnTimes(rows, tid, startMin);
    for (const [rIdx, t] of Object.entries(slots)) {
      const cell = rows[+rIdx].cells.find((c) => c.tatami_id === tid);
      if (cell?.nomor_tanding_id && t.end > t.start) items.push({ cell, start: t.start, end: t.end });
    }
  }
  const out: TimeClash[] = [];
  for (let i = 0; i < items.length; i++) {
    for (let j = i + 1; j < items.length; j++) {
      const x = items[i];
      const y = items[j];
      if (x.cell.tatami_id === y.cell.tatami_id || !(x.start < y.end && y.start < x.end)) continue;
      if (x.cell.nomor_tanding_id === y.cell.nomor_tanding_id) continue; // split category: different halves
      const setY = new Set(ntAthletes[y.cell.nomor_tanding_id!] || []);
      for (const ath of ntAthletes[x.cell.nomor_tanding_id!] || []) {
        if (!setY.has(ath)) continue;
        const side = (it: typeof x) => ({
          tatamiNumber: it.cell.tatami_number,
          categoryId: it.cell.nomor_tanding_id!,
          categoryName: it.cell.nomor_tanding_nama || "",
          start: it.start,
          end: it.end,
        });
        out.push({ athlete: ath, a: side(x), b: side(y) });
      }
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Category ordering
// ---------------------------------------------------------------------------

export const AGE_GROUP_ORDER = [
  "Pra Usia Dini",
  "Usia Dini",
  "Pra Pemula",
  "Pemula",
  "Kadet",
  "Junior",
  "Under-21",
  "Senior",
  "Veteran",
  "Lainnya",
];

export function getAgeGroupRank(ageGroup: string, categoryName = ""): number {
  const s = `${ageGroup || ""} ${categoryName || ""}`.toLowerCase();
  if (s.includes("pra usia dini")) return 0;
  if (s.includes("usia dini")) return 1;
  if (s.includes("pra pemula")) return 2;
  if (s.includes("pemula")) return 3;
  if (s.includes("kadet") || s.includes("cadet")) return 4;
  if (s.includes("junior")) return 5;
  if (s.includes("under-21") || s.includes("under 21") || s.includes("u-21") || s.includes("u21")) return 6;
  if (s.includes("senior")) return 7;
  if (s.includes("veteran")) return 8;
  return 9;
}

function parseWeightClass(name: string): number {
  const m = name.match(/([+-]?\d+)\s*kg/i);
  if (!m) return 999;
  const num = parseInt(m[1].replace(/[^\d]/g, ""), 10) || 0;
  return m[1].includes("+") ? 1000 + num : num;
}

function isKataCategory(c: Pick<TimetableCategoryOption, "discipline" | "nama">): boolean {
  return c.discipline === "kata" || c.nama.toLowerCase().includes("kata");
}

function categoryTuple(c: TimetableCategoryOption) {
  const lower = c.nama.toLowerCase();
  const isKata = isKataCategory(c);
  let gender = 2;
  if (/putra|pria|\bpa\b/.test(lower)) gender = 0;
  else if (/putri|wanita|\bpi\b/.test(lower)) gender = 1;
  return {
    disc: isKata ? 0 : 1,
    age: getAgeGroupRank(c.age_group, c.nama),
    team: lower.includes("beregu") || lower.includes("team") ? 1 : 0,
    gender,
    weight: isKata ? 0 : parseWeightClass(c.nama),
  };
}

/**
 * - kata_then_kumite: all Kata (young to old), then all Kumite (young to old).
 * - age_asc / age_desc: one age group at a time (Kata, then Kumite), young to old / old to young.
 */
export type AutoSortMode = "kata_then_kumite" | "age_asc" | "age_desc";

export const AUTO_SORT_MODES: { value: AutoSortMode; label: string }[] = [
  { value: "kata_then_kumite", label: "Kata dulu, lalu Kumite (usia muda → tua)" },
  { value: "age_asc", label: "Per kelompok usia, muda → tua (Kata lalu Kumite)" },
  { value: "age_desc", label: "Per kelompok usia, tua → muda (Kata lalu Kumite)" },
];

/** Stage = the "wave" a category belongs to; a stage starts only after the previous one is fully placed. */
function stageOf(t: ReturnType<typeof categoryTuple>, mode: AutoSortMode): number {
  if (mode === "age_asc") return t.age * 10 + t.disc;
  if (mode === "age_desc") return (99 - t.age) * 10 + t.disc;
  return t.disc * 100 + t.age;
}

// ---------------------------------------------------------------------------
// Auto-scheduler
// ---------------------------------------------------------------------------

export interface AutoScheduleOptions {
  startMin: number;
  sortMode: AutoSortMode;
  /** Schedule festival categories; with none in the event, open the day with a festival session. */
  includeFestival: boolean;
  festivalSessionMinutes: number;
  /**
   * Roster-format festival: one session per tatami (tatamis absent here get an empty cell).
   * Festival categories are then never scheduled one by one.
   */
  festivalSession?: { tatamiId: number; text: string; minutes: number }[] | null;
  /** A festival session already sits on another day: do not add a second one. */
  festivalSessionElsewhere?: boolean;
  includeLunch: boolean;
  /** Within a stage, start the longest categories first so tatamis finish closer together. */
  balanceFinish: boolean;
  preventClashes: boolean;
  /** true: rebuild the whole day. false: keep the day and append categories not scheduled yet. */
  rebuildDay: boolean;
  isAgeGroupSelected: (c: TimetableCategoryOption) => boolean;
}

export interface AutoScheduleInput {
  dayId: number;
  categories: TimetableCategoryOption[];
  tatamis: { id: number; tatami_number: number | null }[];
  dayRows: EnrichedTimetableRow[];
  /** Categories placed on other days (never moved or duplicated). */
  assignedOnOtherDays: Set<number>;
  ntAthletes: Record<number, string[]>;
}

export interface AutoScheduleResult {
  rows: EnrichedTimetableRow[];
  scheduled: number;
  /** A roster-format festival session was added. */
  festivalSessionAdded: boolean;
  skippedEmpty: TimetableCategoryOption[];
  /** Categories that could only be placed by accepting an athlete clash. */
  forced: number;
  clashes: TimeClash[];
  perTatami: { tatamiId: number; tatamiNumber: number; matches: number; finish: number }[];
}

function blankCell(tatamiId: number, tatamiNumber: number): EnrichedTimetableCell {
  return {
    id: 0,
    row_id: 0,
    tatami_id: tatamiId,
    tatami_number: tatamiNumber,
    nomor_tanding_id: null,
    nomor_tanding_nama: null,
    custom_text: "",
    atlet_count: 0,
    matches: 0,
    discipline: "kumite",
    est_duration_minutes: 0,
    progress_status: "belum_disentuh",
    progress_pct: 0,
    finished_matches: 0,
    remaining_matches: 0,
    is_festival: false,
    is_break: false,
  };
}

export function textCell(tatamiId: number, tatamiNumber: number, text: string): EnrichedTimetableCell {
  const cell = blankCell(tatamiId, tatamiNumber);
  cell.custom_text = text;
  cell.is_festival = isFestivalText(text);
  cell.is_break = isBreakText(text);
  cell.discipline = "kata";
  cell.est_duration_minutes = customTextMinutes(text);
  cell.progress_status = cell.is_festival ? "festival" : cell.is_break ? "break" : "belum_disentuh";
  return cell;
}

function categoryCell(tatamiId: number, tatamiNumber: number, c: TimetableCategoryOption): EnrichedTimetableCell {
  const cell = blankCell(tatamiId, tatamiNumber);
  cell.nomor_tanding_id = c.id;
  cell.nomor_tanding_nama = c.nama;
  cell.atlet_count = c.atlet_count;
  cell.matches = c.matches;
  cell.discipline = c.discipline;
  cell.est_duration_minutes = c.est_duration_minutes;
  cell.progress_status = c.progress_status;
  cell.progress_pct = c.progress_pct;
  cell.remaining_matches = c.matches;
  cell.is_festival = c.is_festival;
  return cell;
}

interface Candidate {
  cat: TimetableCategoryOption;
  stage: number;
  rank: number;
  dur: number;
  athletes: string[];
}

export function runAutoSchedule(input: AutoScheduleInput, opt: AutoScheduleOptions): AutoScheduleResult {
  const { tatamis, ntAthletes } = input;
  const tNum = (tid: number) => tatamis.find((t) => t.id === tid)?.tatami_number ?? 1;
  const kept: EnrichedTimetableRow[] = opt.rebuildDay ? [] : input.dayRows;

  const onThisDay = new Set<number>();
  input.dayRows.forEach((r) => r.cells.forEach((c) => c.nomor_tanding_id && onThisDay.add(c.nomor_tanding_id)));

  const eligible = input.categories.filter(
    (c) =>
      !input.assignedOnOtherDays.has(c.id) &&
      (opt.rebuildDay || !onThisDay.has(c.id)) &&
      (!c.is_festival || (opt.includeFestival && !opt.festivalSession)) &&
      opt.isAgeGroupSelected(c)
  );
  const skippedEmpty = eligible.filter((c) => c.atlet_count <= 0);

  const sorted = eligible
    .filter((c) => c.atlet_count > 0)
    .map((c) => ({ c, t: categoryTuple(c) }))
    .sort((x, y) => {
      const sx = stageOf(x.t, opt.sortMode);
      const sy = stageOf(y.t, opt.sortMode);
      return sx - sy || x.t.team - y.t.team || x.t.gender - y.t.gender || x.t.weight - y.t.weight || x.c.id - y.c.id;
    });
  let pool: Candidate[] = sorted.map(({ c, t }, rank) => ({
    cat: c,
    stage: stageOf(t, opt.sortMode),
    rank,
    dur: c.est_duration_minutes > 0 ? c.est_duration_minutes : 15,
    athletes: ntAthletes[c.id] || [],
  }));

  // Per-tatami state; new cells are appended below the kept rows.
  const ids = tatamis.map((t) => t.id);
  const columns: Record<number, EnrichedTimetableCell[]> = {};
  const clock: Record<number, number> = {};
  const matches: Record<number, number> = {};
  const lunchDone: Record<number, boolean> = {};
  const intervals: { tid: number; start: number; end: number; athletes: Set<string> }[] = [];

  for (const tid of ids) {
    columns[tid] = [];
    const times = computeColumnTimes(kept, tid, opt.startMin);
    clock[tid] = times.finish;
    matches[tid] = times.matches;
    lunchDone[tid] =
      !opt.includeLunch ||
      kept.some((r) => r.row_type === "label") ||
      kept.some((r) => r.cells.some((c) => c.tatami_id === tid && !c.nomor_tanding_id && isLunchText(c.custom_text)));
    for (const [rIdx, t] of Object.entries(times.slots)) {
      const cell = kept[+rIdx].cells.find((c) => c.tatami_id === tid);
      if (cell?.nomor_tanding_id) {
        intervals.push({ tid, start: t.start, end: t.end, athletes: new Set(ntAthletes[cell.nomor_tanding_id] || []) });
      }
    }
  }

  const push = (tid: number, cell: EnrichedTimetableCell, minutes: number) => {
    columns[tid].push(cell);
    clock[tid] += minutes;
  };

  let festivalSessionAdded = false;
  if (opt.festivalSession) {
    // Roster-format festival: one FESTIVAL session on every tatami, in the same session row.
    const keptHasFestival = kept.some((r) => r.cells.some((c) => !c.nomor_tanding_id && isFestivalText(c.custom_text)));
    if (opt.includeFestival && !opt.festivalSessionElsewhere && !keptHasFestival && opt.festivalSession.length > 0) {
      for (const tid of ids) {
        const plan = opt.festivalSession.find((f) => f.tatamiId === tid);
        if (plan) push(tid, textCell(tid, tNum(tid), plan.text), plan.minutes);
        else columns[tid].push(blankCell(tid, tNum(tid)));
      }
      festivalSessionAdded = true;
    }
  } else {
    // Festival session for events that do not model festival as categories.
    const hasFestivalCategories = input.categories.some((c) => c.is_festival);
    if (opt.includeFestival && !hasFestivalCategories && kept.length === 0 && opt.festivalSessionMinutes > 0) {
      for (const tid of ids) {
        push(tid, textCell(tid, tNum(tid), `SESI FESTIVAL (${opt.festivalSessionMinutes} MENIT)`), opt.festivalSessionMinutes);
      }
    }
  }

  const pushLunch = (tid: number) => {
    push(tid, textCell(tid, tNum(tid), "ISHOMA"), LUNCH_MINUTES);
    lunchDone[tid] = true;
  };

  const pushJeda = (tid: number, minutes: number) => {
    const col = columns[tid];
    const last = col[col.length - 1];
    if (last && !last.nomor_tanding_id && isJedaText(last.custom_text)) {
      const total = customTextMinutes(last.custom_text) + minutes;
      col[col.length - 1] = textCell(tid, tNum(tid), `JEDA ${total} MENIT`);
      clock[tid] += minutes;
    } else {
      push(tid, textCell(tid, tNum(tid), `JEDA ${minutes} MENIT`), minutes);
    }
  };

  /** Athletes of the other tatamis' cells in session `rowIdx` (kept rows first, then new ones). */
  const rowAthletes = (rowIdx: number, tid: number): Set<string> => {
    const out = new Set<string>();
    for (const other of ids) {
      if (other === tid) continue;
      const cell =
        rowIdx < kept.length
          ? kept[rowIdx].cells.find((c) => c.tatami_id === other)
          : columns[other][rowIdx - kept.length];
      if (cell?.nomor_tanding_id) (ntAthletes[cell.nomor_tanding_id] || []).forEach((a) => out.add(a));
    }
    return out;
  };

  const timeClash = (cand: Candidate, tid: number, s: number, e: number) =>
    intervals.some((iv) => iv.tid !== tid && s < iv.end && e > iv.start && cand.athletes.some((a) => iv.athletes.has(a)));

  const place = (tid: number, cand: Candidate) => {
    const s = clock[tid];
    push(tid, categoryCell(tid, tNum(tid), cand.cat), cand.dur);
    intervals.push({ tid, start: s, end: s + cand.dur, athletes: new Set(cand.athletes) });
    matches[tid] += cand.cat.matches;
    pool = pool.filter((p) => p !== cand);
  };

  let scheduled = 0;
  let forced = 0;
  let guard = 0;

  while (pool.length > 0 && guard++ < 20000) {
    const tid = [...ids].sort(
      (a, b) => clock[a] - clock[b] || (opt.balanceFinish ? matches[a] - matches[b] : 0) || tNum(a) - tNum(b)
    )[0];
    const s = clock[tid];

    if (!lunchDone[tid] && s >= LUNCH_FROM) {
      pushLunch(tid);
      continue;
    }

    const stage = Math.min(...pool.map((p) => p.stage));
    const stageCands = pool
      .filter((p) => p.stage === stage)
      .sort((a, b) => (opt.balanceFinish ? b.dur - a.dur : 0) || a.rank - b.rank);

    let chosen: Candidate | null = null;
    let lunchOnly: Candidate | null = null;
    let lunchBlocked = false; // some candidate is free of time clashes but would run into lunch
    let rowBlocked = false; // some candidate is free of time clashes but clashes within the session
    const rowAths = opt.preventClashes ? rowAthletes(kept.length + columns[tid].length, tid) : null;

    for (const cand of stageCands) {
      const e = s + cand.dur;
      const lunchBlock = !lunchDone[tid] && s >= LUNCH_GUARD_FROM && e > LUNCH_LATEST_END;
      const tClash = opt.preventClashes && timeClash(cand, tid, s, e);
      const rClash = !!rowAths && cand.athletes.some((a) => rowAths.has(a));
      if (!lunchBlock && !tClash && !rClash) {
        chosen = cand;
        break;
      }
      if (tClash) continue;
      if (lunchBlock) lunchBlocked = true;
      if (rClash) rowBlocked = true;
      if (lunchBlock && !rClash && !lunchOnly) lunchOnly = cand;
    }

    if (chosen) {
      place(tid, chosen);
      scheduled++;
      continue;
    }

    // A category would run into lunch: take lunch now if it is late enough.
    if (lunchBlocked && s >= LUNCH_EARLIEST) {
      pushLunch(tid);
      continue;
    }
    // Too early for lunch: run it anyway, lunch follows.
    if (lunchOnly) {
      place(tid, lunchOnly);
      scheduled++;
      continue;
    }

    // A same-session clash blocks: leave this session empty on this tatami.
    if (rowBlocked) {
      columns[tid].push(blankCell(tid, tNum(tid)));
      continue;
    }

    // Athletes are busy on another tatami: wait (visibly) until the next category there ends.
    const nextEnd = Math.min(...intervals.filter((iv) => iv.tid !== tid && iv.end > s).map((iv) => iv.end));
    if (Number.isFinite(nextEnd)) {
      if (!lunchDone[tid] && nextEnd > LUNCH_FROM) {
        if (s >= LUNCH_EARLIEST) {
          pushLunch(tid);
          continue;
        }
        pushJeda(tid, LUNCH_FROM - s);
        continue;
      }
      pushJeda(tid, nextEnd - s);
      continue;
    }

    // Nothing to wait for: place the first candidate and report the clash.
    place(tid, stageCands[0]);
    scheduled++;
    forced++;
  }

  // Assemble rows: kept rows untouched, then one row per new session index.
  const newRowCount = Math.max(0, ...ids.map((tid) => columns[tid].length));
  const rows: EnrichedTimetableRow[] = [...kept];
  for (let r = 0; r < newRowCount; r++) {
    const cells = ids.map((tid) => columns[tid][r] ?? blankCell(tid, tNum(tid)));
    rows.push({
      id: 0,
      day_id: input.dayId,
      order: rows.length,
      row_type: "slot",
      time_label: "",
      label_text: "",
      cells,
      conflicts: rowConflicts(cells, ntAthletes),
    });
  }

  const perTatami = ids.map((tid) => {
    const t = computeColumnTimes(rows, tid, opt.startMin);
    return { tatamiId: tid, tatamiNumber: tNum(tid), matches: t.matches, finish: t.finish };
  });

  return {
    rows,
    scheduled,
    festivalSessionAdded,
    skippedEmpty,
    forced,
    clashes: findTimeClashes(rows, ids, opt.startMin, ntAthletes),
    perTatami,
  };
}
