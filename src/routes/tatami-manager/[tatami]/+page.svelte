<script lang="ts">
  // Tatami Manager: the brackets of a tatami (kata and kumite), like LO Kata, to assign the
  // referee and judges of each bout. Template for now: the assignment slots are shown, but
  // the referee / judge list comes later from the Wasit-Juri channel.
  // Realtime: follows the bout open in the control panel (live state); the shared socket
  // reconnects by itself and every reconnect reloads the brackets and asks for the state.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { getHostBaseUrl } from "$lib/api_client";
  import { computePodium, deriveBracketMeta, TREE_COL_GAP, TREE_COL_WIDTH, TREE_HEIGHT, treeWidthFor } from "$lib/bracket_layout";
  import BracketTree from "$lib/components/BracketTree.svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));
  let eventFromUrl = $derived(Number(page.url.searchParams.get("event")) || null);
  let eventFromMatch = $state<number | null>(null);
  let eventId = $derived(eventFromUrl ?? eventFromMatch);
  let eventQuery = $derived(eventFromUrl ? `?event=${eventFromUrl}` : "");

  type LinkStatus = "connecting" | "live" | "offline";
  let linkStatus = $state<LinkStatus>("connecting");
  let socket: TatamiSocket | null = null;

  // ---- Follow the tatami (default) or browse any bagan of the event ----
  const FOLLOW_KEY = "ambrilindo.tatamiManager.follow";
  function readFollow(): boolean {
    try {
      return localStorage.getItem(FOLLOW_KEY) !== "0";
    } catch {
      return true;
    }
  }
  let followTatami = $state(readFollow());

  // Bouts assigned to this tatami (which bagan run here, and the open bout's bagan)
  let tatamiMatches = $state<any[]>([]);
  let tatamiBagans = $derived.by(() => {
    const seen = new Map<number, string>();
    for (const m of tatamiMatches) if (m.bagan_id && !seen.has(m.bagan_id)) seen.set(m.bagan_id, m.nama_bagan || `Bagan ${m.bagan_id}`);
    return [...seen].map(([id, nama_bagan]) => ({ id, nama_bagan }));
  });
  let eventBagans = $state<any[]>([]);

  // Bracket on screen
  let viewBaganId = $state<number | null>(null);
  let viewBagan = $state<any | null>(null);
  let baganMatches = $state<any[]>([]);
  let isLoading = $state(false);

  let livePanelMatchId = $state<number | null>(null);

  // Bout being viewed (assignment sheet)
  let editMatchId = $state<number | null>(null);
  let editMatch = $derived(editMatchId ? baganMatches.find((m) => m.id === editMatchId) ?? null : null);

  // Kata or kumite: decides the officials of a bout
  let isKataBagan = $derived(!!viewBagan && (String(viewBagan.tipe_tanding) === "1" || /KATA/i.test(viewBagan.nama_bagan || "")));
  /** Officials of one bout: kumite 1 referee + 4 judges, kata 5 judges */
  let officialSlots = $derived(
    isKataBagan
      ? [1, 2, 3, 4, 5].map((n) => ({ key: `juri${n}`, role: "Juri", label: `Juri ${n}` }))
      : [
          { key: "wasit", role: "Wasit", label: "Wasit (Shushin)" },
          ...[1, 2, 3, 4].map((n) => ({ key: `juri${n}`, role: "Juri", label: `Juri ${n}` })),
        ]
  );

  // ---- Bracket sizing: fill the width, scroll down if taller ----
  let stageW = $state(800);
  let stageEl: HTMLElement | null = $state(null);
  let skipRounds = $derived.by(() => {
    const rounds = baganMatches.filter((m) => m.atlet1_id || m.atlet2_id).map((m) => Number(m.round) || 1);
    if (rounds.length === 0) return 0;
    return Math.max(0, Math.min(2, Math.min(...rounds) - 1));
  });
  let treeFit = $derived.by(() => {
    const columns = 4 - skipRounds;
    const avail = Math.max(320, stageW - 16);
    let colWidth = (avail - (columns - 1) * TREE_COL_GAP) / columns;
    let scale = 1;
    if (colWidth < TREE_COL_WIDTH) {
      colWidth = TREE_COL_WIDTH;
      scale = Math.max(0.55, avail / treeWidthFor(colWidth, columns));
    }
    const cardScale = Math.max(1, Math.min(1.8, colWidth / 170));
    return { columns, colWidth: Math.floor(colWidth), scale, cardScale };
  });
  let treeMeta = $derived(viewBagan ? deriveBracketMeta(viewBagan.nama_bagan || "", viewBagan.pool) : null);
  let isElimination = $derived(!!viewBagan && !viewBagan.round_robin && baganMatches.some((m) => Number(m.round) >= 1 && Number(m.round) <= 4));

  // ---- Data loading ----
  async function getJson(path: string): Promise<any> {
    const res = await fetch(`${getHostBaseUrl()}${path}`, { cache: "no-store" });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const json = await res.json();
    return json?.status === "success" ? json.data : null;
  }

  async function fetchTatamiMatches(): Promise<number | null> {
    let currentBagan: number | null = null;
    try {
      const current = await getJson(`/api/tatami/${tatamiNumber}/current-match${eventQuery}`).catch(() => null);
      const all = await getJson(`/api/tatami/${tatamiNumber}/matches${eventQuery}`).catch(() => null);
      if (Array.isArray(all)) tatamiMatches = all;
      if (current?.id && !livePanelMatchId) livePanelMatchId = Number(current.id);
      currentBagan = current?.bagan_id ? Number(current.bagan_id) : null;
      const anyBagan = currentBagan ?? tatamiMatches[0]?.bagan_id ?? null;
      if (!eventFromUrl && !eventFromMatch && anyBagan) {
        const b = await getJson(`/api/bagan/${anyBagan}`).catch(() => null);
        if (b?.event_id) eventFromMatch = Number(b.event_id);
      }
    } catch (err) {
      console.warn("Gagal memuat partai tatami:", err);
    }
    return currentBagan;
  }

  async function fetchEventBagans() {
    if (!eventId) return;
    try {
      const list = await getJson(`/api/bagans/${eventId}`);
      if (Array.isArray(list)) {
        eventBagans = list.sort((a: any, b: any) => String(a.nama_bagan || "").localeCompare(String(b.nama_bagan || "")));
      }
    } catch (err) {
      console.warn("Gagal memuat daftar bagan:", err);
    }
  }

  async function showBagan(baganId: number | null) {
    if (!baganId) {
      viewBaganId = null;
      viewBagan = null;
      baganMatches = [];
      return;
    }
    if (baganId !== viewBaganId) {
      baganMatches = [];
      editMatchId = null;
    }
    viewBaganId = baganId;
    isLoading = true;
    try {
      const [b, list] = await Promise.all([getJson(`/api/bagan/${baganId}`), getJson(`/api/detail-bagan/${baganId}`)]);
      if (viewBaganId !== baganId) return;
      viewBagan = b;
      baganMatches = (Array.isArray(list) ? list : []).filter((m: any) => !m.team);
    } catch (err) {
      console.warn("Gagal memuat bagan:", err);
    } finally {
      isLoading = false;
    }
  }

  function panelBaganId(): number | null {
    if (!livePanelMatchId) return null;
    const m = tatamiMatches.find((x) => x.id === livePanelMatchId) ?? baganMatches.find((x) => x.id === livePanelMatchId);
    return m?.bagan_id ? Number(m.bagan_id) : null;
  }

  async function refreshAll() {
    const currentBagan = await fetchTatamiMatches();
    if (followTatami) {
      await showBagan(panelBaganId() ?? viewBaganId ?? currentBagan ?? tatamiBagans[0]?.id ?? null);
    } else {
      await fetchEventBagans();
      await showBagan(viewBaganId ?? eventBagans[0]?.id ?? null);
    }
  }

  function setFollow(on: boolean) {
    followTatami = on;
    try {
      localStorage.setItem(FOLLOW_KEY, on ? "1" : "0");
    } catch {}
    if (on) {
      showBagan(panelBaganId() ?? viewBaganId ?? tatamiBagans[0]?.id ?? null);
      socket?.send({ type: "request_tatami_state", tatami: tatamiNumber });
    } else {
      fetchEventBagans();
    }
  }

  function openSheet(m: any) {
    if (!m || (!m.atlet1_id && !m.atlet2_id)) return;
    editMatchId = m.id;
  }

  // ---- Realtime ----
  function handleLiveState(d: any) {
    if (!d || Number(d.tatami) !== tatamiNumber) return;
    const mId = d.matchId ? Number(d.matchId) : null;
    const changed = mId !== livePanelMatchId;
    livePanelMatchId = mId;
    if (followTatami && mId && changed) {
      const b = panelBaganId();
      if (b && b !== viewBaganId) showBagan(b);
      else if (!b) refreshAll(); // a bout we do not know yet
    }
  }

  onMount(() => {
    refreshAll();
    socket = connectTatamiSocket(
      "tatami-manager",
      tatamiNumber,
      (msg) => {
        linkStatus = "live";
        if (msg?.type === "tatami_live_state") handleLiveState(msg.payload || msg.data);
        else if (msg?.type === "bracket_updated") refreshAll();
      },
      (sock) => {
        linkStatus = "live";
        refreshAll();
        sock.send({ type: "request_tatami_state", tatami: tatamiNumber });
      },
      () => (linkStatus = typeof navigator !== "undefined" && navigator.onLine === false ? "offline" : "connecting"),
      { staleMs: 15000, onStale: (sock) => sock.send({ type: "request_tatami_state", tatami: tatamiNumber }) }
    );
    let obs: ResizeObserver | null = null;
    if (stageEl) {
      obs = new ResizeObserver(([e]) => (stageW = e.contentRect.width));
      obs.observe(stageEl);
    }
    return () => {
      socket?.close();
      obs?.disconnect();
    };
  });

  function roundLabel(m: any): string {
    const r = Number(m?.round) || 1;
    return r === 4 ? "Final" : r === 3 ? "Semifinal" : `Babak ${r}`;
  }
