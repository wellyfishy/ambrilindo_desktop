<script lang="ts">
  // Drawing live: one category at a time, in ceremony order, full screen for the coaches.
  // The host draws the category first (same anti-clash drawing as the normal button), then
  // the real bracket is shown in the usual bracket template: its first-round slots shuffle
  // through the participants every few milliseconds while a big countdown runs, and all
  // lock in to the real draw at once when it reaches zero.
  // The animation never decides anything. Progress is saved, so a stopped ceremony can be
  // continued later from the next category.
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { BaganItem, DetailBaganItem, DrawingCategoryInfo, DrawingSummary, EnrichedAtletItem } from "$lib/types";
  import { drawingGroup, DRAWING_GROUP_LABEL, saveLiveProgress, clearLiveProgress } from "$lib/drawingOrder";
  import { computePodium, deriveBracketMeta, TREE_COL_GAP, TREE_COL_WIDTH, TREE_HEIGHT, treeWidthFor } from "$lib/bracket_layout";
  import BracketTree from "$lib/components/BracketTree.svelte";

  interface Props {
    eventId: number;
    eventName: string;
    /** already in ceremony order */
    categories: DrawingCategoryInfo[];
    /** continue a stopped ceremony from this category */
    startIndex?: number;
    shuffleType: string;
    vrCategoryIds: number[];
    shuffleMs: number;
    /** 0 = no timer: shuffle until the operator presses STOP ACAK */
    durationMs: number;
    autoNext: boolean;
    onFinish: (summary: DrawingSummary) => void;
  }

  let {
    eventId,
    eventName,
    categories,
    startIndex = 0,
    shuffleType,
    vrCategoryIds,
    shuffleMs,
    durationMs,
    autoNext,
    onFinish,
  }: Props = $props();

  const HOLD_MS = 6000; // result stays this long before the next category (auto mode)
  const FINAL_SECONDS = 3; // last seconds of the big countdown turn red
  const INTRO_MS = 10000; // auto mode: athlete table shows this long before the draw starts
  const FLASH_MS = 900; // lock-in flash

  type Phase = "ready" | "shuffle" | "reveal" | "hold" | "error" | "done";
  interface Occupant {
    id: number | null;
    nama: string | null;
    kontingen: string | null;
    logo: string | null;
    /** seeded: the bracket draws a star next to this athlete */
    prio: boolean;
  }
  interface Slot {
    match: number; // index in the bagan's match list
    side: 1 | 2;
    real: Occupant;
  }
  interface BaganDraw {
    bagan: BaganItem;
    matches: DetailBaganItem[];
    slots: Slot[];
  }

  // svelte-ignore state_referenced_locally
  let index = $state(startIndex);
  let phase = $state<Phase>("ready");
  let paused = $state(false);
  let errorMessage = $state("");
  let draws = $state<BaganDraw[]>([]);
  let shown = $state<DetailBaganItem[][]>([]); // what the trees draw right now
  let locked = $state(0); // slots already locked to the real draw (across all bagans)
  let holdLeftMs = $state(0);
  // Shuffle countdown (timer mode). Pausing and continuing starts it again from the full time.
  let countdownLeftMs = $state(0);
  let secondsLeft = $derived(Math.max(0, Math.ceil(countdownLeftMs / 1000)));
  let countdownProgress = $derived(durationMs > 0 ? Math.max(0, Math.min(1, countdownLeftMs / durationMs)) : 0);
  let flashId = $state(0); // bumps on every lock-in, replays the flash

  // Sound for the audience: a tick every second, higher for the last 3, a chord at lock-in
  const SOUND_KEY = "ambrilindo.liveDrawing.sound";
  let soundOn = $state(readSound());
  function readSound(): boolean {
    try {
      return localStorage.getItem(SOUND_KEY) !== "0";
    } catch {
      return true;
    }
  }
  function toggleSound() {
    soundOn = !soundOn;
    try {
      localStorage.setItem(SOUND_KEY, soundOn ? "1" : "0");
    } catch {}
  }
  let audioCtx: AudioContext | null = null;
  function tone(freq: number, durSec: number, vol = 0.18, delaySec = 0) {
    if (!soundOn) return;
    try {
      const Ctx = window.AudioContext || (window as any).webkitAudioContext;
      if (!Ctx) return;
      audioCtx ??= new Ctx();
      if (audioCtx.state === "suspended") audioCtx.resume().catch(() => {});
      const t = audioCtx.currentTime + delaySec;
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.type = "sine";
      osc.frequency.setValueAtTime(freq, t);
      gain.gain.setValueAtTime(vol, t);
      gain.gain.exponentialRampToValueAtTime(0.001, t + durSec);
      osc.connect(gain);
      gain.connect(audioCtx.destination);
      osc.start(t);
      osc.stop(t + durSec);
    } catch {}
  }
  function tick(sec: number) {
    if (sec <= FINAL_SECONDS) tone(880, 0.18, 0.22);
    else tone(620, 0.08, 0.1);
  }
  function lockChord() {
    tone(1046.5, 0.6, 0.18);
    tone(1318.5, 0.6, 0.14, 0.06);
    tone(1568, 0.8, 0.12, 0.12);
  }

  // Per category, so drawing a category again replaces its numbers instead of adding to them
  const perCategory = new Map<number, DrawingSummary>();
  function totalSummary(): DrawingSummary {
    const t: DrawingSummary = { categories_processed: 0, bagans_created: 0, total_matches: 0, total_collisions: 0 };
    for (const s of perCategory.values()) {
      t.categories_processed += s.categories_processed;
      t.bagans_created += s.bagans_created;
      t.total_matches += s.total_matches;
      t.total_collisions += s.total_collisions;
    }
    return t;
  }

  let current = $derived(categories[index]);

  // Opening screen of every category: its athletes (or teams) in one table, one screen
  let athletesByCat = $state<Record<number, EnrichedAtletItem[]>>({});
  let athletesLoading = $state(true);
  let introLeftMs = $state(0);
  async function loadAthletes() {
    try {
      const all = await invoke<EnrichedAtletItem[]>("get_enriched_athletes", { eventId });
      const map: Record<number, EnrichedAtletItem[]> = {};
      for (const a of all) {
        if (a.nomor_tanding_id == null) continue;
        // a team category lists its teams only, not the members of each team (as drawn)
        if ((a.additional_code || "").toUpperCase().startsWith("MEMBER_")) continue;
        (map[a.nomor_tanding_id] ??= []).push(a);
      }
      // Grouped by kontingen, so coaches find their athletes quickly
      for (const list of Object.values(map)) {
        list.sort(
          (x, y) =>
            (x.utusan_nama || x.perguruan_nama || "").localeCompare(y.utusan_nama || y.perguruan_nama || "") ||
            (x.nama_atlet || "").localeCompare(y.nama_atlet || "")
        );
      }
      athletesByCat = map;
    } catch (e) {
      console.warn("Gagal memuat data atlet drawing live:", e);
    } finally {
      athletesLoading = false;
    }
  }
  let currentAthletes = $derived(current ? athletesByCat[current.id] ?? [] : []);

  // Table sized to fit one screen: up to 4 columns, rows and font from the stage size
  const TABLE_HEAD_PX = 34;
  const TABLE_COL_GAP_PX = 16;
  let tableLayout = $derived.by(() => {
    const n = Math.max(1, currentAthletes.length);
    const cols = n <= 12 ? 1 : n <= 26 ? 2 : n <= 48 ? 3 : 4;
    const rows = Math.ceil(n / cols);
    const rowH = Math.max(16, Math.min(64, (stageH - TABLE_HEAD_PX - 8) / rows));
    const colW = (stageW - TABLE_COL_GAP_PX * (cols - 1)) / cols;
    const font = Math.max(10, Math.min(30, rowH * 0.5, colW / 30));
    return { cols, rows, rowH, font };
  });
  let totalSlots = $derived(draws.reduce((n, d) => n + d.slots.length, 0));

  let ticker: ReturnType<typeof setInterval> | null = null;
  let destroyed = false;
  let skip = false;
  let stopShuffle = $state(false); // operator pressed STOP ACAK (no-timer mode)
  let manualStop = $derived(durationMs <= 0);

  // Area the trees are scaled into
  let stageW = $state(1200);
  let stageH = $state(600);
  // Fill the screen: the tree's height sets the scale, and its columns are widened (not
  // stretched) to use the full width, so a wide TV has no empty sides and long names fit.
  const TREE_GAP_PX = 12;
  const POOL_LABEL_PX = 22;
  let treeCols = $derived(draws.length > 1 && stageW / stageH > 1.6 ? Math.min(draws.length, 2) : 1);
  // Card width (at size 1) that shows the longest name and contingent of this category in
  // full: AKA/AO pill + gaps + padding + text, measured with the bracket's own fonts.
  let measureCtx: CanvasRenderingContext2D | null = null;
  function textWidth(text: string, font: string): number {
    if (!measureCtx) measureCtx = document.createElement("canvas").getContext("2d");
    if (!measureCtx) return text.length * 5.5;
    measureCtx.font = font;
    return measureCtx.measureText(text).width;
  }
  let neededCardWidth = $derived.by(() => {
    if (typeof document === "undefined" || draws.length === 0) return 150;
    const family = getComputedStyle(document.body).fontFamily || "sans-serif";
    let widest = 0;
    for (const d of draws) {
      for (const s of d.slots) {
        widest = Math.max(
          widest,
          textWidth(s.real.nama || "", `900 8.5px ${family}`) + 12, // + winner crown room
          textWidth(s.real.kontingen || "", `700 7px ${family}`)
        );
      }
    }
    return Math.ceil(widest + 22 + 4 + 12 + 4); // pill, gap, row padding, border
  });

  // Leading rounds with only BYEs are left out (8 athletes: play starts in round 2), so the
  // rounds that matter get the whole width. Same for every pool of the category.
  let skipRounds = $derived.by(() => {
    if (draws.length === 0) return 0;
    const firsts = draws.map((d) => Math.min(...d.slots.map((s) => d.matches[s.match].round || 1)));
    return Math.max(0, Math.min(2, Math.min(...firsts) - 1));
  });

  let treeFit = $derived.by(() => {
    const n = Math.max(1, draws.length);
    const columns = 4 - skipRounds;
    const cols = treeCols;
    const rows = Math.ceil(n / cols);
    const boxW = (stageW - TREE_GAP_PX * (cols - 1)) / cols;
    const boxH = (stageH - TREE_GAP_PX * (rows - 1) - (n > 1 ? POOL_LABEL_PX * rows : 0)) / rows;
    let scale = boxH / TREE_HEIGHT;
    let colWidth = (boxW / scale - (columns - 1) * TREE_COL_GAP) / columns;
    if (colWidth < TREE_COL_WIDTH) {
      // narrow screen: keep the normal columns and fit the width instead
      colWidth = TREE_COL_WIDTH;
      scale = boxW / treeWidthFor(colWidth, columns);
    }
    // Bigger match cards, as big as the longest name / contingent of this category allows
    // (the card keeps the column width, so a bigger card leaves less room per letter)
    const cardScale = Math.max(1, Math.min(2.4, colWidth / Math.max(150, neededCardWidth)));
    return { scale: Math.max(0.2, scale), colWidth: Math.floor(colWidth), cardScale, columns };
  });

  function sleep(ms: number) {
    return new Promise((r) => setTimeout(r, ms));
  }

  function shuffled<T>(arr: T[]): T[] {
    const a = [...arr];
    for (let i = a.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
  }

  function stopTicker() {
    if (ticker) clearInterval(ticker);
    ticker = null;
  }

  function occupant(m: DetailBaganItem, side: 1 | 2): Occupant {
    return side === 1
      ? { id: m.atlet1_id, nama: m.atlet1_nama ?? null, kontingen: m.atlet1_kontingen ?? null, logo: (m as any).atlet1_logo ?? null, prio: !!m.atlet1_is_priority }
      : { id: m.atlet2_id, nama: m.atlet2_nama ?? null, kontingen: m.atlet2_kontingen ?? null, logo: (m as any).atlet2_logo ?? null, prio: !!m.atlet2_is_priority };
  }

  function withOccupant(m: DetailBaganItem, side: 1 | 2, o: Occupant): DetailBaganItem {
    return side === 1
      ? ({ ...m, atlet1_id: o.id, atlet1_nama: o.nama, atlet1_kontingen: o.kontingen, atlet1_logo: o.logo, atlet1_is_priority: o.prio } as DetailBaganItem)
      : ({ ...m, atlet2_id: o.id, atlet2_nama: o.nama, atlet2_kontingen: o.kontingen, atlet2_logo: o.logo, atlet2_is_priority: o.prio } as DetailBaganItem);
  }

  /** The bracket with slots < lockedCount real and the rest filled from a random permutation */
  function render(lockedCount: number) {
    let k = 0;
    shown = draws.map((d) => {
      const pool = shuffled(d.slots.map((s) => s.real));
      const out = d.matches.map((m) => ({ ...m }));
      d.slots.forEach((s, i) => {
        const real = k++ < lockedCount;
        out[s.match] = withOccupant(out[s.match], s.side, real ? s.real : pool[i]);
      });
      return out;
    });
  }

  async function loadDraws(cat: DrawingCategoryInfo): Promise<BaganDraw[]> {
    const bagans = (await invoke<BaganItem[]>("get_bagans", { eventId }))
      .filter((b) => b.nomor_tanding_id === cat.id)
      .sort((a, b) => (a.pool || 0) - (b.pool || 0) || (a.nama_bagan || "").localeCompare(b.nama_bagan || ""));
    const out: BaganDraw[] = [];
    for (const b of bagans) {
      const matches = (await invoke<DetailBaganItem[]>("get_detail_bagan", { baganId: b.id }))
        .filter((m) => !m.team)
        .sort((x, y) => (x.round || 0) - (y.round || 0) || (x.urutan || 0) - (y.urutan || 0));
      // Every athlete's first slot in the bracket (byes start a round later)
      const seen = new Set<number>();
      const slots: Slot[] = [];
      matches.forEach((m, i) => {
        for (const side of [1, 2] as const) {
          const id = side === 1 ? m.atlet1_id : m.atlet2_id;
          if (id && !seen.has(id)) {
            seen.add(id);
            slots.push({ match: i, side, real: occupant(m, side) });
          }
        }
      });
      if (slots.length > 0) out.push({ bagan: b, matches, slots });
    }
    return out;
  }

  async function runCategory() {
    const cat = current;
    if (!cat) return;
    stopTicker();
    skip = false;
    stopShuffle = false;
    errorMessage = "";
    locked = 0;
    phase = "shuffle";
    draws = [];
    shown = [];
    try {
      const s = await invoke<DrawingSummary>("execute_drawing_bagan", {
        eventId,
        categoryIds: [cat.id],
        shuffleType,
        vrCategoryIds: vrCategoryIds.filter((id) => id === cat.id),
      });
      perCategory.set(cat.id, s);
      draws = await loadDraws(cat);
    } catch (e: any) {
      phase = "error";
      errorMessage = `Drawing ${cat.nama_nomor_tanding} gagal: ${e?.message || e}`;
      return;
    }
    if (destroyed) return;

    // Shuffle: every slot shows a random participant, every shuffleMs
    render(0);
    ticker = setInterval(() => {
      if (!paused) render(0);
    }, Math.max(10, shuffleMs));
    if (manualStop) {
      // No timer: keep shuffling until the operator stops it
      while (!stopShuffle && !skip && !destroyed) await sleep(40);
    } else {
      await runCountdown();
    }
    if (destroyed) return;

    // Every slot locks in to the real draw at the same moment
    stopTicker();
    locked = totalSlots;
    render(locked);
    countdownLeftMs = 0;
    phase = "reveal";
    flashId += 1;
    lockChord();
    saveLiveProgress(eventId, categories, index + 1);
    await sleep(FLASH_MS);
    if (destroyed) return;
    startHold();
  }

  /**
   * Counts down durationMs while the slots shuffle. Pausing freezes the shuffle; when the
   * operator continues, the countdown starts again from the full time that was set.
   */
  async function runCountdown() {
    countdownLeftMs = durationMs;
    let last = performance.now();
    let wasPaused = false;
    let lastSec = Math.ceil(durationMs / 1000);
    tick(lastSec);
    while (countdownLeftMs > 0 && !destroyed && !skip) {
      await sleep(40);
      const now = performance.now();
      if (paused) {
        wasPaused = true;
        countdownLeftMs = durationMs; // shows the time it will restart from
        last = now;
        continue;
      }
      if (wasPaused) {
        wasPaused = false;
        countdownLeftMs = durationMs;
        lastSec = Math.ceil(durationMs / 1000);
        last = now;
        tick(lastSec);
        continue;
      }
      countdownLeftMs = Math.max(0, countdownLeftMs - (now - last));
      last = now;
      const sec = Math.ceil(countdownLeftMs / 1000);
      if (sec !== lastSec && sec > 0) tick(sec);
      lastSec = sec;
    }
  }

  async function startHold() {
    phase = "hold";
    skip = false;
    if (index >= categories.length - 1) {
      clearLiveProgress(eventId);
      return;
    }
    if (!autoNext) return;
    holdLeftMs = HOLD_MS;
    const myIndex = index;
    while (holdLeftMs > 0 && !destroyed && phase === "hold" && index === myIndex) {
      await sleep(100);
      if (!paused) holdLeftMs -= 100;
    }
    if (!destroyed && phase === "hold" && index === myIndex) next();
  }

  function next() {
    stopTicker();
    if (index >= categories.length - 1) {
      phase = "done";
      clearLiveProgress(eventId);
      return;
    }
    index += 1;
    showIntro();
  }

  /** Athlete table of the current category; in auto mode the draw starts by itself */
  async function showIntro() {
    phase = "ready";
    draws = [];
    shown = [];
    introLeftMs = 0;
    if (!autoNext) return;
    introLeftMs = INTRO_MS;
    const myIndex = index;
    while (introLeftMs > 0 && !destroyed && phase === "ready" && index === myIndex) {
      await sleep(100);
      if (!paused) introLeftMs -= 100;
    }
    if (!destroyed && phase === "ready" && index === myIndex) runCategory();
  }

  function redraw() {
    // Back to the athlete table: seeds can be changed, then "Mulai Undi" draws again
    // (the host replaces a category's bracket when it is drawn again, no results yet)
    stopTicker();
    phase = "ready";
    draws = [];
    shown = [];
    introLeftMs = 0;
    paused = false;
  }

  // ---- Seeded athletes (is_priority): far apart in the draw, star in the bracket ----
  let seedSaving = $state<Record<number, boolean>>({});
  let seedCount = $derived(currentAthletes.filter((a) => a.is_priority).length);
  async function toggleSeed(a: EnrichedAtletItem) {
    if (phase !== "ready" || seedSaving[a.id] || a.nomor_tanding_id == null) return;
    const next = !a.is_priority;
    const catId = a.nomor_tanding_id;
    const setLocal = (v: boolean) => {
      athletesByCat = {
        ...athletesByCat,
        [catId]: (athletesByCat[catId] ?? []).map((x) => (x.id === a.id ? { ...x, is_priority: v } : x)),
      };
    };
    setLocal(next);
    seedSaving = { ...seedSaving, [a.id]: true };
    try {
      await invoke("toggle_athlete_priority", { id: a.id, isPriority: next });
    } catch (e) {
      console.warn("Gagal menyimpan unggulan:", e);
      setLocal(!next);
    } finally {
      seedSaving = { ...seedSaving, [a.id]: false };
    }
  }

  function skipAnimation() {
    skip = true;
  }

  function stop() {
    stopTicker();
    destroyed = true;
    onFinish(totalSummary());
  }

  function toggleFullscreen() {
    if (!document.fullscreenElement) document.documentElement.requestFullscreen?.().catch(() => {});
    else document.exitFullscreen?.().catch(() => {});
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === " ") {
      e.preventDefault();
      if (phase !== "done" && phase !== "error") paused = !paused;
    } else if (e.key === "Enter" || e.key === "ArrowRight") {
      e.preventDefault();
      if (phase === "ready") runCategory();
      else if (phase === "shuffle" && manualStop && !stopShuffle) stopShuffle = true;
      else if (phase === "shuffle" || phase === "reveal") skipAnimation();
      else if (phase === "hold") next();
    }
  }

  let stageEl: HTMLDivElement | null = $state(null);
  let resizeObs: ResizeObserver | null = null;
  onMount(() => {
    window.addEventListener("keydown", onKey);
    loadAthletes();
    if (stageEl) {
      resizeObs = new ResizeObserver(([e]) => {
        stageW = e.contentRect.width;
        stageH = e.contentRect.height;
      });
      resizeObs.observe(stageEl);
    }
  });
  onDestroy(() => {
    destroyed = true;
    stopTicker();
    resizeObs?.disconnect();
    window.removeEventListener("keydown", onKey);
  });
