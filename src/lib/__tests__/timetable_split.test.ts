import { describe, expect, it } from "vitest";
import { cellDisplayName, missingSplitParts, splitPartShare } from "../timetable_split";
import { rowConflicts } from "../auto_schedule";
import type { EnrichedTimetableCell } from "../types";

describe("split category over two tatamis", () => {
  it("shares entries, bouts and minutes like the Rust side (split_part_share)", () => {
    // same cases as qa_tests::split_part_math
    expect(splitPartShare(9, 8, 40, "top")).toEqual({ entries: 5, matches: 4, minutes: 20 });
    expect(splitPartShare(9, 8, 40, "bottom")).toEqual({ entries: 4, matches: 3, minutes: 15 });
    expect(splitPartShare(9, 8, 40, "final")).toEqual({ entries: 2, matches: 1, minutes: 5 });
    expect(splitPartShare(9, 8, 40, "top_final")).toEqual({ entries: 5, matches: 5, minutes: 25 });
    expect(splitPartShare(9, 8, 40, null)).toEqual({ entries: 9, matches: 8, minutes: 40 });
  });

  it("knows which parts are still to place", () => {
    expect(missingSplitParts([])).toEqual([]);
    expect(missingSplitParts([null])).toEqual([]);
    expect(missingSplitParts(["top", "bottom"])).toEqual(["final"]);
    expect(missingSplitParts(["top_final", "bottom"])).toEqual([]);
    expect(missingSplitParts(["bottom_final"])).toEqual(["top"]);
  });

  it("labels printed names and does not flag its own halves as a clash", () => {
    const cell = (tatami: number, part: EnrichedTimetableCell["part"]) =>
      ({ tatami_id: tatami, tatami_number: tatami, nomor_tanding_id: 7, nomor_tanding_nama: "KATA PERORANGAN PUTRA", custom_text: "", part }) as EnrichedTimetableCell;
    expect(cellDisplayName(cell(1, "top"))).toBe("KATA PERORANGAN PUTRA (POOL ATAS)");
    expect(cellDisplayName(cell(1, null))).toBe("KATA PERORANGAN PUTRA");
    expect(rowConflicts([cell(1, "top"), cell(2, "bottom")], { 7: ["A", "B", "C", "D"] })).toEqual([]);
  });
});
