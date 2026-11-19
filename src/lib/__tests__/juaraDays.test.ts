import { describe, expect, it } from "vitest";
import { groupJuaraByDay } from "../juaraDays";
import type { BaganDaysResult, SummaryBaganItem } from "../types";

const bagan = (id: number): SummaryBaganItem => ({ id, kode: String(id), nama_bagan: `B${id}`, tipe_tanding: "2", is_selesai: true, winners: [] });

const days: BaganDaysResult = {
  days: [
    { id: 10, order: 0, label: "Day 1, 18 November 2026" },
    { id: 20, order: 1, label: "Day 2, 19 November 2026" },
  ],
  // 1: day 1; 2: day 2; 3: pools day 1 + final day 2 -> day 2; 4: not scheduled
  bagan_days: { "1": [10], "2": [20], "3": [10, 20] },
};

describe("groupJuaraByDay", () => {
  it("groups classes per day in day order, a split class under its last day, unscheduled last", () => {
    const groups = groupJuaraByDay([bagan(4), bagan(3), bagan(2), bagan(1)], days);
    expect(groups.map((g) => [g.label, g.bagans.map((b) => b.id)])).toEqual([
      ["Day 1, 18 November 2026", [1]],
      ["Day 2, 19 November 2026", [3, 2]],
      ["Belum terjadwal", [4]],
    ]);
  });

  it("drops empty days", () => {
    const groups = groupJuaraByDay([bagan(1)], days);
    expect(groups.map((g) => g.label)).toEqual(["Day 1, 18 November 2026"]);
  });

  it("without a schedule: one group without a header", () => {
    expect(groupJuaraByDay([bagan(1), bagan(2)], null)).toEqual([{ key: "all", label: "", bagans: [bagan(1), bagan(2)] }]);
    expect(groupJuaraByDay([bagan(1)], { days: [], bagan_days: {} })[0].label).toBe("");
  });
});
