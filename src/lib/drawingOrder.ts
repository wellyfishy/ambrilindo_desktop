/**
 * Order of the drawing ceremony: kata perorangan → kata beregu → kumite → kumite beregu.
 * Inside a group: age class (pra-usia dini … senior), putra before putri, lighter before
 * heavier (+84 after -84), then name.
 */
export interface OrderableCategory {
  nama_nomor_tanding: string;
  is_kumite: boolean;
  is_beregu: boolean;
}

const AGE_ORDER = [
  "PRA USIA DINI", "PRA-USIA DINI", "USIA DINI", "PRA PEMULA", "PRA-PEMULA", "PEMULA",
  "KADET", "CADET", "JUNIOR", "U21", "U-21", "UNDER 21", "SENIOR",
];

export function drawingGroup(c: OrderableCategory): number {
  if (!c.is_kumite) return c.is_beregu ? 1 : 0;
  return c.is_beregu ? 3 : 2;
}

export const DRAWING_GROUP_LABEL = ["KATA PERORANGAN", "KATA BEREGU", "KUMITE PERORANGAN", "KUMITE BEREGU"];

function ageRank(name: string): number {
  const up = name.toUpperCase();
  let best = -1;
  AGE_ORDER.forEach((a, i) => {
    if (up.includes(a)) best = i;
  });
  return best === -1 ? AGE_ORDER.length : best;
}

function weightRank(name: string): number {
  const m = name.toUpperCase().match(/([-+])\s*(\d+(?:[.,]\d+)?)\s*KG/);
  if (!m) return 0;
  const w = parseFloat(m[2].replace(",", "."));
  return m[1] === "+" ? w + 0.5 : w;
}

function genderRank(name: string): number {
  const up = name.toUpperCase();
  if (/\bPUTRA\b|\bMALE\b/.test(up)) return 0;
  if (/\bPUTRI\b|\bFEMALE\b/.test(up)) return 1;
  return 2;
}

export function sortForDrawing<T extends OrderableCategory>(cats: T[]): T[] {
  return [...cats].sort(
    (a, b) =>
      drawingGroup(a) - drawingGroup(b) ||
      ageRank(a.nama_nomor_tanding) - ageRank(b.nama_nomor_tanding) ||
      genderRank(a.nama_nomor_tanding) - genderRank(b.nama_nomor_tanding) ||
      weightRank(a.nama_nomor_tanding) - weightRank(b.nama_nomor_tanding) ||
      a.nama_nomor_tanding.localeCompare(b.nama_nomor_tanding)
  );
}

// ---- Live drawing progress (to continue a stopped ceremony later) ----------------
export interface LiveDrawingProgress {
  /** category ids in ceremony order, as started */
  ids: number[];
  /** how many of them are already drawn */
  next: number;
  at: number;
}

const progressKey = (eventId: number) => `ambrilindo.liveDrawing.progress.${eventId}`;

export function saveLiveProgress(eventId: number, cats: { id: number }[], next: number) {
  try {
    localStorage.setItem(progressKey(eventId), JSON.stringify({ ids: cats.map((c) => c.id), next, at: Date.now() }));
  } catch {}
}

export function loadLiveProgress(eventId: number): LiveDrawingProgress | null {
  try {
    const p = JSON.parse(localStorage.getItem(progressKey(eventId)) || "null");
    if (p && Array.isArray(p.ids) && typeof p.next === "number" && p.next < p.ids.length) return p;
  } catch {}
  return null;
}

export function clearLiveProgress(eventId: number) {
  try {
    localStorage.removeItem(progressKey(eventId));
  } catch {}
}
