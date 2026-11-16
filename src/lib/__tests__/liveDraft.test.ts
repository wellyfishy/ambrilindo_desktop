import { beforeEach, describe, expect, it, vi } from "vitest";
import { clearLiveDraft, draftHasProgress, loadLiveDraft, saveLiveDraft, type LiveDraft } from "$lib/liveDraft";

// Node has no localStorage: a Map-backed stand-in
const store = new Map<string, string>();
(globalThis as any).localStorage = {
  getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
  setItem: (k: string, v: string) => void store.set(k, String(v)),
  removeItem: (k: string) => void store.delete(k),
};

const fresh: Omit<LiveDraft, "savedAt"> = {
  akaScore: 0, aoScore: 0, akaSenshu: false, aoSenshu: false,
  akaPenalties: { c1: false, c2: false, c3: false, hc: false, h: false },
  aoPenalties: { c1: false, c2: false, c3: false, hc: false, h: false },
  hasVr1: false, hasVr2: false, isHantei: false, matchWinner: "", timerMs: 120_000,
  kataAkaName: "", kataAoName: "", akaJudgeScores: ["", "", "", "", ""], aoJudgeScores: ["", "", "", "", ""],
  boardAkaScore: 0, boardAoScore: 0, isScorePublished: false, manualFlags: null,
};

describe("live score draft", () => {
  beforeEach(() => {
    store.clear();
    vi.useRealTimers();
  });

  it("keeps a running score until the bout is saved", () => {
    saveLiveDraft(41, { ...fresh, akaScore: 3, aoScore: 1, aoPenalties: { ...fresh.aoPenalties, c1: true }, timerMs: 64_000 });
    const d = loadLiveDraft(41)!;
    expect(d.akaScore).toBe(3);
    expect(d.aoPenalties.c1).toBe(true);
    expect(d.timerMs).toBe(64_000);
    expect(draftHasProgress(d, 120_000)).toBe(true);
    clearLiveDraft(41);
    expect(loadLiveDraft(41)).toBeNull();
  });

  it("writes a point at once, timer-only changes at most once a second", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    saveLiveDraft(7, { ...fresh, timerMs: 119_000 });
    vi.setSystemTime(1_000_200);
    saveLiveDraft(7, { ...fresh, timerMs: 118_800 }); // timer only, too soon: skipped
    expect(loadLiveDraft(7)!.timerMs).toBe(119_000);
    saveLiveDraft(7, { ...fresh, akaScore: 2, timerMs: 118_700 }); // a point: written now
    expect(loadLiveDraft(7)!.akaScore).toBe(2);
    expect(loadLiveDraft(7)!.timerMs).toBe(118_700);
    vi.setSystemTime(1_001_300);
    saveLiveDraft(7, { ...fresh, akaScore: 2, timerMs: 117_600 });
    expect(loadLiveDraft(7)!.timerMs).toBe(117_600);
  });

  it("drops a draft from another day and ignores an untouched bout", () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    saveLiveDraft(9, { ...fresh, akaScore: 1 });
    vi.setSystemTime(13 * 60 * 60 * 1000);
    expect(loadLiveDraft(9)).toBeNull();
    expect(draftHasProgress({ ...fresh, savedAt: 0 }, 120_000)).toBe(false);
  });
});
