/**
 * Pure layout helpers for the tournament (elimination) bracket.
 *
 * Moved out of BaganView so the bagan channel, the results booklet and the tatami
 * booklet all draw the exact same bracket. The tree has a fixed geometry: up to four
 * columns of 242 px (gap 32 px) and slots of 82/164/328/656 px, which is what the
 * connector coordinates below are computed from.
 */
import type { BaganPodium, DetailBaganItem } from "$lib/types";

export const TREE_WIDTH = 1064;
export const TREE_HEIGHT = 692;
/** Default column width and the gap between columns (TREE_WIDTH = 4 columns + 3 gaps). */
export const TREE_COL_WIDTH = 242;
export const TREE_COL_GAP = 32;

/** Width of the tree for a given column width (wider columns: same height, longer names). */
export function treeWidthFor(colWidth: number = TREE_COL_WIDTH, columns = 4): number {
  return columns * colWidth + (columns - 1) * TREE_COL_GAP;
}

export interface BracketMeta {
  poolCount: number;
  isFinalBagan: boolean;
  isRepechageBagan: boolean;
  baganName: string;
}

export function deriveBracketMeta(baganName: string, pool: number | null | undefined): BracketMeta {
  const upper = (baganName || "").toUpperCase();
  const lower = (baganName || "").toLowerCase();
  let poolCount = 1;
  if (pool === 2 || pool === 4) {
    poolCount = pool;
  } else if (
    lower.includes("pool a") ||
    lower.includes("pool b") ||
    lower.includes("pool 1") ||
    lower.includes("pool 2") ||
    lower.includes("2 pool")
  ) {
    poolCount = 2;
  } else if (lower.includes("pool c") || lower.includes("pool d") || lower.includes("4 pool")) {
    poolCount = 4;
  }
  const trimmed = upper.trim();
  return {
    poolCount,
    isFinalBagan: pool === 0 || trimmed.endsWith("- FINAL") || trimmed.endsWith("FINAL"),
    isRepechageBagan: upper.includes("REFERCHANGE") || upper.includes("REPECHAGE"),
    baganName: baganName || "",
  };
}

export function isFestivalName(name: string | null | undefined): boolean {
  return (name || "").toUpperCase().includes("FESTIVAL");
}

/** Matches grouped by round, each round sorted by urutan. */
export function groupRounds(matches: DetailBaganItem[]): [number, DetailBaganItem[]][] {
  const map = new Map<number, DetailBaganItem[]>();
  for (const m of matches) {
    const r = m.round || 1;
    if (!map.has(r)) map.set(r, []);
    map.get(r)!.push(m);
  }
  for (const [, list] of map) {
    list.sort((a, b) => (a.urutan || a.id) - (b.urutan || b.id));
  }
  return Array.from(map.entries()).sort((a, b) => a[0] - b[0]);
}

/** Rounds 1-4 form the elimination tree (5+ are podium/team rows). */
export function elimRoundsOf(matches: DetailBaganItem[]): [number, DetailBaganItem[]][] {
  return groupRounds(matches).filter(([r]) => r <= 4);
}

export function getRoundBadge(meta: BracketMeta, roundNum: number, totalElimRounds: number): string {
  if (meta.isRepechageBagan) {
    if (roundNum === totalElimRounds) return "BR";
    return `R${roundNum}`;
  }
  if (meta.poolCount === 2 && !meta.isFinalBagan) {
    if (roundNum === 4) return "SF";
    if (roundNum === 3) return "R3";
    if (roundNum === 2) return "R2";
    return "R1";
  }
  if (meta.poolCount === 4 && !meta.isFinalBagan) {
    if (roundNum === 4) return "R4";
    if (roundNum === 3) return "R3";
    if (roundNum === 2) return "R2";
    return "R1";
  }
  if (roundNum === 4) return "FN";
  if (roundNum === 3) return totalElimRounds >= 4 ? "SF" : "FN";
  if (roundNum === 2) return totalElimRounds >= 4 ? "R2" : "SF";
  return "R1";
}

