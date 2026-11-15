<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import type {
    DrawingCategoryInfo,
    DrawingSummary,
    ManualBracketPreview,
    ManualDrawingAthlete,
    ManualPreviewMatch,
    ManualSlotInput,
  } from "$lib/types";

  interface Props {
    eventId: number;
    onClose: () => void;
    /** Called after the bracket was saved. */
    onSuccess: (summary: DrawingSummary, category: DrawingCategoryInfo) => void;
  }

  let { eventId, onClose, onSuccess }: Props = $props();

  const MAX_ATHLETES = 16;
  const R1_MATCHES = MAX_ATHLETES / 2;

  /** Moves keyboard focus into a newly shown step / dialog (the modal opens over other UI). */
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }

  // Step 1: category
  let categories = $state<DrawingCategoryInfo[]>([]);
  let isLoading = $state(true);
  let loadError = $state<string | null>(null);
  let categorySearch = $state("");
  let category = $state<DrawingCategoryInfo | null>(null);
  let hasVr = $state(false);

  // Step 2: placement. slots[i] = athlete id; partai = floor(i / 2) + 1, posisi = i % 2 + 1.
  let athletes = $state<ManualDrawingAthlete[]>([]);
  let slots = $state<(number | null)[]>(Array(MAX_ATHLETES).fill(null));
  let selectedAthleteId = $state<number | null>(null);
  let poolSearch = $state("");
  let isLoadingAthletes = $state(false);
  let isSaving = $state(false);
  let saveError = $state<string | null>(null);
  /** Pending "discard placement?" question: what to do if the operator says yes. */
  let confirmLeave = $state<null | "back" | "close">(null);

  let eligible = $derived(
    categories
      .filter((c) => !c.nama_nomor_tanding.toLowerCase().includes("festival"))
      .filter((c) => c.nama_nomor_tanding.toLowerCase().includes(categorySearch.trim().toLowerCase()))
  );

  let byId = $derived(new Map(athletes.map((a) => [a.id, a])));
  let placedIds = $derived(new Set(slots.filter((s): s is number => s !== null)));
  let placedCount = $derived(placedIds.size);
  let allPlaced = $derived(athletes.length > 0 && placedCount === athletes.length);

  let pool = $derived(
    athletes
      .filter((a) => !placedIds.has(a.id))
      .filter((a) => {
        const q = poolSearch.trim().toLowerCase();
        if (!q) return true;
        return [a.nama_atlet, a.utusan_nama, a.perguruan_nama].some((v) => (v || "").toLowerCase().includes(q));
      })
  );

  /** Round-1 bouts where two athletes of the same kontingen or perguruan meet. */
  function collisionOf(matchIndex: number): string | null {
    const a = slots[matchIndex * 2] != null ? byId.get(slots[matchIndex * 2]!) : undefined;
    const b = slots[matchIndex * 2 + 1] != null ? byId.get(slots[matchIndex * 2 + 1]!) : undefined;
    if (!a || !b) return null;
    if (a.utusan_id != null && a.utusan_id === b.utusan_id) return `Satu kontingen: ${a.utusan_nama || "-"}`;
    if (a.perguruan_id != null && a.perguruan_id === b.perguruan_id) return `Satu perguruan: ${a.perguruan_nama || "-"}`;
    return null;
  }

  let collisionCount = $derived(
    Array.from({ length: R1_MATCHES }, (_, m) => collisionOf(m)).filter(Boolean).length
  );

  // ---- Bracket geometry (fixed 16-slot tree: 8 → 4 → 2 → final) ----
  const ROUND_TITLES = ["16 besar", "Perempat final", "Semifinal", "Final"];
  const SLOT_H = 34;
  const CARD_H = 18 + SLOT_H * 2 + 2; // header + two slots + border
  const UNIT = CARD_H + 12;
  const COL_W = 200;
  const GAP_X = 40;
  const TREE_W = 4 * COL_W + 3 * GAP_X;
  const TREE_H = R1_MATCHES * UNIT - 12;
  const MATCH_OFFSET = [0, 8, 12, 14];

  const colX = (r: number) => r * (COL_W + GAP_X);
  function cardCenter(r: number, j: number): number {
    if (r === 0) return j * UNIT + CARD_H / 2;
    return (cardCenter(r - 1, 2 * j) + cardCenter(r - 1, 2 * j + 1)) / 2;
  }
  const cardTop = (r: number, j: number) => cardCenter(r, j) - CARD_H / 2;
  /** Bout number across the bracket: round 1 = 1–8, quarterfinal = 9–12, semifinal = 13–14, final = 15. */
  const matchNumber = (r: number, j: number) => MATCH_OFFSET[r] + j + 1;

  const connectorPaths: string[] = [1, 2, 3].flatMap((r) =>
    Array.from({ length: R1_MATCHES >> r }, (_, j) => {
      const xa = colX(r - 1) + COL_W;
      const xm = xa + GAP_X / 2;
      const ya = cardCenter(r - 1, 2 * j);
      const yb = cardCenter(r - 1, 2 * j + 1);
      return `M${xa} ${ya} H${xm} V${yb} H${xa} M${xm} ${cardCenter(r, j)} H${colX(r)}`;
    })
  );

  // ---- Live preview of later rounds (BYEs advanced by the same engine that saves) ----
  function slotPayload(): ManualSlotInput[] {
    return slots.flatMap((atletId, i) =>
      atletId === null ? [] : [{ atlet_id: atletId, match_num: Math.floor(i / 2) + 1, position: (i % 2) + 1 }]
    );
  }

  let preview = $state<Map<string, ManualPreviewMatch>>(new Map());
  let previewSeq = 0;
  $effect(() => {
    const cat = category;
    const payload = slotPayload();
    const seq = ++previewSeq;
    if (!cat || payload.length === 0) {
      preview = new Map();
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const res = await invoke<ManualBracketPreview>("preview_manual_drawing_bagan", { categoryId: cat.id, slots: payload });
        if (seq === previewSeq) preview = new Map(res.matches.map((m) => [`${m.round}-${m.urutan}`, m]));
      } catch {
        if (seq === previewSeq) preview = new Map();
      }
    }, 120);
    return () => clearTimeout(timer);
  });

  /**
   * Bouts that could never be played: one side empty while the other side still has a
   * bout to win first. Same rule as `unreachable_bouts` in bracket.rs (the save refuses these).
   */
  let deadBouts = $derived.by(() => {
    const dead = new Set<number>();
    for (let r = 1; r <= 3; r++) {
      for (let j = 0; j < R1_MATCHES >> r; j++) {
        const a = subtreeCount(r - 1, 2 * j);
        const b = subtreeCount(r - 1, 2 * j + 1);
        if ((a === 0 && b >= 2) || (b === 0 && a >= 2)) dead.add(matchNumber(r, j));
      }
    }
    return dead;
  });
  let deadList = $derived([...deadBouts].sort((x, y) => x - y).join(", "));

  /** How many athletes sit in round 1 under this bout. */
  function subtreeCount(r: number, j: number): number {
    if (r === 0) return (slots[2 * j] != null ? 1 : 0) + (slots[2 * j + 1] != null ? 1 : 0);
    return subtreeCount(r - 1, 2 * j) + subtreeCount(r - 1, 2 * j + 1);
  }

  /** One side of a later-round bout: an athlete already through on a BYE, or what will fill it. */
  function previewSlot(r: number, j: number, p: number): { athlete?: ManualDrawingAthlete; label: string } {
    const m = preview.get(`${r + 1}-${j + 1}`);
    const id = p === 0 ? m?.atlet1_id : m?.atlet2_id;
    const athlete = id != null ? byId.get(id) : undefined;
    if (athlete) return { athlete, label: "" };
    const n = subtreeCount(r - 1, 2 * j + p);
    if (n >= 2) return { label: `Pemenang partai ${matchNumber(r - 1, 2 * j + p)}` };
    if (n === 1) return { label: "BYE" };
    return { label: "Kosong" };
  }

  function categoryBlockedReason(c: DrawingCategoryInfo): string | null {
    if (c.athlete_count === 0) return "Belum ada atlet";
    if (c.athlete_count > MAX_ATHLETES) return "Lebih dari 16 atlet — pakai drawing otomatis";
    if ((c.finished_matches || 0) > 0) return "Sudah ada hasil pertandingan";
    return null;
  }

  async function loadCategories() {
    isLoading = true;
    loadError = null;
    try {
      categories = await invoke<DrawingCategoryInfo[]>("get_drawing_eligible_categories", { eventId });
    } catch (err: any) {
      loadError = typeof err === "string" ? err : err?.message || "Gagal memuat nomor tanding.";
    } finally {
      isLoading = false;
    }
  }

  async function chooseCategory(c: DrawingCategoryInfo) {
    if (categoryBlockedReason(c)) return;
    category = c;
    hasVr = c.is_kumite && c.has_vr;
    slots = Array(MAX_ATHLETES).fill(null);
    selectedAthleteId = null;
    poolSearch = "";
    saveError = null;
    isLoadingAthletes = true;
    try {
      athletes = await invoke<ManualDrawingAthlete[]>("get_manual_drawing_athletes", { categoryId: c.id });
    } catch (err: any) {
      saveError = typeof err === "string" ? err : err?.message || "Gagal memuat atlet.";
      athletes = [];
    } finally {
      isLoadingAthletes = false;
    }
  }

  function pickAthlete(id: number) {
    selectedAthleteId = selectedAthleteId === id ? null : id;
  }

  function clickSlot(i: number) {
    const current = slots[i];
    const next = [...slots];
    if (selectedAthleteId !== null) {
      // Put the picked athlete here; whoever was here goes back to the pool.
      next[i] = selectedAthleteId;
      slots = next;
      selectedAthleteId = null;
    } else if (current !== null) {
      next[i] = null;
      slots = next;
    }
  }

  function clearAll() {
    slots = Array(MAX_ATHLETES).fill(null);
    selectedAthleteId = null;
  }

  function goBack() {
    category = null;
    athletes = [];
    saveError = null;
    clearAll();
  }

  function backToCategories() {
    if (placedCount > 0) confirmLeave = "back";
    else goBack();
  }

  function requestClose() {
    if (isSaving) return;
    if (category && placedCount > 0) confirmLeave = "close";
    else onClose();
  }

  function discardAndLeave() {
    const action = confirmLeave;
    confirmLeave = null;
    if (action === "back") goBack();
    else if (action === "close") onClose();
  }

  async function save() {
    if (!category || !allPlaced || isSaving) return;
    isSaving = true;
    saveError = null;
    try {
      const payload = slotPayload();
      const summary = await invoke<DrawingSummary>("execute_manual_drawing_bagan", {
        eventId,
        categoryId: category.id,
        slots: payload,
        hasVr: category.is_kumite && hasVr,
      });
      onSuccess(summary, category);
    } catch (err: any) {
      saveError = typeof err === "string" ? err : err?.message || "Gagal menyimpan bagan.";
    } finally {
      isSaving = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    if (confirmLeave) confirmLeave = null;
    else if (selectedAthleteId !== null) selectedAthleteId = null;
    else requestClose();
  }

  onMount(loadCategories);
</script>

<svelte:window onkeydown={onKeydown} />

<div class="fixed inset-0 z-60 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs font-sans select-none">
  <div
    class="bg-[#181a20] border border-white/10 rounded-2xl shadow-2xl w-full max-w-[1320px] h-[88vh] flex flex-col overflow-hidden"
    role="dialog"
    aria-modal="true"
    aria-labelledby="manual-drawing-title"
  >
    <!-- Header -->
    <div class="px-6 py-4 border-b border-white/[0.08] flex items-center justify-between gap-4 shrink-0">
      <div class="flex items-center gap-3 min-w-0">
        {#if category}
          <button
            type="button"
            onclick={backToCategories}
            class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
            aria-label="Kembali ke daftar nomor tanding"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" /></svg>
          </button>
        {/if}
        <h2 id="manual-drawing-title" class="text-lg font-black text-white tracking-tight shrink-0">Drawing manual</h2>
        {#if category}
          <span class="text-sm text-zinc-400 truncate" title={category.nama_nomor_tanding}>{category.nama_nomor_tanding}</span>
        {/if}
      </div>
      <button
        type="button"
        onclick={requestClose}
        class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        aria-label="Tutup"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
      </button>
    </div>

    {#if !category}
      <!-- Step 1: choose a category -->
      <div class="px-6 pt-4 shrink-0">
        <div class="relative w-72">
          <svg class="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-4.35-4.35M17 11a6 6 0 11-12 0 6 6 0 0112 0z" /></svg>
          <input
            type="text"
            bind:value={categorySearch}
            use:focusOnMount
            placeholder="Cari nomor tanding"
            aria-label="Cari nomor tanding"
            class="w-full bg-[#111215] text-xs text-white pl-8 pr-3 py-2 rounded-xl border border-white/[0.1] focus:border-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          />
        </div>
      </div>
      <div class="flex-1 overflow-y-auto min-h-0 px-6 py-4">
        {#if isLoading}
          <p class="text-sm text-zinc-400">Memuat nomor tanding…</p>
        {:else if loadError}
          <p class="text-sm text-red-400" role="alert">{loadError}</p>
        {:else if eligible.length === 0}
          <p class="text-sm text-zinc-400">Tidak ada nomor tanding prestasi.</p>
        {:else}
          <ul class="divide-y divide-white/[0.06] rounded-xl border border-white/[0.08] overflow-hidden">
            {#each eligible as c (c.id)}
              {@const blocked = categoryBlockedReason(c)}
              <li>
                <button
                  type="button"
                  disabled={!!blocked}
                  onclick={() => chooseCategory(c)}
                  class="w-full flex items-center justify-between gap-4 px-4 py-3 text-left transition {blocked ? 'opacity-50 cursor-not-allowed' : 'hover:bg-white/[0.03] cursor-pointer'} focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-white/40"
                >
                  <span class="min-w-0">
                    <span class="block text-sm font-semibold text-white truncate">{c.nama_nomor_tanding}</span>
                    {#if blocked}
                      <span class="block text-xs text-zinc-400 mt-0.5">{blocked}</span>
                    {:else if c.bagan_count > 0}
                      <span class="flex items-center gap-1 text-xs text-amber-300 mt-0.5">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.008M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" /></svg>
                        Bagan lama akan diganti
                      </span>
                    {/if}
                  </span>
                  <span class="text-xs text-zinc-400 tabular-nums shrink-0">{c.athlete_count} atlet</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else}
      <!-- Step 2: place athletes -->
      <div class="flex-1 min-h-0 flex">
        <!-- Pool -->
        <div class="w-64 shrink-0 border-r border-white/[0.08] flex flex-col min-h-0">
          <div class="px-4 pt-4 pb-2 shrink-0">
            <div class="flex items-baseline justify-between mb-2">
              <span class="text-xs font-semibold text-zinc-300">Belum ditempatkan</span>
              <span class="text-xs text-zinc-400 tabular-nums">{athletes.length - placedCount}</span>
            </div>
            <input
              type="text"
              bind:value={poolSearch}
              use:focusOnMount
              placeholder="Cari atlet, kontingen"
              aria-label="Cari atlet"
              class="w-full bg-[#111215] text-xs text-white px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
            />
          </div>
          <div class="flex-1 overflow-y-auto min-h-0 px-2 pb-3">
            {#if isLoadingAthletes}
              <p class="px-2 text-sm text-zinc-400">Memuat atlet…</p>
            {:else if pool.length === 0}
              <p class="px-2 py-2 text-xs text-zinc-400">{placedCount === athletes.length ? "Semua atlet sudah ditempatkan." : "Tidak ada yang cocok."}</p>
            {:else}
              {#each pool as a (a.id)}
                <button
                  type="button"
                  onclick={() => pickAthlete(a.id)}
                  aria-pressed={selectedAthleteId === a.id}
                  class="w-full text-left px-2.5 py-2 rounded-lg transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {selectedAthleteId === a.id ? 'bg-white text-black' : 'hover:bg-white/[0.05]'}"
                >
                  <span class="flex items-center gap-1.5 text-sm font-semibold {selectedAthleteId === a.id ? 'text-black' : 'text-white'}">
                    {#if a.is_priority}
                      <svg class="w-3.5 h-3.5 shrink-0 {selectedAthleteId === a.id ? 'text-amber-600' : 'text-amber-300'}" viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Seed"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" /></svg>
                    {/if}
                    <span class="truncate">{a.nama_atlet || `Atlet #${a.id}`}</span>
                  </span>
                  <span class="block text-xs truncate {selectedAthleteId === a.id ? 'text-zinc-700' : 'text-zinc-400'}">
                    {a.utusan_nama || "-"}{a.perguruan_nama ? ` · ${a.perguruan_nama}` : ""}
                  </span>
                </button>
              {/each}
            {/if}
          </div>
        </div>

        <!-- Bracket: round 1 is placed by the operator; later rounds are the live preview -->
        <div class="flex-1 min-w-0 flex flex-col min-h-0">
          <div class="flex items-center justify-between gap-3 px-5 pt-4 pb-3 shrink-0">
            <span class="text-xs text-zinc-400" aria-live="polite">
              {selectedAthleteId !== null ? "Klik slot di babak 16 besar untuk menempatkan atlet." : "Pilih atlet di kiri, lalu klik slot. Slot kosong = BYE."}
            </span>
            <div class="flex items-center gap-2 shrink-0">
              {#if category.is_kumite}
                <label class="inline-flex items-center gap-1.5 h-8 px-2 rounded-lg text-xs text-zinc-300 cursor-pointer hover:bg-white/[0.05] focus-within:ring-2 focus-within:ring-white/40">
                  <input type="checkbox" bind:checked={hasVr} class="accent-white" />
                  VR
                </label>
              {/if}
              <button
                type="button"
                onclick={clearAll}
                disabled={placedCount === 0}
                class="h-8 px-3 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl transition hover:bg-white/[0.08] disabled:opacity-40 cursor-pointer disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
              >
                Kosongkan semua
              </button>
            </div>
          </div>

          <div class="flex-1 min-h-0 overflow-auto px-5 pb-5">
            <!-- Round titles -->
            <div class="relative mb-2" style="width: {TREE_W}px; height: 16px;">
              {#each ROUND_TITLES as title, r (r)}
                <span class="absolute text-[11px] font-semibold text-zinc-400" style="left: {colX(r)}px;">{title}</span>
              {/each}
            </div>

            <div class="relative" style="width: {TREE_W}px; height: {TREE_H}px;">
              <!-- Connector lines (decorative; every card is named by its partai number) -->
              <svg class="absolute inset-0 pointer-events-none" width={TREE_W} height={TREE_H} aria-hidden="true">
                {#each connectorPaths as d, k (k)}
                  <path {d} fill="none" stroke="#71717a" stroke-width="1.5" />
                {/each}
              </svg>

              <!-- Round 1: the operator's slots -->
              {#each Array.from({ length: R1_MATCHES }, (_, m) => m) as m (m)}
                {@const collision = collisionOf(m)}
                <div
                  class="absolute rounded-lg border bg-[#111215] overflow-hidden {collision ? 'border-amber-400/70' : 'border-white/[0.14]'}"
                  style="left: {colX(0)}px; top: {cardTop(0, m)}px; width: {COL_W}px; height: {CARD_H}px;"
                  role="group"
                  aria-label="Partai {m + 1}"
                >
                  <div class="flex items-center justify-between gap-2 h-[18px] px-2 border-b border-white/[0.08]">
                    <span class="text-[11px] font-mono font-semibold text-zinc-400">Partai {m + 1}</span>
                    {#if collision}
                      <span class="flex items-center gap-1 min-w-0 text-[11px] text-amber-300" title={collision}>
                        <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.008M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" /></svg>
                        <span class="truncate">{collision.startsWith("Satu kontingen") ? "Satu kontingen" : "Satu perguruan"}</span>
                      </span>
                    {/if}
                  </div>
                  {#each [m * 2, m * 2 + 1] as i (i)}
                    {@const a = slots[i] != null ? byId.get(slots[i]!) : undefined}
                    <button
                      type="button"
                      onclick={() => clickSlot(i)}
                      disabled={!a && selectedAthleteId === null}
                      aria-label={a ? `Slot ${i + 1}: ${a.nama_atlet}. Klik untuk ${selectedAthleteId !== null ? "mengganti" : "mengembalikan ke daftar"}` : `Slot ${i + 1} kosong`}
                      class="w-full flex items-center gap-2 px-2 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-white/50
                        {i % 2 === 0 ? 'border-b border-white/[0.06]' : ''}
                        {a ? 'hover:bg-white/[0.06] cursor-pointer' : selectedAthleteId !== null ? 'bg-white/[0.03] hover:bg-white/[0.08] cursor-pointer' : 'cursor-default'}"
                      style="height: {SLOT_H}px;"
                    >
                      <span class="w-4 text-right text-[11px] font-mono text-zinc-400 tabular-nums shrink-0">{i + 1}</span>
                      {#if a}
                        <span class="min-w-0 flex-1">
                          <span class="flex items-center gap-1 text-xs font-semibold text-white leading-tight">
                            {#if a.is_priority}
                              <svg class="w-3 h-3 shrink-0 text-amber-300" viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Seed"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" /></svg>
                            {/if}
                            <span class="truncate">{a.nama_atlet || `Atlet #${a.id}`}</span>
                          </span>
                          <span class="block text-[11px] text-zinc-400 truncate leading-tight">{a.utusan_nama || "-"}</span>
                        </span>
                      {:else}
                        <span class="text-[11px] {selectedAthleteId !== null ? 'text-zinc-200' : 'text-zinc-400'}">{selectedAthleteId !== null ? "Taruh di sini" : "Kosong"}</span>
                      {/if}
                    </button>
                  {/each}
                </div>
              {/each}

              <!-- Rounds 2-4: preview (BYE winners already advanced) -->
              {#each [1, 2, 3] as r (r)}
                {#each Array.from({ length: R1_MATCHES >> r }, (_, j) => j) as j (j)}
                  {@const dead = deadBouts.has(matchNumber(r, j))}
                  <div
                    class="absolute rounded-lg border bg-[#14161b] overflow-hidden {dead ? 'border-red-400/80' : r === 3 ? 'border-amber-400/50' : 'border-white/[0.1]'}"
                    style="left: {colX(r)}px; top: {cardTop(r, j)}px; width: {COL_W}px; height: {CARD_H}px;"
                    role="group"
                    aria-label="Partai {matchNumber(r, j)}{dead ? ', tidak akan punya lawan' : ''}"
                  >
                    <div class="flex items-center justify-between gap-2 h-[18px] px-2 border-b border-white/[0.06]">
                      <span class="text-[11px] font-mono font-semibold text-zinc-400">Partai {matchNumber(r, j)}</span>
                      {#if dead}
                        <span class="flex items-center gap-1 text-[11px] text-red-300">
                          <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.008M21 12a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
                          Tanpa lawan
                        </span>
                      {/if}
                    </div>
                    {#each [0, 1] as p (p)}
                      {@const slot = previewSlot(r, j, p)}
                      <div class="flex items-center gap-2 px-2 {p === 0 ? 'border-b border-white/[0.05]' : ''}" style="height: {SLOT_H}px;">
                        {#if slot.athlete}
                          <span class="min-w-0 flex-1">
                            <span class="block text-xs font-semibold text-zinc-100 truncate leading-tight">{slot.athlete.nama_atlet || `Atlet #${slot.athlete.id}`}</span>
                            <span class="block text-[11px] text-zinc-400 truncate leading-tight">Lolos BYE · {slot.athlete.utusan_nama || "-"}</span>
                          </span>
                        {:else}
                          <span class="text-[11px] text-zinc-400 truncate">{slot.label}</span>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {/each}
              {/each}
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-3.5 border-t border-white/[0.08] flex items-center justify-between gap-4 shrink-0">
        <div class="text-xs min-w-0">
          {#if saveError}
            <span class="text-red-400" role="alert">{saveError}</span>
          {:else}
            <span role="status" aria-live="polite" aria-atomic="true">
              <span class="text-zinc-300 tabular-nums">{placedCount} dari {athletes.length} atlet ditempatkan</span>
              {#if collisionCount > 0}
                <span class="text-amber-300"> · {collisionCount} partai satu kontingen/perguruan</span>
              {/if}
              {#if deadBouts.size > 0}
                <span class="text-red-300"> · Partai {deadList} tanpa lawan: pindahkan atlet ke sisi yang kosong</span>
              {/if}
            </span>
          {/if}
        </div>
        <button
          type="button"
          onclick={save}
          disabled={!allPlaced || deadBouts.size > 0 || isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition disabled:opacity-40 disabled:cursor-not-allowed shrink-0 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        >
          {isSaving ? "Menyimpan…" : category.bagan_count > 0 ? "Ganti bagan" : "Simpan bagan"}
        </button>
      </div>
    {/if}
  </div>

  {#if confirmLeave}
    <div class="fixed inset-0 z-70 flex items-center justify-center p-4 bg-black/60">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl w-full max-w-sm" role="alertdialog" aria-modal="true" aria-labelledby="manual-leave-title">
        <h3 id="manual-leave-title" class="text-sm font-bold text-white">Buang penempatan?</h3>
        <p class="text-xs text-zinc-400 mt-1.5 tabular-nums">{placedCount} atlet sudah ditempatkan dan belum disimpan.</p>
        <div class="flex justify-end gap-2 mt-5">
          <button
            type="button"
            onclick={() => (confirmLeave = null)}
            use:focusOnMount
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            Lanjutkan
          </button>
          <button
            type="button"
            onclick={discardAndLeave}
            class="h-9 px-4 bg-red-500/15 text-red-300 border border-red-500/30 font-bold text-xs rounded-xl hover:bg-red-500/25 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            Buang
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
