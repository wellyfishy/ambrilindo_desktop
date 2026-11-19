import type { BaganDaysResult, SummaryBaganItem } from "$lib/types";

export interface JuaraDayGroup {
  key: string;
  /** "Day 1, 18 November 2026", "Belum terjadwal", or "" when there is no schedule */
  label: string;
  bagans: SummaryBaganItem[];
}

/**
 * Juara per bagan grouped by Jadwal day, in day order. A class played over several days
 * (pools on Day 1, final on Day 2) goes to its last day, where its champions are decided.
 * Classes on no day come last as "Belum terjadwal". Without a schedule: one group, no label.
 */
export function groupJuaraByDay(bagans: SummaryBaganItem[], baganDays: BaganDaysResult | null | undefined): JuaraDayGroup[] {
  const days = baganDays?.days ?? [];
  if (!baganDays || days.length === 0) return [{ key: "all", label: "", bagans }];

  const orderOf = new Map(days.map((d) => [d.id, d.order]));
  const groups = new Map<string, JuaraDayGroup>(
    days.map((d) => [String(d.id), { key: String(d.id), label: d.label, bagans: [] as SummaryBaganItem[] }]),
  );
  const unscheduled: SummaryBaganItem[] = [];

  for (const b of bagans) {
    const ids = (baganDays.bagan_days[String(b.id)] ?? []).filter((id) => orderOf.has(id));
    if (ids.length === 0) {
      unscheduled.push(b);
      continue;
    }
    const last = ids.reduce((a, c) => ((orderOf.get(c) ?? 0) > (orderOf.get(a) ?? 0) ? c : a));
    groups.get(String(last))!.bagans.push(b);
  }

  const out = [...groups.values()].filter((g) => g.bagans.length > 0);
  if (unscheduled.length > 0) out.push({ key: "none", label: "Belum terjadwal", bagans: unscheduled });
  return out;
}
