<script lang="ts">
  // VR referee tablet (as in Ambrilindo Local): shows each corner's open request
  // from the coach and decides it: ACCEPT (+the value asked for), DECLINE (card
  // lost) or MINAI (inconclusive, card kept). Both corners can be under review at
  // once. Everything comes from the control panel's live state, so match changes
  // and reconnects show the latest data straight away.
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import {
    connectVrLink,
    emptyView,
    mergeLiveState,
    sendVrDecision,
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
  let now = $state(Date.now());
  let ticker: ReturnType<typeof setInterval> | null = null;

  // When this tablet first saw each open request (for the review stopwatch;
  // independent of the operator laptop's clock)
  const seenAt: Record<VrSide, { ts: number; at: number } | null> = { aka: null, ao: null };
  let seenTick = $state(0);

  // Decision sent, waiting for the control panel to confirm it
  let sending = $state<{ aka: { decision: string; at: number } | null; ao: { decision: string; at: number } | null }>({ aka: null, ao: null });

  // Review log for this match
  interface LogRow {
    key: string;
    time: string;
    side: VrSide;
    asked?: number;
    decision: "accepted" | "rejected" | "minai";
    points?: number;
  }
  let log = $state<LogRow[]>([]);

  // Value the coach asked for, remembered for the log after the decision
  const askedFor: Record<VrSide, number | undefined> = { aka: undefined, ao: undefined };

  function applyState(data: any) {
    const prevMatch = view.matchId;
    view = mergeLiveState(view, data);
    if (prevMatch !== null && view.matchId !== prevMatch) {
      // New match on this tatami: start clean
      log = [];
      sending = { aka: null, ao: null };
      seenAt.aka = null;
      seenAt.ao = null;
    }
    for (const side of ["aka", "ao"] as const) {
      const req = view.requests[side];
      if (req && req.status === "in_progress") {
        if (!seenAt[side] || seenAt[side]!.ts !== req.timestamp) {
          seenAt[side] = { ts: req.timestamp, at: Date.now() };
          seenTick++;
        }
      }
      if (req && req.status !== "in_progress") {
        const key = `${side}-${req.timestamp}-${req.status}`;
        if (!log.some((r) => r.key === key)) {
          const t = new Date();
          log = [
            {
              key,
              time: `${String(t.getHours()).padStart(2, "0")}:${String(t.getMinutes()).padStart(2, "0")}:${String(t.getSeconds()).padStart(2, "0")}`,
              side,
              asked: askedFor[side],
              decision: req.status,
              points: req.points,
            },
            ...log,
          ].slice(0, 20);
        }
      }
      if (req?.status === "in_progress") askedFor[side] = req.points;
      // Our decision has landed (or the request is gone): stop "sending"
      if (sending[side] && (!req || req.status !== "in_progress")) sending = { ...sending, [side]: null };
    }
  }

  function decide(side: VrSide, decision: "accepted" | "rejected" | "minai") {
    const req = view.requests[side];
    if (!req || req.status !== "in_progress" || sending[side]) return;
    sending = { ...sending, [side]: { decision, at: Date.now() } };
    sendVrDecision(tatamiNumber, side, decision, decision === "accepted" ? req.points : undefined);
    navigator.vibrate?.(decision === "accepted" ? [100, 50, 100] : [200]);
  }

  function elapsed(side: VrSide): string {
    void seenTick;
    const s = seenAt[side];
    if (!s) return "00:00";
    return formatClock(now - s.at);
  }

  onMount(() => {
    view = emptyView(tatamiNumber);
    link = connectVrLink("wasit-vr", tatamiNumber, applyState, (s) => (status = s));
    ticker = setInterval(() => (now = Date.now()), 500);
  });

  onDestroy(() => {
    link?.close();
    if (ticker) clearInterval(ticker);
  });

  function toggleFullscreen() {
    if (!document.fullscreenElement) document.documentElement.requestFullscreen?.().catch(() => {});
    else document.exitFullscreen?.().catch(() => {});
  }

  let bothOpen = $derived(view.requests.aka?.status === "in_progress" && view.requests.ao?.status === "in_progress");

  // Decision table side: AO on the left, AKA on the right (as in Ambrilindo Local)
  const SIDES: { side: VrSide; label: string }[] = [
    { side: "ao", label: "AO (BIRU)" },
    { side: "aka", label: "AKA (MERAH)" },
  ];
</script>

<div class="min-h-screen bg-[#0e1014] text-zinc-100 flex flex-col font-sans select-none">
  <!-- Header -->
  <header class="bg-[#14161c] border-b border-white/10 px-4 py-3 flex items-center justify-between gap-3 sticky top-0 z-30">
    <div class="flex items-center gap-3 min-w-0">
      <div class="px-2.5 py-1 rounded-lg bg-amber-500 text-black font-black text-sm shrink-0">T{tatamiNumber}</div>
      <div class="min-w-0">
        <div class="text-[11px] font-bold uppercase tracking-wider text-zinc-400">Wasit Video Review</div>
        <div class="text-xs font-black text-white uppercase truncate">
          {view.category || "Menunggu data partai…"}{view.round ? ` · ${view.round}` : ""}{view.matchNumber ? ` · Partai ${view.matchNumber}` : ""}
        </div>
      </div>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      <span class="hidden sm:inline px-2.5 py-1 rounded-lg bg-black/40 border border-white/10 font-mono text-xs font-bold">
        {formatClock(view.timerMs)}{view.isTimerRunning ? " ▶" : ""}
      </span>
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

  <main class="flex-1 p-3 sm:p-4 flex flex-col gap-3">
    {#if view.hasData && !view.vrEnabled}
      <div class="rounded-xl border border-amber-500/40 bg-amber-500/10 text-amber-300 text-sm font-semibold px-4 py-2 text-center">
        Kategori pertandingan ini tidak menggunakan Video Review (VR).
      </div>
    {/if}
    {#if bothOpen}
      <div class="rounded-xl border border-amber-400 bg-amber-500/15 text-amber-200 text-sm font-bold px-4 py-2 text-center animate-pulse">
        🚨 DUA SUDUT MENGAJUKAN REVIEW SEKALIGUS: tinjau rekaman dan putuskan untuk AO dan AKA.
      </div>
    {/if}

    <div class="flex-1 grid grid-cols-1 md:grid-cols-2 gap-3">
      {#each SIDES as { side, label } (side)}
        {@const req = view.requests[side]}
        {@const corner = view[side]}
        {@const send = sending[side]}
        <section class="rounded-2xl border-2 bg-[#14161c] flex flex-col overflow-hidden {side === 'aka' ? 'border-red-600/60' : 'border-blue-600/60'}">
          <!-- Corner header -->
          <div class="flex items-center justify-between gap-2 px-4 py-3 {side === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">
            <span class="font-black tracking-wide">{label}</span>
            <span class="px-2.5 py-1 rounded-full text-[11px] font-black font-mono {corner.cardUsed ? 'bg-black/40 text-rose-200' : 'bg-white text-black'}">
              {corner.cardUsed ? "KARTU HANGUS" : "KARTU AKTIF"}
            </span>
          </div>
          <div class="px-4 pt-3 pb-2 flex items-end justify-between gap-3">
            <div class="min-w-0">
              <div class="text-2xl font-black uppercase leading-tight truncate">{corner.name}</div>
              <div class="text-sm text-zinc-400 truncate">{corner.club}</div>
            </div>
            <div class="text-4xl font-black font-mono shrink-0">{corner.score}</div>
          </div>

          <div class="flex-1 px-3 pb-3 flex flex-col gap-2 min-h-[16rem]">
            {#if !req}
              <!-- Standby -->
              <div class="flex-1 rounded-xl border border-dashed border-white/15 flex flex-col items-center justify-center gap-2 text-zinc-500 text-center px-4">
                <svg class="w-10 h-10" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.6" d="M15 10l4.55-2.28A1 1 0 0121 8.62v6.76a1 1 0 01-1.45.9L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/></svg>
                {#if corner.cardUsed}
                  <div class="text-sm font-semibold text-rose-300">Kartu VR {side === "aka" ? "AKA" : "AO"} sudah hangus</div>
                {:else}
                  <div class="text-sm font-semibold">Menunggu pengajuan Video Review dari pelatih {side === "aka" ? "AKA" : "AO"}</div>
                {/if}
              </div>
            {:else if req.status === "in_progress"}
              <!-- Open request -->
              <div class="rounded-xl bg-amber-400 text-black px-4 py-3 flex items-center justify-between gap-3">
                <div>
                  <div class="text-[11px] font-black uppercase">Pengajuan pelatih</div>
                  <div class="text-2xl font-black leading-tight">
                    {req.points ? `${POINT_NAME[req.points]} (${req.points} POINT)` : "VIDEO REVIEW"}
                  </div>
                </div>
                <div class="text-right">
                  <div class="text-[10px] font-bold uppercase">Durasi tinjau</div>
                  <div class="text-xl font-black font-mono">{elapsed(side)}</div>
                </div>
              </div>
              <button
                type="button"
                onclick={() => decide(side, "accepted")}
                disabled={!!send}
                class="flex-1 min-h-[4.5rem] rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-black text-xl transition active:scale-[0.98]"
              >
                ✓ ACCEPT{req.points ? ` (+${req.points} ${POINT_NAME[req.points]})` : ""}
              </button>
              <div class="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onclick={() => decide(side, "rejected")}
                  disabled={!!send}
                  class="min-h-[3.75rem] rounded-xl bg-rose-600 hover:bg-rose-500 disabled:opacity-50 text-white font-black text-base transition active:scale-[0.98]"
                >
                  ✕ DECLINE (DITOLAK)
                </button>
                <button
                  type="button"
                  onclick={() => decide(side, "minai")}
                  disabled={!!send}
                  class="min-h-[3.75rem] rounded-xl bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-black font-black text-base transition active:scale-[0.98]"
                >
                  MINAI (TIDAK TERLIHAT)
                </button>
              </div>
              {#if send}
                <div class="text-center text-xs font-bold {now - send.at > 4000 ? 'text-rose-300' : 'text-zinc-400'}">
                  {now - send.at > 4000 ? "Belum dikonfirmasi control panel, periksa koneksi…" : "Mengirim keputusan…"}
                </div>
              {/if}
            {:else}
              <!-- Decision locked in (shown for a few seconds) -->
              <div class="flex-1 rounded-xl border-2 flex flex-col items-center justify-center gap-1 text-center px-4
                {req.status === 'accepted' ? 'border-emerald-500 bg-emerald-500/15 text-emerald-200' : req.status === 'rejected' ? 'border-rose-500 bg-rose-500/15 text-rose-200' : 'border-amber-500 bg-amber-500/15 text-amber-200'}">
                <div class="text-xs font-bold uppercase">Keputusan {side === "aka" ? "AKA" : "AO"}</div>
                <div class="text-3xl font-black">
                  {req.status === "accepted" ? `✓ ACCEPT${req.points ? ` +${req.points}` : ""}` : req.status === "rejected" ? "✕ DECLINE" : "MINAI"}
                </div>
                <div class="text-sm font-semibold">
                  {req.status === "rejected" ? "Kartu VR hangus" : "Kartu VR tetap aktif"}
                </div>
              </div>
            {/if}
          </div>
        </section>
      {/each}
    </div>

    <!-- Review log for this match -->
    <section class="rounded-2xl border border-white/10 bg-[#14161c] overflow-hidden">
      <div class="px-4 py-2 text-[11px] font-bold uppercase tracking-wider text-zinc-400 border-b border-white/10">Log Video Review partai ini</div>
      {#if log.length === 0}
        <div class="px-4 py-3 text-sm text-zinc-500 text-center">Belum ada riwayat Video Review pada partai ini.</div>
      {:else}
        <table class="w-full text-sm">
          <tbody>
            {#each log as row (row.key)}
              <tr class="border-t border-white/5">
                <td class="px-4 py-2 font-mono text-zinc-400">{row.time}</td>
                <td class="px-2 py-2 font-black {row.side === 'aka' ? 'text-red-400' : 'text-blue-400'}">{row.side === "aka" ? "AKA" : "AO"}</td>
                <td class="px-2 py-2 text-zinc-300">{row.asked ? `${POINT_NAME[row.asked]} (${row.asked})` : "-"}</td>
                <td class="px-4 py-2 text-right font-black {row.decision === 'accepted' ? 'text-emerald-400' : row.decision === 'rejected' ? 'text-rose-400' : 'text-amber-300'}">
                  {row.decision === "accepted" ? `ACCEPT${row.points ? ` +${row.points}` : ""}` : row.decision === "rejected" ? "DECLINE" : "MINAI"}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  </main>
</div>