export function getRoundTitle(meta: BracketMeta, roundNum: number, totalElimRounds: number): string {
  if (meta.isRepechageBagan) {
    if (roundNum === totalElimRounds) return "FINAL REPECHAGE (JUARA 3)";
    return `BABAK ${roundNum} REPECHAGE`;
  }
  if (meta.poolCount === 2 && !meta.isFinalBagan) {
    if (roundNum === 4) return "SEMI FINAL";
    if (roundNum === 3) return "BABAK 8 BESAR";
    if (roundNum === 2) return "BABAK 16 BESAR";
    return "BABAK 32 BESAR";
  }
  if (meta.poolCount === 4 && !meta.isFinalBagan) {
    if (roundNum === 4) return "BABAK 8 BESAR";
    if (roundNum === 3) return "BABAK 16 BESAR";
    if (roundNum === 2) return "BABAK 32 BESAR";
    return "BABAK 64 BESAR";
  }
  if (roundNum === 4) return "PARTAI FINAL";
  if (roundNum === 3) return totalElimRounds >= 4 ? "SEMI FINAL" : "PARTAI FINAL";
  if (roundNum === 2) return totalElimRounds >= 4 ? "PEREMPAT FINAL" : "SEMI FINAL";
  return "BABAK 16 BESAR";
}

export function getSlotCountForRound(roundNum: number): number {
  switch (roundNum) {
    case 1: return 8;
    case 2: return 4;
    case 3: return 2;
    case 4: return 1;
    default: return 1;
  }
}

/** Slot height in px per round (a match card is centred in its slot). */
export function getSlotHeightPx(roundNum: number): number {
  return roundNum >= 1 && roundNum <= 4 ? 82 * 2 ** (roundNum - 1) : 82;
}

/** Natural height of a match card at size 1 (header + two athlete rows). */
export const MATCH_CARD_HEIGHT = 58;

export function getSlotHeightClass(roundNum: number): string {
  switch (roundNum) {
    case 1: return "h-[82px]";
    case 2: return "h-[164px]";
    case 3: return "h-[328px]";
    case 4: return "h-[656px]";
    default: return "h-[82px]";
  }
}

export function getMatchForSlot(roundMatches: DetailBaganItem[], slotNum: number): DetailBaganItem | undefined {
  return roundMatches.find((m) => (m.urutan || 0) === slotNum) || roundMatches[slotNum - 1];
}

export interface ConnectorLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/** Exact connector coordinates for the fixed tree geometry (no DOM measurement). */
export function computeConnectorLines(
  elimRoundCount: number,
  isRepechageBagan: boolean,
  colWidth: number = TREE_COL_WIDTH,
  /** leading rounds left out of the drawing (live drawing hides empty early rounds) */
  skipRounds = 0
): ConnectorLine[] {
  const list: ConnectorLine[] = [];
  if (elimRoundCount < 2) return list;
  // right edge of round column i (0 = round 1), and left edge of column i + 1
  const right = (i: number) => (i - skipRounds) * (colWidth + TREE_COL_GAP) + colWidth;
  const left = (i: number) => right(i - 1) + TREE_COL_GAP;

  if (isRepechageBagan) {
    list.push({ x1: right(0), y1: 159, x2: left(1), y2: 159 });
    list.push({ x1: right(0), y1: 487, x2: left(1), y2: 487 });
    if (elimRoundCount >= 3) {
      list.push({ x1: right(1), y1: 159, x2: left(2), y2: 159 });
      list.push({ x1: right(1), y1: 487, x2: left(2), y2: 487 });
    }
    return list;
  }

  const addFork = (xFrom: number, xTo: number, yA: number, yB: number, yOut: number) => {
    const xMid = Math.round((xFrom + xTo) / 2);
    list.push({ x1: xFrom, y1: yA, x2: xMid, y2: yA });
    list.push({ x1: xFrom, y1: yB, x2: xMid, y2: yB });
    list.push({ x1: xMid, y1: yA, x2: xMid, y2: yB });
    list.push({ x1: xMid, y1: yOut, x2: xTo, y2: yOut });
  };

  if (skipRounds < 1) {
    addFork(right(0), left(1), 77, 159, 118);
    addFork(right(0), left(1), 241, 323, 282);
    addFork(right(0), left(1), 405, 487, 446);
    addFork(right(0), left(1), 569, 651, 610);
  }
  if (elimRoundCount >= 3 && skipRounds < 2) {
    addFork(right(1), left(2), 118, 282, 200);
    addFork(right(1), left(2), 446, 610, 528);
  }
  if (elimRoundCount >= 4 && skipRounds < 3) {
    addFork(right(2), left(3), 200, 528, 364);
  }
  return list;
}

export interface PodiumEntry {
  name: string;
  contingent: string;
  logo: string | null;
  is_priority: boolean;
}

export interface PodiumResult {
  j1: PodiumEntry;
  j2: PodiumEntry;
  j3a: PodiumEntry;
  j3b: PodiumEntry;
}

export const PODIUM_EMPTY = "Belum Ditentukan";

