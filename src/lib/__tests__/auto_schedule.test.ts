import { describe, expect, it } from "vitest";
import { findTimeClashes, runAutoSchedule, type AutoScheduleOptions } from "$lib/auto_schedule";
import type { TimetableCategoryOption } from "$lib/types";

/** PORPROV-like day: 17 categories, some athletes enter kata perorangan and kata beregu. */
function porprov() {
  const names = [
    "KATA PERORANGAN PUTRA", "KATA PERORANGAN PUTRI", "KATA BEREGU PUTRA", "KATA BEREGU PUTRI",
    "KUMITE -55 KG PUTRA", "KUMITE -60 KG PUTRA", "KUMITE -67 KG PUTRA", "KUMITE -75 KG PUTRA", "KUMITE -84 KG PUTRA",
    "KUMITE +84 KG PUTRA", "KUMITE -50 KG PUTRI", "KUMITE -55 KG PUTRI", "KUMITE -61 KG PUTRI", "KUMITE -68 KG PUTRI",
    "KUMITE +68 KG PUTRI", "KUMITE BEREGU PUTRA", "KUMITE BEREGU PUTRI",
  ];
  const categories: TimetableCategoryOption[] = names.map((nama, i) => ({
    id: 100 + i, nama, age_group: "SENIOR", atlet_count: 7, matches: 7,
    discipline: nama.startsWith("KATA") ? "kata" : "kumite", est_duration_minutes: 60, is_festival: false,
    progress_status: "belum_disentuh", progress_pct: 0, is_assigned: false, assigned_tatami: null, assigned_day_order: null,
  }));
  const ntAthletes: Record<number, string[]> = {};
  categories.forEach((c) => (ntAthletes[c.id] = Array.from({ length: 7 }, (_, k) => `${c.nama} #${k}`)));
  // doubles: the same athlete in individual and team events, and in kumite + kata
  ntAthletes[100].push("ANDI", "BUDI");
  ntAthletes[102].push("ANDI", "BUDI");
  ntAthletes[101].push("CITRA");
  ntAthletes[103].push("CITRA");
  ntAthletes[105].push("DONI");
  ntAthletes[115].push("DONI");
  const tatamis = [1, 2, 3].map((n) => ({ id: n, tatami_number: n }));
  return { categories, ntAthletes, tatamis };
}

const opt: AutoScheduleOptions = {
  startMin: 8 * 60, sortMode: "kata_then_kumite", includeFestival: false, festivalSessionMinutes: 0,
  includeLunch: true, balanceFinish: true, preventClashes: true, rebuildDay: true, isAgeGroupSelected: () => true,
};

describe("auto schedule (PORPROV day, 3 tatami)", () => {
  const { categories, ntAthletes, tatamis } = porprov();
  const res = runAutoSchedule({ dayId: 1, categories, tatamis, dayRows: [], assignedOnOtherDays: new Set(), ntAthletes }, opt);
  const placed = res.rows.flatMap((r) => r.cells.map((c) => c.nomor_tanding_id).filter((x): x is number => !!x));

  it("schedules every category exactly once", () => {
    expect(res.scheduled).toBe(categories.length);
    expect(new Set(placed).size).toBe(placed.length);
    expect(new Set(placed)).toEqual(new Set(categories.map((c) => c.id)));
  });

  it("no athlete is due on two tatamis at the same time", () => {
    expect(res.forced).toBe(0);
    expect(findTimeClashes(res.rows, tatamis.map((t) => t.id), opt.startMin, ntAthletes)).toEqual([]);
  });

  it("a day running past noon gets ISHOMA on every tatami", () => {
    for (const t of tatamis) {
      const lunch = res.rows.some((r) => r.cells.some((c) => c.tatami_id === t.id && /ISHOMA/i.test(c.custom_text)));
      expect(lunch, `tatami ${t.tatami_number} has ISHOMA`).toBe(true);
    }
    expect(res.perTatami).toHaveLength(3);
  });

  it("keeps categories that are already on another day", () => {
    const again = runAutoSchedule(
      { dayId: 2, categories, tatamis, dayRows: [], assignedOnOtherDays: new Set([100, 101]), ntAthletes },
      opt,
    );
    const ids = again.rows.flatMap((r) => r.cells.map((c) => c.nomor_tanding_id));
    expect(ids).not.toContain(100);
    expect(ids).not.toContain(101);
  });
});