</script>

<div class="fixed inset-0 z-[70] bg-[#0d0e12] text-zinc-100 flex flex-col font-sans select-none">
  <!-- Header -->
  <header class="px-6 py-3 bg-[#14161b] border-b border-white/[0.08] flex flex-wrap items-center justify-between gap-3 shrink-0">
    <div class="flex items-center gap-2.5 min-w-0">
      <span class="w-2 h-2 shrink-0 rounded-full {paused ? 'bg-zinc-500' : phase === 'shuffle' || phase === 'reveal' ? 'bg-amber-400 animate-pulse' : 'bg-emerald-400'}"></span>
      <h1 class="text-sm font-black tracking-wider uppercase text-white font-mono truncate">Drawing Live · {eventName}</h1>
    </div>
    <div class="flex items-center gap-2 shrink-0 font-mono text-xs">
      <span class="px-2 py-0.5 rounded text-[11px] font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
        {Math.min(index + 1, categories.length)} / {categories.length}
      </span>
      {#if phase !== "done" && phase !== "error" && (phase !== "ready" || introLeftMs > 0)}
        <button
          type="button"
          onclick={() => (paused = !paused)}
          class="h-9 px-3.5 font-semibold text-xs rounded-xl border {paused ? 'bg-white text-black border-white' : 'bg-white/[0.04] text-zinc-200 border-white/[0.1]'}"
        >
          {paused ? "Lanjutkan" : "Jeda"}
        </button>
      {/if}
      <button
        type="button"
        onclick={toggleSound}
        aria-pressed={soundOn}
        class="h-9 px-3.5 font-semibold text-xs rounded-xl border {soundOn ? 'bg-white/[0.04] text-zinc-200 border-white/[0.1]' : 'bg-transparent text-zinc-500 border-white/[0.06]'}"
        title="Bunyi hitung mundur"
      >
        {soundOn ? "Suara: on" : "Suara: off"}
      </button>
      <button type="button" onclick={toggleFullscreen} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">
        Layar Penuh
      </button>
      <button
        type="button"
        onclick={stop}
        class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
        title="Tutup drawing live. Nomor yang sudah diundi tetap tersimpan; sisanya bisa dilanjutkan nanti."
      >
        {phase === "done" || (phase === "hold" && index >= categories.length - 1) ? "Tutup" : "Hentikan (lanjut nanti)"}
      </button>
    </div>
  </header>

  <!-- Progress: every category in ceremony order -->
  <div class="px-6 py-2 border-b border-white/[0.06] flex gap-1 shrink-0 overflow-x-auto scrollbar-thin">
    {#each categories as c, i (c.id)}
      <div
        class="h-1.5 min-w-6 flex-1 rounded-full {i < index || (i === index && (phase === 'hold' || phase === 'done')) ? 'bg-emerald-500' : i === index ? 'bg-amber-400' : 'bg-white/[0.08]'}"
        title={c.nama_nomor_tanding}
      ></div>
    {/each}
  </div>

  {#if current}
    <div class="px-4 pt-2 flex flex-wrap items-center justify-between gap-x-4 gap-y-1 shrink-0">
      <div class="min-w-0 flex items-center gap-3">
        <img src="/img/ambrilindo-logo-dark.png" alt="Ambrilindo" class="h-12 w-auto shrink-0 object-contain" />
        <h2 class="text-2xl md:text-3xl font-black tracking-tight text-white uppercase leading-tight truncate">
          {current.nama_nomor_tanding}
        </h2>
        <span class="text-[11px] font-mono font-bold tracking-widest text-amber-400 uppercase shrink-0">
          {DRAWING_GROUP_LABEL[drawingGroup(current)]}
        </span>
      </div>
      <div class="flex items-center gap-3 font-mono text-sm text-zinc-400">
        {#if paused}<span class="px-2 py-0.5 rounded bg-white/[0.06] text-zinc-200 text-[11px] font-bold">DIJEDA</span>{/if}
        <span>{current.athlete_count} {current.is_beregu ? "TIM" : "ATLET"}</span>
        {#if phase === "ready" && seedCount > 0}
          <span class="px-2 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 text-[11px] font-bold">★ {seedCount} UNGGULAN</span>
        {/if}
        {#if phase === "shuffle" && !manualStop && shown.length > 0}
          <!-- Countdown ring -->
          <div
            class="ld-ring"
            class:is-final={!paused && secondsLeft <= FINAL_SECONDS}
            class:is-paused={paused}
            role="timer"
            aria-label="Sisa {secondsLeft} detik"
          >
            <svg viewBox="0 0 100 100" aria-hidden="true">
              <circle class="ld-ring-track" cx="50" cy="50" r="44" />
              <circle class="ld-ring-bar" cx="50" cy="50" r="44" style="stroke-dashoffset: {276.46 * (1 - countdownProgress)}" />
            </svg>
            {#key secondsLeft}
              <span class="ld-ring-num">{secondsLeft}</span>
            {/key}
          </div>
        {:else if (phase === "reveal" || phase === "hold") && shown.length > 0}
          <div class="ld-ring is-done" aria-label="Terundi">
            <svg viewBox="0 0 100 100" aria-hidden="true"><circle class="ld-ring-track" cx="50" cy="50" r="44" /></svg>
            <svg class="ld-ring-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" /></svg>
          </div>
        {/if}
        <img src="/img/forki-logo.png" alt="FORKI" class="h-12 w-auto shrink-0 object-contain" />
      </div>
    </div>
    {#if phase === "shuffle" && !manualStop && shown.length > 0}
      <!-- Full-width time bar -->
      <div class="mx-4 mt-2 h-1.5 rounded-full bg-white/[0.06] overflow-hidden shrink-0">
        <div
          class="h-full rounded-full {paused ? 'bg-zinc-500' : secondsLeft <= FINAL_SECONDS ? 'bg-red-500' : 'bg-amber-400'}"
          style="width: {countdownProgress * 100}%"
        ></div>
      </div>
    {/if}
  {/if}

  <!-- Stage: the brackets -->
  <div bind:this={stageEl} class="relative flex-1 min-h-0 mx-4 my-2 overflow-hidden">
    {#if phase === "ready"}
      {#if athletesLoading}
        <div class="h-full flex items-center justify-center text-xs font-mono text-zinc-500">Memuat data atlet…</div>
      {:else if currentAthletes.length === 0}
        <div class="h-full flex items-center justify-center text-sm font-mono text-zinc-500">Belum ada data atlet untuk nomor ini</div>
      {:else}
        {@const L = tableLayout}
        {#key current?.id}
          <div class="h-full flex flex-col" style="font-size: {L.font}px;">
            <div class="grid shrink-0" style="grid-template-columns: repeat({L.cols}, minmax(0, 1fr)); column-gap: {TABLE_COL_GAP_PX}px; height: {TABLE_HEAD_PX}px;">
              {#each Array.from({ length: L.cols }) as _, c (c)}
                <div class="ld-th">
                  <span>No</span>
                  <span>{current?.is_beregu ? "Tim" : "Nama Atlet"}</span>
                  <span>Kontingen</span>
                  <span class="text-center" title="Unggulan (seeded)">★</span>
                </div>
              {/each}
            </div>
            <div
              class="grid flex-1 min-h-0 content-start"
              style="grid-auto-flow: column; grid-template-columns: repeat({L.cols}, minmax(0, 1fr)); grid-template-rows: repeat({L.rows}, {L.rowH}px); column-gap: {TABLE_COL_GAP_PX}px;"
            >
              {#each currentAthletes as a, i (a.id)}
                <div class="ld-tr" class:is-odd={i % 2 === 1} class:is-seed={a.is_priority} style="--d: {Math.min(i, 40) * 25}ms;">
                  <span class="ld-no">{i + 1}</span>
                  <span class="ld-name">{a.nama_atlet || "-"}</span>
                  <span class="ld-club">{a.utusan_nama || a.perguruan_nama || "-"}</span>
                  <button
                    type="button"
                    role="checkbox"
                    aria-checked={a.is_priority}
                    aria-label="Unggulan {a.nama_atlet}"
                    title={a.is_priority ? "Unggulan (klik untuk batal)" : "Jadikan unggulan"}
                    disabled={seedSaving[a.id]}
                    onclick={() => toggleSeed(a)}
                    class="ld-seed"
                    class:is-on={a.is_priority}
                  >{a.is_priority ? "★" : ""}</button>
                </div>
              {/each}
            </div>
          </div>
        {/key}
      {/if}
    {:else if phase === "error"}
      <div class="h-full flex flex-col items-center justify-center gap-4 text-center">
        <p class="text-sm text-red-300 max-w-xl">{errorMessage}</p>
        <div class="flex gap-2">
          <button type="button" onclick={runCategory} class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md">Ulangi</button>
          <button type="button" onclick={next} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">Lewati nomor ini</button>
        </div>
      </div>
    {:else if shown.length === 0}
      <div class="h-full flex items-center justify-center text-xs font-mono text-zinc-500">Mengundi…</div>
    {:else}
      <div class="h-full grid content-center justify-center" style="gap: {TREE_GAP_PX}px; grid-template-columns: repeat({treeCols}, max-content);">
        {#each draws as d, i (d.bagan.id)}
          {@const meta = deriveBracketMeta(d.bagan.nama_bagan || "", d.bagan.pool)}
          {@const treeW = treeWidthFor(treeFit.colWidth, treeFit.columns)}
          <div class="flex flex-col">
            {#if draws.length > 1}
              <div class="text-xs font-mono font-bold tracking-wider text-zinc-400 uppercase" style="height: {POOL_LABEL_PX}px;">{d.bagan.nama_bagan}</div>
            {/if}
            <div class="overflow-hidden" style="width: {treeW * treeFit.scale}px; height: {TREE_HEIGHT * treeFit.scale}px;">
              <div class="relative" style="transform: scale({treeFit.scale}); transform-origin: top left; width: {treeW}px; height: {TREE_HEIGHT}px;">
                <BracketTree
                  matches={shown[i] || d.matches}
                  {meta}
                  podium={computePodium(d.matches, meta)}
                  theme="dark"
                  colWidth={treeFit.colWidth}
                  showPodium={false}
                  cardScale={treeFit.cardScale}
                  hideScores
                  {skipRounds}
                />
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
    <!-- Giant countdown over the shuffling bracket, every second (red for the last ones) -->
    {#if phase === "shuffle" && !manualStop && !paused && shown.length > 0 && secondsLeft > 0}
      {#key secondsLeft}
        <div class="ld-final" class:is-final={secondsLeft <= FINAL_SECONDS} aria-hidden="true">{secondsLeft}</div>
      {/key}
    {/if}
    {#if paused && phase === "shuffle" && !manualStop && shown.length > 0}
      <div class="ld-paused" aria-hidden="true">
        <span class="ld-paused-title">JEDA</span>
        <span class="ld-paused-sub">Lanjut: hitung ulang {Math.round(durationMs / 1000)} detik</span>
      </div>
    {/if}
    <!-- Lock-in flash: every slot shows its real athlete at the same moment -->
    {#key flashId}
      {#if flashId > 0}
        <div class="ld-flash" aria-hidden="true"></div>
      {/if}
    {/key}
    <!-- Controls float over the bracket's bottom-right corner (empty: no podium here) -->
  {#if phase === "ready" && !athletesLoading}
    <div class="absolute bottom-3 right-3 z-30 flex items-center gap-3 font-mono p-2 rounded-xl bg-[#0d0e12]/85 backdrop-blur-xs">
      {#if introLeftMs > 0}
        <span class="text-xs text-zinc-400">{paused ? "Dijeda" : `Undian mulai dalam ${Math.ceil(introLeftMs / 1000)} dtk`}</span>
      {/if}
      <button
        type="button"
        onclick={runCategory}
        class="h-12 px-8 bg-white text-black hover:bg-zinc-200 font-black text-sm rounded-xl shadow-md uppercase tracking-wider"
        title="Enter"
      >
        {index > 0 ? "Mulai undi nomor ini" : "Mulai Undi"}
      </button>
    </div>
  {/if}
  {#if phase === "shuffle" || phase === "hold"}
    <div class="absolute bottom-3 right-3 z-30 flex flex-wrap items-center justify-end gap-2 font-mono max-w-[60%] p-2 rounded-xl bg-[#0d0e12]/85 backdrop-blur-xs">
      {#if phase === "shuffle" && manualStop && !stopShuffle}
        <button
          type="button"
          onclick={() => (stopShuffle = true)}
          disabled={shown.length === 0}
          class="h-12 px-8 bg-white text-black hover:bg-zinc-200 font-black text-sm rounded-xl shadow-md uppercase tracking-wider disabled:opacity-40"
        >
          Stop Acak
        </button>
      {:else if phase === "shuffle"}
        <button type="button" onclick={skipAnimation} class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 border border-white/[0.1] font-semibold text-xs rounded-xl">
          Tampilkan hasil sekarang
        </button>
      {:else}
        <button
          type="button"
          onclick={redraw}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 border border-white/[0.1] font-semibold text-xs rounded-xl"
          title="Undi ulang nomor ini (bagan lama diganti)"
        >
          Undi ulang nomor ini
        </button>
        {#if index < categories.length - 1}
          {#if autoNext}
            <span class="text-xs text-zinc-500">{paused ? "Dijeda" : `Lanjut dalam ${Math.ceil(holdLeftMs / 1000)} dtk`}</span>
          {/if}
          <button type="button" onclick={next} class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md">
            Nomor berikutnya: {categories[index + 1].nama_nomor_tanding}
          </button>
        {:else}
          <button type="button" onclick={stop} class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md">
            Selesai · Lihat bagan
          </button>
        {/if}
      {/if}
    </div>
  {/if}
  </div>

</div>

<style>
  /* Countdown ring next to the category name */
  .ld-ring {
    position: relative;
    width: 84px;
    height: 84px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    color: #fbbf24;
  }
  .ld-ring svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }
  .ld-ring-track {
    fill: rgba(255, 255, 255, 0.04);
    stroke: rgba(255, 255, 255, 0.1);
    stroke-width: 7;
  }
  .ld-ring-bar {
    fill: none;
    stroke: currentColor;
    stroke-width: 7;
    stroke-linecap: round;
    stroke-dasharray: 276.46;
    transition: stroke-dashoffset 80ms linear;
    filter: drop-shadow(0 0 6px currentColor);
  }
  .ld-ring-num {
    position: relative;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 34px;
    font-weight: 900;
    line-height: 1;
    color: #ffffff;
    animation: ld-num-pop 360ms cubic-bezier(0.2, 1.4, 0.4, 1);
  }
  .ld-ring.is-final { color: #ef4444; animation: ld-pulse 1s ease-in-out infinite; }
  .ld-ring.is-paused { color: #71717a; }
  .ld-ring.is-paused .ld-ring-num { color: #a1a1aa; animation: none; }
  .ld-ring.is-done { color: #34d399; }
  .ld-ring.is-done .ld-ring-track { stroke: #34d399; }
  .ld-ring-check {
    position: relative;
    width: 38px;
    height: 38px;
    color: #34d399;
    animation: ld-num-pop 420ms cubic-bezier(0.2, 1.4, 0.4, 1);
  }

  /* Giant last seconds over the stage */
  .ld-final {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    pointer-events: none;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-weight: 900;
    font-size: min(46vh, 40vw);
    line-height: 1;
    color: #ffffff;
    text-shadow:
      0 0 40px rgba(251, 191, 36, 0.85),
      0 0 120px rgba(251, 191, 36, 0.45);
    animation: ld-final 1s ease-out forwards;
    opacity: 0;
  }
  .ld-final.is-final {
    text-shadow:
      0 0 40px rgba(239, 68, 68, 0.9),
      0 0 120px rgba(239, 68, 68, 0.6);
  }

  /* Opening athlete table (one screen, readable from the hall) */
  .ld-th,
  .ld-tr {
    display: grid;
    grid-template-columns: 2.6em minmax(0, 1.35fr) minmax(0, 1fr) 2.2em;
    align-items: center;
    column-gap: 0.7em;
    padding: 0 0.7em;
  }
  .ld-th {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #a1a1aa;
    background: #111215;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
  }
  .ld-tr {
    min-width: 0;
    height: 100%;
    border-radius: 8px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    animation: ld-row-in 420ms ease-out both;
    animation-delay: var(--d, 0ms);
  }
  .ld-tr.is-odd { background: rgba(255, 255, 255, 0.035); }
  .ld-tr.is-seed { background: rgba(251, 191, 36, 0.12); box-shadow: inset 3px 0 0 #fbbf24; }
  .ld-tr.is-seed .ld-name { color: #fde68a; }
  /* Seeded checkbox: a box that shows a star when ticked */
  .ld-seed {
    justify-self: center;
    width: 1.4em;
    height: 1.4em;
    min-width: 18px;
    min-height: 18px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 2px solid rgba(255, 255, 255, 0.28);
    background: rgba(0, 0, 0, 0.25);
    color: #111215;
    font-size: 0.9em;
    font-weight: 900;
    line-height: 1;
    cursor: pointer;
    transition: background-color 120ms, border-color 120ms, transform 120ms;
  }
  .ld-seed:hover { border-color: #fbbf24; }
  .ld-seed:active { transform: scale(0.92); }
  .ld-seed.is-on { background: #fbbf24; border-color: #fbbf24; }
  .ld-seed:disabled { opacity: 0.5; cursor: progress; }
  .ld-no {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-weight: 900;
    color: #fbbf24;
    text-align: right;
  }
  .ld-name {
    font-weight: 900;
    color: #ffffff;
    text-transform: uppercase;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ld-club {
    font-weight: 700;
    font-size: 0.88em;
    color: #d4d4d8;
    text-transform: uppercase;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  @keyframes ld-row-in {
    0% { opacity: 0; transform: translateY(6px); }
    100% { opacity: 1; transform: translateY(0); }
  }

  /* Paused: the countdown will start again from the full time */
  .ld-paused {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    pointer-events: none;
    background: rgba(13, 14, 18, 0.55);
    backdrop-filter: blur(2px);
  }
  .ld-paused-title {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: min(16vh, 14vw);
    font-weight: 900;
    line-height: 1;
    letter-spacing: 0.08em;
    color: #ffffff;
  }
  .ld-paused-sub {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 15px;
    font-weight: 700;
    color: #fbbf24;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  /* Lock-in flash */
  .ld-flash {
    position: absolute;
    inset: 0;
    z-index: 25;
    pointer-events: none;
    background: radial-gradient(circle at center, rgba(255, 255, 255, 0.85) 0%, rgba(251, 191, 36, 0.35) 40%, rgba(251, 191, 36, 0) 75%);
    animation: ld-flash 900ms ease-out forwards;
  }

  @keyframes ld-num-pop {
    0% { transform: scale(1.6); opacity: 0; }
    100% { transform: scale(1); opacity: 1; }
  }
  @keyframes ld-pulse {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.08); }
  }
  @keyframes ld-final {
    0% { transform: scale(1.8); opacity: 0; }
    18% { transform: scale(1); opacity: 0.95; }
    75% { opacity: 0.85; }
    100% { transform: scale(0.7); opacity: 0; }
  }
  @keyframes ld-flash {
    0% { opacity: 0; }
    15% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
