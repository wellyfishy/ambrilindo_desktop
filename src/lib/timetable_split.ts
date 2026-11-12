// One category played on two tatamis at once: the top half of the bracket (pool atas) on
// one tatami, the bottom half (pool bawah) on the other, and the final on one of them or
// in a session of its own. Mirrors `split_part_share` / `missing_split_parts` in
// src-tauri/src/db.rs — keep the two in step.

import type { EnrichedTimetableCell, SplitPart } from "./types";

export const SPLIT_PART_LABEL: Record<SplitPart, string> = {
  top: "POOL ATAS",
  bottom: "POOL BAWAH",
  final: "FINAL",
  top_final: "POOL ATAS + FINAL",
  bottom_final: "POOL BAWAH + FINAL",
};

/** Fewest entries worth splitting: two per half. */
export const MIN_SPLIT_ENTRIES = 4;

type Half = "top" | "bottom" | "final";

export function partCovers(part: SplitPart, half: Half): boolean {
  return part === half || part === `${half}_final` || (half === "final" && part.endsWith("_final"));
}

/** Entries, bouts and minutes of one part, from the whole category's numbers. */
export function splitPartShare(
  entries: number,
  matches: number,
  minutes: number,
  part: SplitPart | null | undefined
): { entries: number; matches: number; minutes: number } {
  if (!part) return { entries, matches, minutes };
  const top = Math.floor((entries + 1) / 2);
  const bottom = Math.floor(entries / 2);
  const fin = entries >= 2 ? 1 : 0;
  const [e, m] =
    part === "top"
      ? [top, Math.max(0, top - 1)]
      : part === "bottom"
        ? [bottom, Math.max(0, bottom - 1)]
        : part === "final"
          ? [Math.min(entries, 2), fin]
          : part === "top_final"
            ? [top, Math.max(0, top - 1) + fin]
            : [bottom, Math.max(0, bottom - 1) + fin];
  // Rust rounds half away from zero; Math.round rounds half up. Same for positive values.
  const est = matches > 0 ? Math.round((minutes * m) / matches) : 0;
  return { entries: e, matches: m, minutes: m > 0 ? Math.max(5, est) : 0 };
}

/**
 * Parts of a split category that are not on the timetable yet. `placed` holds the part of
 * every cell of the category (null = whole). Empty when fully scheduled or not at all.
 */
export function missingSplitParts(placed: (SplitPart | null | undefined)[]): Half[] {
  if (placed.length === 0 || placed.some((p) => !p)) return [];
  return (["top", "bottom", "final"] as Half[]).filter((h) => !placed.some((p) => p && partCovers(p, h)));
}

/** Category name with its part, e.g. "KATA PERORANGAN PUTRA (POOL ATAS)". */
export function cellDisplayName(cell: Pick<EnrichedTimetableCell, "nomor_tanding_nama" | "custom_text" | "part">): string {
  const base = cell.nomor_tanding_nama || cell.custom_text || "";
  return cell.part ? `${base} (${SPLIT_PART_LABEL[cell.part]})` : base;
}
