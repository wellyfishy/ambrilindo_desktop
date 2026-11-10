<script lang="ts">
  // Admin Control: watch which buttons the 4 judges press on their sticks (mouse mode) for
  // one tatami, from any device. The kumite control panel scores the sticks and sends its
  // monitor as it computed it (stick_monitor), so this page shows exactly the same votes,
  // results and history, it never scores on its own. The shared socket reconnects by
  // itself; every (re)connect asks the control panel for its current state.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { getHostBaseUrl } from "$lib/api_client";
  import type { SideResult, StickHistoryRow, StickState } from "$lib/judgeSticks";
  import JudgeStickMonitor from "$lib/components/JudgeStickMonitor.svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));

  type LinkStatus = "connecting" | "live" | "offline";
  let linkStatus = $state<LinkStatus>("connecting");
  let socket: TatamiSocket | null = null;

  interface MonitorSnapshot {
    matchId: number | null;
    mouseMode: boolean;
    votes: StickState["votes"];
    aka: SideResult | null;
    ao: SideResult | null;
    hantei: boolean;
    hanteiTally: { aka: number; ao: number };
    timerRunning: boolean;
    checkMode: boolean;
    tested: string[];
    lastKey: string | null;
    history: StickHistoryRow[];
    at: number;
  }
  let monitor = $state<MonitorSnapshot | null>(null);

  // Match on the control panel (from its live state)
  let live = $state<{
    matchId: number | null;
    category: string;
    round: string;
    matchNumber: string;
    isKata: boolean;
    timerMs: number;
    isTimerRunning: boolean;
    aka: { name: string; contingent: string; score: number };
    ao: { name: string; contingent: string; score: number };
  } | null>(null);

  function applyLive(d: any) {
    if (!d || Number(d.tatami) !== tatamiNumber) return;
    live = {
      matchId: d.matchId ?? null,
      category: String(d.category || ""),
      round: String(d.round || ""),
      matchNumber: d.matchNumber != null ? String(d.matchNumber) : "",
      isKata: !!d.isKata,
      timerMs: Number(d.timerMs) || 0,
      isTimerRunning: !!d.isTimerRunning,
      aka: { name: d.aka?.name || "AKA", contingent: d.aka?.contingent || "-", score: Number(d.aka?.score) || 0 },
      ao: { name: d.ao?.name || "AO", contingent: d.ao?.contingent || "-", score: Number(d.ao?.score) || 0 },
    };
  }

  function applyMonitor(msg: any) {
    if (Number(msg?.tatami) !== tatamiNumber || !msg.payload) return;
    monitor = msg.payload as MonitorSnapshot;
  }

  function clock(ms: number): string {
    const total = Math.max(0, Math.ceil(ms / 1000));
    return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
  }

  async function fetchLiveState() {
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/tatami/${tatamiNumber}/live-state?_t=${Date.now()}`, { cache: "no-store" });
      if (res.ok) {
        const json = await res.json();
        if (json?.data) applyLive(json.data);
      }
    } catch {}
  }

  onMount(() => {
    fetchLiveState();
    socket = connectTatamiSocket(
      "admin-view",
      tatamiNumber,
      (msg) => {
        linkStatus = "live";
        if (msg?.type === "stick_monitor") applyMonitor(msg);
        else if (msg?.type === "tatami_live_state") applyLive(msg.payload || msg.data);
      },
      (sock) => {
        linkStatus = "live";
        // Every (re)connect: latest match and monitor from the control panel
        sock.send({ type: "request_tatami_state", tatami: tatamiNumber });
        fetchLiveState();
      },
      () => (linkStatus = typeof navigator !== "undefined" && navigator.onLine === false ? "offline" : "connecting"),
      { staleMs: 15000, onStale: (sock) => sock.send({ type: "request_tatami_state", tatami: tatamiNumber }) }
    );
    return () => socket?.close();
  });
</script>

<svelte:head>
  <title>Admin Control • Tatami {tatamiNumber}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col select-none font-sans overflow-hidden">
  <!-- Header -->
  <header class="bg-[#14161b] border-b border-white/10 px-4 py-2.5 flex items-center justify-between gap-3 shrink-0">
    <div class="flex items-center gap-2 min-w-0">
      <PortalExitButton title="Keluar dari Admin Control?" />
      <span class="px-2.5 py-1 rounded-lg bg-purple-500/20 text-purple-300 font-mono font-black text-xs tracking-wider border border-purple-500/30 shrink-0">
        ADMIN CONTROL • TATAMI {tatamiNumber}
      </span>
      <span
        class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono shrink-0 border {linkStatus === 'live'
          ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
          : linkStatus === 'connecting'
            ? 'bg-amber-500/10 text-amber-300 border-amber-500/30'
            : 'bg-red-500/10 text-red-400 border-red-500/20'}"
      >
        <span class="w-1.5 h-1.5 rounded-full {linkStatus === 'live' ? 'bg-emerald-400 animate-pulse' : linkStatus === 'connecting' ? 'bg-amber-300 animate-pulse' : 'bg-red-400'}"></span>
        {linkStatus === "live" ? "ONLINE" : linkStatus === "connecting" ? "MENYAMBUNG ULANG…" : "OFFLINE"}
      </span>
    </div>
    {#if live}
      <div class="text-right min-w-0 hidden sm:block">
        <div class="text-xs font-black text-white uppercase truncate">{live.category || "-"}</div>
        <div class="text-[11px] font-mono text-zinc-400 truncate">{live.round}{live.matchNumber ? ` • Partai ${live.matchNumber}` : ""}</div>
      </div>
    {/if}
  </header>

  <main class="flex-1 min-h-0 w-full max-w-5xl mx-auto p-3 sm:p-4 flex flex-col gap-3">
    <!-- Bout on the control panel: AO left, AKA right (operator's view) -->
    {#if live}
      <div class="grid grid-cols-[1fr_auto_1fr] items-stretch gap-2 shrink-0">
        <div class="rounded-xl border border-blue-500/40 bg-blue-950/30 px-3 py-2 flex items-center justify-between gap-2 min-w-0">
          <div class="min-w-0">
            <div class="text-[10px] font-mono font-black text-blue-300">AO</div>
            <div class="text-sm font-black text-white uppercase truncate">{live.ao.name}</div>
            <div class="text-[11px] text-zinc-400 truncate">{live.ao.contingent}</div>
          </div>
          <div class="text-3xl font-mono font-black text-blue-300 tabular-nums">{live.isKata ? "-" : live.ao.score}</div>
        </div>
        <div class="rounded-xl border px-3 py-2 flex flex-col items-center justify-center font-mono {live.isTimerRunning ? 'border-amber-500/60 bg-amber-500/10' : 'border-white/10 bg-[#13151b]'}">
          <div class="text-2xl font-black tabular-nums {live.isTimerRunning ? 'text-amber-300' : 'text-white'}">{clock(live.timerMs)}</div>
          <div class="text-[10px] font-bold {live.isTimerRunning ? 'text-amber-300' : 'text-zinc-500'}">{live.isTimerRunning ? "BERJALAN" : "YAME"}</div>
        </div>
        <div class="rounded-xl border border-red-500/40 bg-red-950/30 px-3 py-2 flex items-center justify-between gap-2 min-w-0">
          <div class="text-3xl font-mono font-black text-red-300 tabular-nums">{live.isKata ? "-" : live.aka.score}</div>
          <div class="min-w-0 text-right">
            <div class="text-[10px] font-mono font-black text-red-300">AKA</div>
            <div class="text-sm font-black text-white uppercase truncate">{live.aka.name}</div>
            <div class="text-[11px] text-zinc-400 truncate">{live.aka.contingent}</div>
          </div>
        </div>
      </div>
    {/if}

    <!-- Judge sticks, exactly as the control panel computed them -->
    {#if live?.isKata}
      <div class="flex-1 flex items-center justify-center text-sm font-mono text-zinc-500 text-center">Partai kata: monitor stick juri hanya untuk kumite</div>
    {:else if !monitor}
      <div class="flex-1 flex items-center justify-center text-sm font-mono text-zinc-500 text-center">
        {linkStatus === "live" ? "Menunggu control panel tatami ini…" : "Menyambungkan ke meja utama…"}
      </div>
    {:else}
      {#if !monitor.mouseMode}
        <div class="px-3 py-2 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs font-semibold shrink-0">
          Mode Mouse Juri belum aktif di control panel tatami ini
        </div>
      {/if}
      <div class="admin-monitor flex-1 min-h-0 flex flex-col">
        <JudgeStickMonitor
          votes={monitor.votes}
          aka={monitor.aka}
          ao={monitor.ao}
          hantei={monitor.hantei}
          hanteiTally={monitor.hanteiTally}
          timerRunning={monitor.timerRunning}
          checkMode={monitor.checkMode}
          tested={monitor.tested}
          lastKey={monitor.lastKey}
          history={monitor.history}
          readOnly
          historyAlwaysOpen
        />
      </div>
    {/if}
  </main>
</div>

<style>
  /* Bigger than in the control panel: this screen is only the monitor */
  .admin-monitor :global(> div) {
    flex: 1;
    min-height: 0;
  }
  /* Laptop / TV width: bigger text (zoom only where the width allows it) */
  @media (min-width: 1100px) {
    .admin-monitor {
      zoom: 1.3;
    }
  }
</style>
