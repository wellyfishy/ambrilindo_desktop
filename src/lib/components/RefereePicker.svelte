<script lang="ts">
  // Choose a referee for one seat: photo cards, search, ranked like Ambrilindo Local
  // (this tatami's team, standby, borrow from another tatami, then conflicts ⚠).
  // Typing a name and pressing Enter picks the closest name (typos allowed).
  import type { BoutPanel, WasitItem } from "$lib/types";
  import { rankCandidates, searchScore } from "$lib/refereeSeats";
  import RefereeAvatar from "./RefereeAvatar.svelte";

  interface Props {
    title: string;
    wasits: WasitItem[];
    tatamiId: number | null;
    /** bout context for conflict checks (null for the tatami team) */
    panel?: BoutPanel | null;
    /** referees already seated in this panel (shown, marked) */
    takenIds?: Set<number>;
    currentId?: number | null;
    onPick: (w: WasitItem) => void;
    onClear?: () => void;
    onClose: () => void;
  }
  let { title, wasits, tatamiId, panel = null, takenIds = new Set(), currentId = null, onPick, onClear, onClose }: Props = $props();

  let query = $state("");
  let candidates = $derived(rankCandidates(wasits, { tatamiId, panel, takenIds }));
  /** below this, a typed name matches nobody */
  const MIN_NAME_SCORE = 0.55;
  let shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return candidates;
    // closest names first (typos allowed), then matches on perguruan / kota / lisensi
    const scored = candidates.map((c) => ({ c, s: searchScore(q, c.wasit.nama_wasit) }));
    const byName = scored.filter((x) => x.s >= MIN_NAME_SCORE).sort((a, b) => b.s - a.s || a.c.rank - b.c.rank);
    const others = candidates.filter(
      (c) =>
        !byName.some((x) => x.c === c) &&
        [c.wasit.nama_perguruan, c.wasit.kab_kota, c.wasit.lisensi, c.wasit.keterangan].some((v) => (v || "").toLowerCase().includes(q))
    );
    return [...byName.map((x) => x.c), ...others];
  });
  /** what Enter picks */
  let best = $derived(query.trim() ? shown[0] ?? null : null);

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (best) onPick(best.wasit);
    } else if (e.key === "Escape") {
      onClose();
    }
  }

  let searchEl: HTMLInputElement | null = $state(null);
  $effect(() => {
    searchEl?.focus();
  });
</script>

<div
  class="fixed inset-0 z-[80] flex items-end sm:items-center justify-center p-0 sm:p-4 bg-black/70 backdrop-blur-xs"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose();
  }}
>
  <div class="bg-[#181a20] border border-white/10 rounded-t-2xl sm:rounded-2xl shadow-2xl w-full sm:max-w-3xl max-h-[88vh] flex flex-col">
    <div class="px-4 pt-4 pb-3 border-b border-white/[0.08] flex items-center gap-3">
      <h3 class="text-sm font-black uppercase tracking-wider text-white font-mono flex-1 truncate">{title}</h3>
      {#if onClear && currentId}
        <button type="button" onclick={onClear} class="h-9 px-3.5 bg-white/[0.04] text-red-300 border border-white/[0.1] font-semibold text-xs rounded-xl">
          Kosongkan
        </button>
      {/if}
      <button type="button" onclick={onClose} class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08]" aria-label="Tutup">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
      </button>
    </div>
    <div class="px-4 py-3">
      <input
        bind:this={searchEl}
        bind:value={query}
        onkeydown={onKeydown}
        type="search"
        enterkeyhint="done"
        autocomplete="off"
        autocapitalize="characters"
        placeholder="Ketik nama lalu Enter…"
        class="w-full bg-[#111215] text-base text-white px-3.5 py-3 rounded-xl border border-white/[0.1] focus:border-white outline-none"
      />
      {#if query.trim()}
        <div class="mt-1.5 text-[11px] font-mono {best ? (best.conflict ? 'text-amber-300' : 'text-emerald-300') : 'text-zinc-500'}">
          {#if best}
            Enter ↵ {best.wasit.nama_wasit}{best.conflict ? ` · ⚠ ${best.conflict}` : ""}
          {:else}
            Tidak ada nama yang mirip
          {/if}
        </div>
      {/if}
    </div>
    <div class="flex-1 overflow-y-auto px-4 pb-4 grid grid-cols-1 sm:grid-cols-2 gap-2">
      {#each shown as c (c.wasit.id)}
        {@const isCurrent = c.wasit.id === currentId}
        <button
          type="button"
          onclick={() => onPick(c.wasit)}
          class="text-left flex items-center gap-3 p-2.5 rounded-xl border transition {c === best
            ? 'bg-emerald-500/[0.08] border-emerald-400/70 ring-1 ring-emerald-400/40'
            : isCurrent
            ? 'bg-white/[0.08] border-white/40'
            : c.conflict
              ? 'bg-amber-500/[0.04] border-amber-500/25 hover:border-amber-400/60'
              : 'bg-[#111215] border-white/[0.08] hover:border-white/30'}"
        >
          <RefereeAvatar foto={c.wasit.foto} nama={c.wasit.nama_wasit} size={52} />
          <div class="min-w-0 flex-1">
            <div class="text-sm font-bold text-white truncate">{c.wasit.nama_wasit}</div>
            <div class="text-[11px] text-zinc-400 truncate">
              {c.wasit.nama_perguruan || "PB / Umum"}{c.wasit.kab_kota ? ` • ${c.wasit.kab_kota}` : ""}
            </div>
            <div class="mt-1 flex flex-wrap gap-1 text-[10px] font-mono font-bold">
              <span class="px-1.5 py-0.5 rounded {c.rank % 3 === 0 && !c.conflict ? 'bg-emerald-500/15 text-emerald-300' : 'bg-white/[0.06] text-zinc-300'}">
                {c.label}
              </span>
              {#if c.wasit.keterangan}
                <span class="px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300">{c.wasit.keterangan}</span>
              {/if}
              {#if c.conflict}
                <span class="px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300" title={c.conflict}>⚠ {c.conflict}</span>
              {/if}
            </div>
          </div>
        </button>
      {:else}
        <div class="col-span-full py-10 text-center text-xs text-zinc-500">Tidak ada wasit yang cocok.</div>
      {/each}
    </div>
  </div>
</div>
