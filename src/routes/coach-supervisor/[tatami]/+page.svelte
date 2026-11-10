<script lang="ts">
  // Coach supervisor tablet (as in Ambrilindo Local): one tap on 1 / 2 / 3 asks
  // for a Video Review of that value for a corner. The VR referee decides on the
  // VR tablet; everything shown here comes from the control panel's live state,
  // so it follows match changes and reconnects at once.
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";
  import {
    connectVrLink,
    emptyView,
    mergeLiveState,
    sendVrDecision,
    sendVrRequest,
    formatClock,
    POINT_NAME,
    type LinkStatus,
    type VrLink,
    type VrSide,
    type VrView,
  } from "$lib/vrLink";

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));

  let view = $state<VrView>(emptyView(1));
  let status = $state<LinkStatus>("connecting");
  let link: VrLink | null = null;

  // Which corners this tablet shows (each coach may hold their own tablet)
  type Focus = "both" | "aka" | "ao";
  let focus = $state<Focus>("both");

  // A tap waiting for the control panel to confirm it
  let pending = $state<{ aka: { points: number; at: number } | null; ao: { points: number; at: number } | null }>({ aka: null, ao: null });
  let now = $state(Date.now());
  let ticker: ReturnType<typeof setInterval> | null = null;

  function applyState(data: any) {
    const prevMatch = view.matchId;
    view = mergeLiveState(view, data);
    // New match: forget anything still pending from the old one
    if (prevMatch !== null && view.matchId !== prevMatch) pending = { aka: null, ao: null };
    // Confirmed (or decided) by the control panel: stop showing our own pending tap
    for (const side of ["aka", "ao"] as const) {
      if (pending[side] && view.requests[side]) pending = { ...pending, [side]: null };
    }
  }

  function ask(side: VrSide, points: number) {
    if (!canAsk(side)) return;
    pending = { ...pending, [side]: { points, at: Date.now() } };
    sendVrRequest(tatamiNumber, side, points);
    navigator.vibrate?.([80, 40, 80]);
  }

  function cancel(side: VrSide) {
    pending = { ...pending, [side]: null };
    sendVrDecision(tatamiNumber, side, "cancelled");
    navigator.vibrate?.(60);
  }

  function canAsk(side: VrSide) {
    return view.hasData && view.vrEnabled && !view[side].cardUsed && !view.requests[side] && !pending[side];
  }

  type CardState = "novr" | "used" | "reviewing" | "accepted" | "rejected" | "minai" | "sending" | "ready" | "nodata";
  function cardState(side: VrSide): CardState {
    if (!view.hasData) return "nodata";
    if (!view.vrEnabled) return "novr";
    const req = view.requests[side];
    if (req) return req.status === "in_progress" ? "reviewing" : req.status;
    if (pending[side]) return "sending";
    return view[side].cardUsed ? "used" : "ready";
  }

  function askedPoints(side: VrSide): number | undefined {
    return view.requests[side]?.points ?? pending[side]?.points;
  }

  onMount(() => {
    try {
      const saved = localStorage.getItem("ambrilindo.coachFocus") as Focus | null;
      if (saved === "aka" || saved === "ao" || saved === "both") focus = saved;
    } catch {}
    view = emptyView(tatamiNumber);
    link = connectVrLink("coach-supervisor", tatamiNumber, applyState, (s) => (status = s));
    ticker = setInterval(() => (now = Date.now()), 500);
  });

  onDestroy(() => {
    link?.close();
    if (ticker) clearInterval(ticker);
  });

  function setFocus(f: Focus) {
    focus = f;
    try {
      localStorage.setItem("ambrilindo.coachFocus", f);
    } catch {}
  }

  function toggleFullscreen() {
    if (!document.fullscreenElement) document.documentElement.requestFullscreen?.().catch(() => {});
    else document.exitFullscreen?.().catch(() => {});
  }

  const SIDES: { side: VrSide; label: string }[] = [
    { side: "aka", label: "AKA (MERAH)" },
    { side: "ao", label: "AO (BIRU)" },
  ];
</script>

