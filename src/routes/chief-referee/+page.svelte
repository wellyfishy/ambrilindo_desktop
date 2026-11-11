<script lang="ts">
  // Chief Referee: bouts whose operator wants to start, from every tatami, with the officials
  // the Tatami Manager submitted. Terima → the control panel opens; Tolak (+ reason) → the
  // bout is a draft again and the Tatami Manager is told.
  // Realtime over the shared socket, plus an HTTP refresh every 4 s (tablets on picky Wi-Fi).
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { apiCall, getHostBaseUrl } from "$lib/api_client";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import type { ApprovalQueue, BoutPanel, EventItem } from "$lib/types";
  import { boutJudgeCount, boutSeats, conflictReason, effectivePanel } from "$lib/refereeSeats";
  import RefereeSeatCard from "$lib/components/RefereeSeatCard.svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";

  let eventId = $state<number | null>(Number(page.url.searchParams.get("event")) || null);
  let eventName = $state("");
  let queue = $state<ApprovalQueue | null>(null);
  let panels = $state<Record<number, BoutPanel>>({});
  let loadError = $state("");
  let linkLive = $state(false);
  let busyId = $state<number | null>(null);
  let actionError = $state("");

  // Reject dialog
  const QUICK_REASONS = ["Bentrok perguruan / daerah", "Juri kurang", "Salah orang", "Wasit tidak hadir", "Susunan tidak sesuai"];
  let rejectFor = $state<number | null>(null);
  let rejectChips = $state<string[]>([]);
  let rejectText = $state("");

  let socket: TatamiSocket | null = null;
  let lastPending = -1;

  async function loadEvent() {
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/events`, { cache: "no-store" });
      const list: EventItem[] = (await res.json())?.data ?? [];
      if (!eventId && list.length) eventId = list[0].id;
      eventName = list.find((e) => e.id === eventId)?.nama_event ?? "";
    } catch {
      loadError = "Meja utama belum terhubung, mencoba lagi…";
    }
  }

  async function load() {
    if (!eventId || !eventName) {
      await loadEvent();
      if (!eventId) return;
    }
    try {
      const q = await apiCall<ApprovalQueue>("get_approval_queue", { eventId });
      // officials of each waiting bout (fetched once: changing them makes the bout a draft)
      const missing = q.pending.filter((p) => !panels[p.match_id]);
      if (missing.length) {
        const got = await Promise.all(
          missing.map((p) =>
            apiCall<BoutPanel>("get_bout_panel", { matchId: p.match_id, tatamiNumber: p.tatami_number ?? 1 }).catch(() => null)
          )
        );
        const next = { ...panels };
        got.forEach((p, i) => {
          if (p) next[missing[i].match_id] = p;
        });
        panels = next;
      }
      if (lastPending >= 0 && q.pending.length > lastPending) chime();
      lastPending = q.pending.length;
      queue = q;
      loadError = "";
    } catch (e: any) {
      loadError = typeof e === "string" ? e : e?.message || "Gagal memuat antrian";
    }
  }

  async function decide(matchId: number, approve: boolean, note: string | null = null) {
    busyId = matchId;
    actionError = "";
    try {
      await apiCall("decide_bout_approval", { matchId, approve, note });
      const { [matchId]: _gone, ...rest } = panels;
      panels = rest;
      await load();
    } catch (e: any) {
      actionError = typeof e === "string" ? e : e?.message || "Gagal menyimpan keputusan";
      await load();
    } finally {
      busyId = null;
    }
  }

  function openReject(matchId: number) {
    rejectFor = matchId;
    rejectChips = [];
    rejectText = "";
  }

  async function confirmReject() {
    const id = rejectFor;
    if (!id) return;
    const note = [...rejectChips, rejectText.trim()].filter(Boolean).join("; ");
    rejectFor = null;
    await decide(id, false, note || null);
  }

  /** a short two-tone chime when a new request arrives */
  function chime() {
    try {
      const ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
      [880, 1175].forEach((f, i) => {
        const o = ctx.createOscillator();
        const g = ctx.createGain();
        o.frequency.value = f;
        g.gain.setValueAtTime(0.0001, ctx.currentTime + i * 0.18);
        g.gain.exponentialRampToValueAtTime(0.25, ctx.currentTime + i * 0.18 + 0.02);
        g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + i * 0.18 + 0.3);
        o.connect(g).connect(ctx.destination);
        o.start(ctx.currentTime + i * 0.18);
        o.stop(ctx.currentTime + i * 0.18 + 0.32);
      });
    } catch {}
  }

  function roundLabel(p: BoutPanel): string {
    const r = Number(p.round) || 1;
    return r === 4 ? "Final" : r === 3 ? "Semifinal" : `Babak ${r}`;
  }

  function timeOf(s: string | null): string {
    if (!s) return "";
    const t = s.split(" ")[1] ?? s;
    return t.slice(0, 5);
  }

  onMount(() => {
    load();
    socket = connectTatamiSocket(
      "chief-referee",
      0,
      (msg) => {
        linkLive = true;
        if (msg?.type === "bout_status" || msg?.type === "referee_updated") load();
      },
      () => {
        linkLive = true;
        load();
      },
      () => (linkLive = false),
      { staleMs: 20000, onStale: () => load() }
    );
    const poll = setInterval(load, 4000);
    return () => {
      socket?.close();
      clearInterval(poll);
    };
  });
</script>

<svelte:head>
  <title>Chief Referee</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col select-none font-sans overflow-hidden touch-manipulation">
  <header class="bg-[#14161b] border-b border-white/10 px-4 py-2.5 flex items-center justify-between gap-2 shrink-0 shadow-lg">
    <div class="flex items-center gap-2 min-w-0">
      <PortalExitButton title="Keluar dari Chief Referee?" />
      <span class="px-2.5 py-1 rounded-lg bg-violet-500/20 text-violet-100 font-mono font-black text-xs tracking-wider border border-violet-400/40 shrink-0">CHIEF REFEREE</span>
      <span class="text-xs text-zinc-400 truncate">{eventName}</span>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      <span class="px-2 py-0.5 rounded-full text-[11px] font-mono font-bold border {queue?.pending.length ? 'bg-violet-500/20 text-violet-100 border-violet-400/50' : 'bg-white/[0.06] text-zinc-400 border-white/10'}">
        {queue?.pending.length ?? 0} MENUNGGU
      </span>
      <span class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono border {linkLive ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20' : 'bg-amber-500/10 text-amber-300 border-amber-500/30'}">
        <span class="w-1.5 h-1.5 rounded-full animate-pulse {linkLive ? 'bg-emerald-400' : 'bg-amber-300'}"></span>
        {linkLive ? "ONLINE" : "ONLINE · HTTP"}
      </span>
    </div>
  </header>

  <main class="flex-1 min-h-0 overflow-y-auto p-3 flex flex-col gap-3">
    {#if loadError}
      <div class="rounded-xl px-3 py-2 text-xs font-semibold bg-amber-500/10 border border-amber-500/30 text-amber-200">{loadError}</div>
    {/if}
    {#if actionError}
      <div class="rounded-xl px-3 py-2 text-xs font-semibold bg-red-500/10 border border-red-500/30 text-red-200">{actionError}</div>
    {/if}

    {#if queue && queue.pending.length === 0}
      <div class="rounded-2xl border border-white/[0.08] bg-[#13151b] py-14 text-center">
        <div class="text-sm font-bold text-white">Tidak ada permintaan</div>
        <div class="text-xs text-zinc-500 mt-1">Permintaan muncul saat operator membuka partai yang wasitnya sudah ditetapkan.</div>
      </div>
    {/if}

    {#each queue?.pending ?? [] as item (item.match_id)}
      {@const p = panels[item.match_id]}
      <section class="rounded-2xl border-2 border-violet-400/40 bg-[#14161d] shadow-xl">
        <div class="px-3 py-2.5 border-b border-white/[0.08] flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2 flex-wrap min-w-0">
            <span class="px-2.5 py-1 rounded-lg bg-white text-black font-mono font-black text-sm">TATAMI {item.tatami_number ?? "-"}</span>
            <span class="text-sm font-black text-white uppercase">{item.category ?? ""}</span>
            {#if p}<span class="text-xs font-mono text-zinc-400">Partai #{p.urutan ?? "-"} • {roundLabel(p)}</span>{/if}
          </div>
          <span class="text-[11px] font-mono text-violet-200">Menunggu sejak {timeOf(item.status_at)}</span>
        </div>

        {#if !p}
          <div class="p-4 text-xs font-mono text-zinc-500">Memuat wasit & juri…</div>
        {:else}
          {@const judges = boutJudgeCount(p, null)}
          {@const seats = boutSeats(p.category, judges)}
          {@const eff = effectivePanel(p, seats)}
          {@const conflicts = seats.filter((s) => eff.seats[s.key] && conflictReason(eff.seats[s.key]!, p)).length}
          {@const empty = seats.filter((s) => !eff.seats[s.key]).length}
          <div class="p-3 flex flex-col gap-3">
            <div class="flex items-center flex-wrap gap-x-2 gap-y-1 text-[13px]">
              <span class="px-2 py-0.5 rounded bg-red-600 text-white font-mono font-black text-[11px]">AKA</span>
              <span class="font-bold text-white">{p.aka_nama ?? "-"}</span>
              <span class="text-xs text-zinc-400">({[p.aka_perguruan, p.aka_utusan].filter(Boolean).join(" • ") || "-"})</span>
              <span class="px-1.5 py-0.5 rounded bg-white/10 text-white font-mono font-black text-[10px] mx-1">VS</span>
              <span class="px-2 py-0.5 rounded bg-sky-600 text-white font-mono font-black text-[11px]">AO</span>
              <span class="font-bold text-white">{p.ao_nama ?? "-"}</span>
              <span class="text-xs text-zinc-400">({[p.ao_perguruan, p.ao_utusan].filter(Boolean).join(" • ") || "-"})</span>
            </div>

            <div class="grid gap-2" style="grid-template-columns: repeat(auto-fit, minmax(118px, 1fr));">
              {#each seats as seat (seat.key)}
                {@const h = eff.seats[seat.key]}
                <RefereeSeatCard {seat} holder={h} conflict={h ? conflictReason(h, p) : null} readonly photo={72} />
              {/each}
            </div>

            <div class="flex items-center justify-between flex-wrap gap-2">
              <div class="flex items-center gap-2 text-[11px] font-mono font-bold">
                <span class="text-zinc-400">{judges} juri</span>
                {#if conflicts}<span class="px-2 py-0.5 rounded bg-red-600/20 text-red-200 border border-red-500/40">⚠ {conflicts} bentrok</span>{/if}
                {#if empty}<span class="px-2 py-0.5 rounded bg-amber-500/15 text-amber-200 border border-amber-500/40">{empty} kursi kosong</span>{/if}
              </div>
              <div class="flex items-center gap-2">
                <button
                  type="button"
                  disabled={busyId === item.match_id}
                  onclick={() => openReject(item.match_id)}
                  class="h-12 px-6 rounded-xl border-2 border-red-500/60 bg-red-500/10 text-red-200 font-black text-sm hover:bg-red-600 hover:text-white transition disabled:opacity-40"
                >
                  Tolak
                </button>
                <button
                  type="button"
                  disabled={busyId === item.match_id}
                  onclick={() => decide(item.match_id, true)}
                  class="h-12 px-8 rounded-xl bg-emerald-500 hover:bg-emerald-400 text-black font-black text-sm shadow-lg transition disabled:opacity-40"
                >
                  {busyId === item.match_id ? "Menyimpan…" : "Terima"}
                </button>
              </div>
            </div>
          </div>
        {/if}
      </section>
    {/each}

    {#if queue?.recent.length}
      <section class="rounded-2xl border border-white/[0.08] bg-[#13151b]">
        <div class="px-3 py-2 border-b border-white/[0.08] text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400">Riwayat keputusan</div>
        <div class="divide-y divide-white/[0.05]">
          {#each queue.recent as r (r.id)}
            <div class="px-3 py-2 flex items-center gap-3 text-xs">
              <span class="font-mono text-zinc-500 shrink-0">{timeOf(r.at)}</span>
              <span class="px-1.5 py-0.5 rounded text-[10px] font-black border shrink-0 {r.action === 'approve' ? 'bg-emerald-500/15 text-emerald-300 border-emerald-500/40' : 'bg-red-500/15 text-red-300 border-red-500/40'}">
                {r.action === "approve" ? "DITERIMA" : "DITOLAK"}
              </span>
              <span class="font-mono text-zinc-300 shrink-0">T{r.tatami_number ?? "-"}</span>
              <span class="min-w-0 flex-1 truncate text-zinc-200">
                {r.category ?? ""} · Partai #{r.urutan ?? "-"} · {r.aka_nama ?? "-"} vs {r.ao_nama ?? "-"}
              </span>
              {#if r.note}<span class="text-zinc-400 truncate max-w-[40%]">{r.note}</span>{/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}
  </main>

  {#if rejectFor}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs" role="presentation" onclick={() => (rejectFor = null)}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="w-full max-w-md bg-[#181a20] border border-white/10 rounded-2xl shadow-2xl" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
        <div class="px-5 py-4 bg-[#111215] border-b border-white/[0.08] rounded-t-2xl">
          <h3 class="text-sm font-bold text-white tracking-tight">Tolak Match</h3>
        </div>
        <div class="p-5 flex flex-col gap-3">
          <div class="flex flex-wrap gap-2">
            {#each QUICK_REASONS as r (r)}
              {@const on = rejectChips.includes(r)}
              <button
                type="button"
                onclick={() => (rejectChips = on ? rejectChips.filter((x) => x !== r) : [...rejectChips, r])}
                class="px-3 py-2 rounded-lg text-xs font-semibold border transition {on ? 'bg-red-600 text-white border-red-500' : 'bg-white/[0.04] text-zinc-300 border-white/10 hover:text-white'}"
              >
                {r}
              </button>
            {/each}
          </div>
          <textarea
            bind:value={rejectText}
            rows="3"
            placeholder="Catatan (opsional)"
            class="w-full bg-[#111215] text-sm text-white px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white outline-none resize-none"
          ></textarea>
        </div>
        <div class="px-5 py-3.5 bg-[#111215] border-t border-white/[0.08] rounded-b-2xl flex justify-end gap-2">
          <button type="button" onclick={() => (rejectFor = null)} class="h-10 px-4 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">Batal</button>
          <button type="button" onclick={confirmReject} class="h-10 px-5 bg-red-600 hover:bg-red-500 text-white font-bold text-xs rounded-xl shadow-md">Tolak Match</button>
        </div>
      </div>
    </div>
  {/if}
</div>
