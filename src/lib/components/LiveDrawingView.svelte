<script lang="ts">
  // Drawing live: one category at a time, in ceremony order, full screen for the coaches.
  // The host draws the category first (same anti-clash drawing as the normal button), then
  // the real bracket is shown in the usual bracket template: its first-round slots shuffle
  // through the participants every few milliseconds and lock in to the real draw one by one.
  // The animation never decides anything. Progress is saved, so a stopped ceremony can be
  // continued later from the next category.
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { BaganItem, DetailBaganItem, DrawingCategoryInfo, DrawingSummary } from "$lib/types";
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
  const LOCK_STEP_MS = 160; // one slot locks in to its real athlete every step

  type Phase = "ready" | "shuffle" | "reveal" | "hold" | "error" | "done";
  interface Occupant {
    id: number | null;
    nama: string | null;
    kontingen: string | null;
    logo: string | null;
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
  /** waits ms of un-paused time */
  async function wait(ms: number) {
    let left = ms;
    while (left > 0 && !destroyed && !skip) {
      await sleep(40);
      if (!paused) left -= 40;
    }
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
      ? { id: m.atlet1_id, nama: m.atlet1_nama ?? null, kontingen: m.atlet1_kontingen ?? null, logo: (m as any).atlet1_logo ?? null }
      : { id: m.atlet2_id, nama: m.atlet2_nama ?? null, kontingen: m.atlet2_kontingen ?? null, logo: (m as any).atlet2_logo ?? null };
  }

  function withOccupant(m: DetailBaganItem, side: 1 | 2, o: Occupant): DetailBaganItem {
    return side === 1
      ? ({ ...m, atlet1_id: o.id, atlet1_nama: o.nama, atlet1_kontingen: o.kontingen, atlet1_logo: o.logo } as DetailBaganItem)
      : ({ ...m, atlet2_id: o.id, atlet2_nama: o.nama, atlet2_kontingen: o.kontingen, atlet2_logo: o.logo } as DetailBaganItem);
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
      if (!paused) render(locked);
    }, Math.max(10, shuffleMs));
    if (manualStop) {
      // No timer: keep shuffling until the operator stops it
      while (!stopShuffle && !skip && !destroyed) await sleep(40);
    } else {
      await wait(durationMs);
    }

    // Lock in the real draw slot by slot (the others keep shuffling)
    phase = "reveal";
    while (locked < totalSlots && !destroyed && !skip) {
      await wait(LOCK_STEP_MS);
      if (!paused) locked += 1;
    }
    stopTicker();
    locked = totalSlots;
    render(locked);
    if (destroyed) return;
    saveLiveProgress(eventId, categories, index + 1);
    startHold();
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
    runCategory();
  }

  function redraw() {
    // The host replaces a category's bracket when it is drawn again (no results yet)
    runCategory();
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
      if (phase !== "ready") paused = !paused;
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
      {#if phase !== "ready" && phase !== "done" && phase !== "error"}
        <button
          type="button"
          onclick={() => (paused = !paused)}
          class="h-9 px-3.5 font-semibold text-xs rounded-xl border {paused ? 'bg-white text-black border-white' : 'bg-white/[0.04] text-zinc-200 border-white/[0.1]'}"
        >
          {paused ? "Lanjutkan" : "Jeda"}
        </button>
      {/if}
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
    <div class="px-4 pt-2 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 shrink-0">
      <div class="min-w-0 flex items-baseline gap-3">
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
      </div>
    </div>
  {/if}

  <!-- Stage: the brackets -->
  <div bind:this={stageEl} class="relative flex-1 min-h-0 mx-4 my-2 overflow-hidden">
    {#if phase === "ready"}
      <div class="h-full flex flex-col items-center justify-center gap-4">
        <button
          type="button"
          onclick={runCategory}
          class="h-12 px-8 bg-white text-black hover:bg-zinc-200 font-black text-sm rounded-xl shadow-md font-mono uppercase tracking-wider"
        >
          {index > 0 ? `Lanjutkan undian (nomor ${index + 1})` : "Mulai Undi"}
        </button>
        <span class="text-xs font-mono text-zinc-500">Enter = mulai / lanjut · Spasi = jeda</span>
      </div>
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
    <!-- Controls float over the bracket's bottom-right corner (empty: no podium here) -->
  {#if phase === "shuffle" || phase === "reveal" || phase === "hold"}
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
      {:else if phase === "shuffle" || phase === "reveal"}
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