<div class="min-h-screen bg-[#0e1014] text-zinc-100 flex flex-col font-sans select-none">
  <!-- Header -->
  <header class="bg-[#14161c] border-b border-white/10 px-4 py-3 flex items-center justify-between gap-3 sticky top-0 z-30">
    <div class="flex items-center gap-3 min-w-0">
      <PortalExitButton title="Keluar dari Coach Supervisor?" />
      <div class="px-2.5 py-1 rounded-lg bg-amber-500 text-black font-black text-sm shrink-0">T{tatamiNumber}</div>
      <div class="min-w-0">
        <div class="text-[11px] font-bold uppercase tracking-wider text-zinc-400">Video Review · Pelatih</div>
        <div class="text-xs font-black text-white uppercase truncate">
          {view.category || "Menunggu data partai…"}{view.round ? ` · ${view.round}` : ""}{view.matchNumber ? ` · Partai ${view.matchNumber}` : ""}
        </div>
      </div>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      <div class="hidden sm:flex items-center bg-black/30 border border-white/10 rounded-lg p-0.5 text-[11px] font-bold">
        {#each [["both", "AKA+AO"], ["aka", "AKA"], ["ao", "AO"]] as [f, label]}
          <button
            type="button"
            onclick={() => setFocus(f as Focus)}
            class="px-2.5 py-1 rounded-md transition {focus === f ? 'bg-white text-black' : 'text-zinc-400 hover:text-white'}"
          >{label}</button>
        {/each}
      </div>
      <span class="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-mono font-bold border
        {status === 'live' ? 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30' : status === 'offline' ? 'bg-rose-500/15 text-rose-400 border-rose-500/30' : 'bg-amber-500/15 text-amber-300 border-amber-500/30'}">
        <span class="w-2 h-2 rounded-full {status === 'live' ? 'bg-emerald-400' : status === 'offline' ? 'bg-rose-500' : 'bg-amber-400 animate-pulse'}"></span>
        {status === "live" ? "ONLINE" : status === "offline" ? "OFFLINE" : "MENYAMBUNG ULANG…"}
      </span>
      <button type="button" onclick={toggleFullscreen} class="w-8 h-8 rounded-lg border border-white/10 text-zinc-400 hover:text-white flex items-center justify-center" aria-label="Layar penuh">
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4h4M20 8V4h-4M4 16v4h4M20 16v4h-4"/></svg>
      </button>
    </div>
  </header>

  <!-- Focus switch on small screens -->
  <div class="sm:hidden flex items-center justify-center gap-1 p-2 text-[11px] font-bold">
    {#each [["both", "AKA+AO"], ["aka", "AKA"], ["ao", "AO"]] as [f, label]}
      <button type="button" onclick={() => setFocus(f as Focus)} class="px-3 py-1 rounded-md border {focus === f ? 'bg-white text-black border-white' : 'border-white/10 text-zinc-400'}">{label}</button>
    {/each}
  </div>

  <main class="flex-1 p-3 sm:p-4 flex flex-col gap-3">
    {#if view.hasData && !view.vrEnabled}
      <div class="rounded-xl border border-amber-500/40 bg-amber-500/10 text-amber-300 text-sm font-semibold px-4 py-2 text-center">
        Kategori pertandingan ini tidak menggunakan Video Review (VR).
      </div>
    {/if}

    <div class="flex-1 grid gap-3 {focus === 'both' ? 'grid-cols-1 md:grid-cols-2' : 'grid-cols-1'}">
      {#each SIDES as { side, label } (side)}
        {#if focus === "both" || focus === side}
          {@const st = cardState(side)}
          {@const asked = askedPoints(side)}
          {@const corner = view[side]}
          <section class="rounded-2xl border-2 bg-[#14161c] flex flex-col overflow-hidden {side === 'aka' ? 'border-red-600/60' : 'border-blue-600/60'}">
            <!-- Corner header -->
            <div class="flex items-center justify-between gap-2 px-4 py-3 {side === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">
              <span class="font-black tracking-wide">{label}</span>
              <span class="px-2.5 py-1 rounded-full text-[11px] font-black font-mono
                {st === 'reviewing' || st === 'sending' ? 'bg-amber-400 text-black animate-pulse'
                : st === 'used' || st === 'rejected' ? 'bg-black/40 text-rose-200'
                : st === 'novr' || st === 'nodata' ? 'bg-black/40 text-zinc-300'
                : 'bg-white text-black'}">
                {st === "reviewing" || st === "sending" ? "REVIEW BERJALAN" : st === "used" || st === "rejected" ? "KARTU HANGUS" : st === "novr" ? "TANPA VR" : st === "nodata" ? "—" : "KARTU AKTIF"}
              </span>
            </div>

            <!-- Athlete -->
            <div class="px-4 pt-3 pb-2">
              <div class="text-2xl font-black uppercase leading-tight truncate">{corner.name}</div>
              <div class="text-sm text-zinc-400 truncate">{corner.club}</div>
            </div>

            <!-- One-tap value buttons -->
            <div class="flex-1 grid grid-cols-3 gap-2 px-3 pb-3 min-h-[9rem]">
              {#each [1, 2, 3] as pts}
                {@const isAsked = asked === pts && (st === "reviewing" || st === "sending")}
                <button
                  type="button"
                  onclick={() => ask(side, pts)}
                  disabled={!canAsk(side)}
                  class="rounded-xl border-2 flex flex-col items-center justify-center gap-1 transition active:scale-95
                    {isAsked
                      ? 'bg-amber-400 border-amber-200 text-black shadow-lg shadow-amber-500/30'
                      : canAsk(side)
                        ? (side === 'aka' ? 'bg-red-600/20 border-red-500 text-white hover:bg-red-600/40' : 'bg-blue-600/20 border-blue-500 text-white hover:bg-blue-600/40')
                        : 'bg-white/[0.03] border-white/10 text-zinc-600 cursor-not-allowed'}"
                >
                  <span class="text-5xl font-black leading-none">{pts}</span>
                  <span class="text-xs font-black tracking-wider">{POINT_NAME[pts]}</span>
                </button>
              {/each}
            </div>

            <!-- Status bar -->
            <div class="border-t border-white/10 px-4 py-3 flex items-center justify-between gap-3 bg-black/20">
              <div class="min-w-0">
                <div class="text-[10px] font-bold text-zinc-500 uppercase">Status review</div>
                <div class="text-sm font-bold truncate
                  {st === 'accepted' ? 'text-emerald-400' : st === 'rejected' || st === 'used' ? 'text-rose-400' : st === 'minai' ? 'text-amber-300' : st === 'reviewing' || st === 'sending' ? 'text-amber-300' : 'text-zinc-300'}">
                  {#if st === "nodata"}
                    Menunggu data dari control panel…
                  {:else if st === "novr"}
                    Kategori ini tanpa Video Review
                  {:else if st === "sending"}
                    {#if pending[side] && now - pending[side]!.at > 4000}
                      Belum diterima control panel, periksa koneksi…
                    {:else}
                      Mengirim pengajuan {asked ? POINT_NAME[asked] : ""}…
                    {/if}
                  {:else if st === "reviewing"}
                    Sedang ditinjau wasit VR{asked ? `: ${asked} poin (${POINT_NAME[asked]})` : ""}
                  {:else if st === "accepted"}
                    ✓ Diterima{view.requests[side]?.points ? ` +${view.requests[side]?.points} poin` : ""} (kartu tetap aktif)
                  {:else if st === "rejected"}
                    ✕ Ditolak (kartu hangus)
                  {:else if st === "minai"}
                    Minai: tidak terlihat (kartu tetap aktif)
                  {:else if st === "used"}
                    Kartu VR sudah hangus
                  {:else}
                    Siap diajukan
                  {/if}
                </div>
              </div>
              {#if st === "reviewing" || st === "sending"}
                <button
                  type="button"
                  onclick={() => cancel(side)}
                  class="shrink-0 h-10 px-4 rounded-lg bg-amber-500 hover:bg-amber-400 text-black text-xs font-black transition active:scale-95"
                >
                  BATAL / REVISI
                </button>
              {/if}
            </div>
          </section>
        {/if}
      {/each}
    </div>

    <div class="text-center text-[11px] font-mono text-zinc-500">
      Skor AKA {view.aka.score} – {view.ao.score} AO · {formatClock(view.timerMs)}{view.isTimerRunning ? " (berjalan)" : ""}
    </div>
  </main>
</div>