</script>

<svelte:head>
  <title>Tatami Manager • Tatami {tatamiNumber}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col select-none font-sans overflow-hidden touch-manipulation">
  <!-- Header -->
  <header class="bg-[#14161b] border-b border-white/10 px-4 py-2.5 flex items-center justify-between gap-2 shrink-0 shadow-lg relative z-20">
    <div class="flex items-center gap-2 min-w-0">
      <PortalExitButton title="Keluar dari Tatami Manager?" />
      <span class="px-2.5 py-1 rounded-lg bg-zinc-500/20 text-zinc-200 font-mono font-black text-xs tracking-wider border border-zinc-400/30 shrink-0">
        TATAMI MANAGER • TATAMI {tatamiNumber}
      </span>
      <div
        class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono shrink-0 {linkStatus === 'live'
          ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
          : linkStatus === 'connecting'
            ? 'bg-amber-500/10 text-amber-300 border border-amber-500/30'
            : 'bg-red-500/10 text-red-400 border border-red-500/20'}"
      >
        <span class="w-1.5 h-1.5 rounded-full {linkStatus === 'live' ? 'bg-emerald-400 animate-pulse' : linkStatus === 'connecting' ? 'bg-amber-300 animate-pulse' : 'bg-red-400'}"></span>
        <span>{linkStatus === "live" ? "ONLINE" : linkStatus === "connecting" ? "MENYAMBUNG ULANG…" : "OFFLINE"}</span>
      </div>
    </div>
    <button
      onclick={refreshAll}
      class="p-1.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/10 transition shrink-0"
      title="Segarkan data"
      aria-label="Segarkan data"
    >
      <svg class="w-4 h-4 {isLoading ? 'animate-spin text-zinc-200' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
    </button>
  </header>

  <!-- Which bracket: follow the tatami, or any bagan of the event -->
  <div class="bg-[#111317] border-b border-white/[0.06] px-4 py-2 flex flex-wrap items-center gap-x-3 gap-y-2 shrink-0">
    <button type="button" role="switch" aria-checked={followTatami} onclick={() => setFollow(!followTatami)} class="flex items-center gap-2 text-xs font-bold shrink-0">
      <span class="relative w-9 h-5 rounded-full transition {followTatami ? 'bg-zinc-200' : 'bg-white/[0.12]'}">
        <span class="absolute top-0.5 w-4 h-4 rounded-full transition-all {followTatami ? 'left-[18px] bg-black' : 'left-0.5 bg-white'}"></span>
      </span>
      <span class={followTatami ? "text-white" : "text-zinc-400"}>Ikut bagan tatami {tatamiNumber}</span>
    </button>

    {#if followTatami}
      <div class="flex-1 min-w-0 flex items-center gap-1.5 overflow-x-auto scrollbar-thin">
        {#each tatamiBagans as b (b.id)}
          <button
            type="button"
            onclick={() => showBagan(b.id)}
            class="shrink-0 px-3 py-1.5 rounded-lg text-xs font-semibold transition border {viewBaganId === b.id ? 'bg-white/[0.1] text-white border-white/40' : 'bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white'}"
          >
            {b.nama_bagan}
            {#if b.id === panelBaganId()}<span class="ml-1 inline-block w-1.5 h-1.5 rounded-full bg-emerald-400 align-middle"></span>{/if}
          </button>
        {:else}
          <span class="text-xs text-zinc-500">Belum ada bagan di tatami ini</span>
        {/each}
      </div>
    {:else}
      <select
        value={viewBaganId}
        onchange={(e) => showBagan(Number((e.target as HTMLSelectElement).value) || null)}
        class="flex-1 min-w-0 bg-[#181a20] text-xs font-semibold text-white px-2.5 py-1.5 rounded-xl border border-white/10 focus:outline-none focus:border-white"
      >
        <option value={null}>{eventBagans.length ? "Pilih bagan…" : "Memuat bagan…"}</option>
        {#each eventBagans as b (b.id)}
          <option value={b.id}>{b.nama_bagan}</option>
        {/each}
      </select>
    {/if}
  </div>

  <!-- The bracket: tap a bout to see / assign its officials -->
  <main bind:this={stageEl} class="flex-1 min-h-0 overflow-auto p-2">
    {#if !viewBagan || !treeMeta}
      <div class="h-full flex items-center justify-center text-xs font-mono text-zinc-500">
        {isLoading ? "Memuat bagan…" : followTatami ? "Belum ada bagan di tatami ini" : "Pilih bagan"}
      </div>
    {:else if isElimination}
      {@const treeW = treeWidthFor(treeFit.colWidth, treeFit.columns)}
      <div style="width: {treeW * treeFit.scale}px; height: {TREE_HEIGHT * treeFit.scale}px;">
        <div style="transform: scale({treeFit.scale}); transform-origin: top left; width: {treeW}px; height: {TREE_HEIGHT}px;">
          <BracketTree
            matches={baganMatches}
            meta={treeMeta}
            podium={computePodium(baganMatches, treeMeta)}
            theme="dark"
            interactive
            showPodium={false}
            colWidth={treeFit.colWidth}
            cardScale={treeFit.cardScale}
            {skipRounds}
            highlightMatchId={livePanelMatchId}
            clickHint="Ketuk untuk mengatur wasit & juri"
            onMatchClick={openSheet}
          />
        </div>
      </div>
    {:else}
      <!-- Not an elimination tree (e.g. round robin): plain list of bouts -->
      <div class="grid gap-2" style="grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));">
        {#each baganMatches.filter((m) => m.atlet1_id || m.atlet2_id) as m (m.id)}
          <button
            type="button"
            onclick={() => openSheet(m)}
            class="text-left rounded-xl border p-3 bg-[#16181f] {m.id === livePanelMatchId ? 'border-emerald-400 ring-2 ring-emerald-400/40' : 'border-white/10'}"
          >
            <div class="text-[10px] font-mono text-zinc-400 mb-1">Partai #{m.urutan || 1} • {roundLabel(m)}</div>
            <div class="text-xs font-black text-red-300 truncate">{m.atlet1_nama || "-"}</div>
            <div class="text-xs font-black text-blue-300 truncate">{m.atlet2_nama || "-"}</div>
          </button>
        {/each}
      </div>
    {/if}
  </main>

  <footer class="bg-[#14161b] border-t border-white/10 px-4 py-2 shrink-0 flex items-center gap-4 text-[11px] font-mono text-zinc-400">
    <span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded ring-2 ring-emerald-400"></span>Partai di control panel</span>
    <span>Ketuk partai untuk mengatur wasit & juri</span>
  </footer>

  <!-- Officials of the tapped bout (template: the list of referees comes from the Wasit-Juri channel) -->
  {#if editMatch}
    <div class="fixed inset-0 z-40 flex items-end sm:items-center justify-center bg-black/60 backdrop-blur-xs" role="presentation" onclick={() => (editMatchId = null)}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="w-full sm:max-w-xl bg-[#181a20] border border-white/10 rounded-t-2xl sm:rounded-2xl shadow-2xl flex flex-col max-h-[92vh]"
        role="dialog"
        aria-modal="true"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
      >
        <div class="px-4 py-3 border-b border-white/10 flex items-center justify-between gap-2">
          <div class="min-w-0">
            <div class="text-sm font-black text-white uppercase truncate">Partai #{editMatch.urutan || 1} • {roundLabel(editMatch)}</div>
            <div class="text-[11px] font-mono {editMatch.id === livePanelMatchId ? 'text-emerald-300' : 'text-zinc-500'}">
              {editMatch.id === livePanelMatchId ? "Sedang dibuka di control panel" : (viewBagan?.nama_bagan ?? "")}
            </div>
          </div>
          <button onclick={() => (editMatchId = null)} aria-label="Tutup" class="p-1.5 rounded-lg hover:bg-white/10 text-zinc-400 hover:text-white transition">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
          </button>
        </div>

        <div class="p-3 flex flex-col gap-3 overflow-y-auto">
          <!-- Athletes -->
          <div class="grid grid-cols-2 gap-2">
            {#each [["ao", "AO", editMatch.atlet2_nama, editMatch.atlet2_kontingen], ["aka", "AKA", editMatch.atlet1_nama, editMatch.atlet1_kontingen]] as [side, label, name, club] (side)}
              <div class="rounded-xl border p-2.5 min-w-0 {side === 'aka' ? 'border-red-600/60 bg-red-950/20' : 'border-blue-600/60 bg-blue-950/20'}">
                <span class="px-1.5 py-0.5 rounded text-white font-mono font-black text-[10px] {side === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">{label}</span>
                <div class="text-sm font-black text-white truncate mt-1">{name || "BYE"}</div>
                <div class="text-[11px] text-zinc-400 truncate">{club || "-"}</div>
              </div>
            {/each}
          </div>

          <!-- Officials -->
          <div class="rounded-2xl border border-white/10 bg-[#13151b] p-3 flex flex-col gap-2">
            <div class="flex items-center justify-between gap-2">
              <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400">
                {isKataBagan ? "Panel Juri Kata" : "Wasit & Juri Kumite"}
              </span>
              <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/30">SEGERA</span>
            </div>
            {#each officialSlots as slot (slot.key)}
              <div class="flex items-center justify-between gap-2 rounded-xl bg-[#101216] border border-white/10 px-3 py-2.5">
                <div class="min-w-0">
                  <div class="text-[10px] font-mono font-bold uppercase text-zinc-500">{slot.role}</div>
                  <div class="text-sm font-black text-zinc-300 truncate">{slot.label}</div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                  <span class="text-xs font-mono text-zinc-500">Belum ditentukan</span>
                  <button
                    type="button"
                    disabled
                    class="h-9 px-3.5 bg-white/[0.04] text-zinc-500 border border-white/[0.08] font-semibold text-xs rounded-xl cursor-not-allowed"
                    title="Daftar wasit & juri dari channel Wasit-Juri (segera)"
                  >
                    Pilih
                  </button>
                </div>
              </div>
            {/each}
            <p class="text-[11px] text-zinc-500 px-1">Daftar wasit dan juri diambil dari channel Wasit-Juri.</p>
          </div>
        </div>

        <div class="px-4 py-3 border-t border-white/10 flex items-center justify-end">
          <button onclick={() => (editMatchId = null)} class="h-10 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md">Tutup</button>
        </div>
      </div>
    </div>
  {/if}
</div>