function emptyEntry(): PodiumEntry {
  return { name: PODIUM_EMPTY, contingent: "—", logo: null, is_priority: false };
}

function side(m: DetailBaganItem, s: 1 | 2): PodiumEntry {
  return s === 1
    ? {
        name: m.atlet1_nama || (m.atlet1_id ? `Atlet #${m.atlet1_id}` : "BYE"),
        contingent: m.atlet1_kontingen || "—",
        logo: m.atlet1_logo || null,
        is_priority: !!m.atlet1_is_priority,
      }
    : {
        name: m.atlet2_nama || (m.atlet2_id ? `Atlet #${m.atlet2_id}` : "BYE"),
        contingent: m.atlet2_kontingen || "—",
        logo: m.atlet2_logo || null,
        is_priority: !!m.atlet2_is_priority,
      };
}

/**
 * Hasil Juara: podium saved in the DB first, then filled in from finished matches
 * (final -> juara 1/2, semifinal losers -> 3A/3B; in a 2-pool preliminary the pool's
 * semifinal loser is 3A for pool A and 3B for pool B).
 */
export function computePodium(matches: DetailBaganItem[], meta: BracketMeta, dbPodium?: BaganPodium | null): PodiumResult {
  const fromDb = (p: BaganPodium["j1"]): PodiumEntry =>
    p ? { name: p.name, contingent: p.contingent, logo: p.logo, is_priority: p.is_priority } : emptyEntry();
  let j1 = fromDb(dbPodium?.j1 ?? null);
  let j2 = fromDb(dbPodium?.j2 ?? null);
  let j3a = fromDb(dbPodium?.j3a ?? null);
  let j3b = fromDb(dbPodium?.j3b ?? null);
  const empty = (e: PodiumEntry) => e.name === PODIUM_EMPTY;

  if (meta.poolCount === 2 && !meta.isFinalBagan) {
    const semi = matches.find((m) => m.round === 4);
    if (semi && semi.selesai && semi.pemenang) {
      const loser = side(semi, semi.pemenang === "2" ? 1 : 2);
      const lower = meta.baganName.toLowerCase();
      const isPoolA = lower.includes("pool a") || lower.includes("pool 1");
      if (isPoolA) {
        if (empty(j3a)) j3a = loser;
      } else if (empty(j3b)) {
        j3b = loser;
      }
    }
  } else {
    const elim = elimRoundsOf(matches);
    const finalMatch = matches.find((m) => m.round === 4) || elim[elim.length - 1]?.[1]?.[0];
    const semis = matches.filter((m) => m.round === 3).sort((a, b) => (a.urutan || 0) - (b.urutan || 0));
    if (finalMatch && finalMatch.selesai) {
      if (finalMatch.pemenang === "1") {
        if (empty(j1)) j1 = side(finalMatch, 1);
        if (empty(j2)) j2 = side(finalMatch, 2);
      } else if (finalMatch.pemenang === "2") {
        if (empty(j1)) j1 = side(finalMatch, 2);
        if (empty(j2)) j2 = side(finalMatch, 1);
      }
    }
    const [semi1, semi2] = semis;
    if (semi1 && semi1.selesai && (semi1.pemenang === "1" || semi1.pemenang === "2") && empty(j3a)) {
      j3a = side(semi1, semi1.pemenang === "1" ? 2 : 1);
    }
    if (semi2 && semi2.selesai && (semi2.pemenang === "1" || semi2.pemenang === "2") && empty(j3b)) {
      j3b = side(semi2, semi2.pemenang === "1" ? 2 : 1);
    }
  }
  return { j1, j2, j3a, j3b };
}

/**
 * Booklet data carries the saved podium as `winners`; convert it to the shape the
 * bagan channel gets from get_bagan_podium (same "perguruan • utusan" label).
 */
export function winnersToPodium(
  winners: { juara: string; athlete_id: number; nama_atlet: string; perguruan_nama: string; utusan_nama: string; logo: string | null }[] | null | undefined
): BaganPodium {
  const pick = (rank: string) => {
    const w = (winners || []).find((x) => x.juara === rank);
    if (!w) return null;
    const p = w.perguruan_nama && w.perguruan_nama !== "-" ? w.perguruan_nama : "";
    const u = w.utusan_nama && w.utusan_nama !== "-" ? w.utusan_nama : "";
    const contingent = p && u && p !== u ? `${p} • ${u}` : p || u || "—";
    return { id: w.athlete_id, name: w.nama_atlet, contingent, logo: w.logo, is_priority: false };
  };
  return { j1: pick("1"), j2: pick("2"), j3a: pick("3a"), j3b: pick("3b") };
}
