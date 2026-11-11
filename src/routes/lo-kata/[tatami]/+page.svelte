<script lang="ts">
  // LO / Guide (Liaison Officer at the mat): the brackets of a tatami in the order of its
  // schedule, kata and kumite alike, so the guide knows which bout is next and who to call.
  // Kata bagan: tap a bout to enter its kata (also before the operator opens it); it is
  // saved and sent at once. Kumite bagan: tap a bout for the athletes and their contingent.
  // The guide can also look at a neighbouring tatami.
  // Realtime both ways: the control panel's live state carries the kata it shows (and which
  // bout is open), kata from another LO tablet arrive as lo_kata_broadcast. The shared
  // socket reconnects by itself; every reconnect reloads the bracket and asks the control
  // panel for its state.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { KATA_ENTRIES, type KataItem } from "$lib/kata_data";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { getHostBaseUrl } from "$lib/api_client";
  import { computePodium, deriveBracketMeta, TREE_COL_GAP, TREE_COL_WIDTH, TREE_HEIGHT, treeWidthFor } from "$lib/bracket_layout";
  import BracketTree from "$lib/components/BracketTree.svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";

  /** this tablet's tatami (from the link) */
  let tatamiNumber = $derived(Number(page.params.tatami || "1"));
  let eventFromUrl = $derived(Number(page.url.searchParams.get("event")) || null);
  let eventFromMatch = $state<number | null>(null);
  let eventId = $derived(eventFromUrl ?? eventFromMatch);
  let eventQuery = $derived(eventFromUrl ? `?event=${eventFromUrl}` : "");

  type LinkStatus = "connecting" | "live" | "offline";
  let linkStatus = $state<LinkStatus>("connecting");
  /** socket of this tablet's tatami */
  let socket: TatamiSocket | null = null;
  /** socket of a neighbouring tatami while it is on screen (listens only) */
  let viewSocket: TatamiSocket | null = null;

  // ---- Tatami on screen: this tablet's own, or a neighbouring one ----
  let viewTatami = $state(Number(page.params.tatami || "1"));
  let isHomeTatami = $derived(viewTatami === tatamiNumber);
  let tatamiNumbers = $state<number[]>([]);
  let tatamiChoices = $derived([...new Set([...tatamiNumbers, tatamiNumber])].sort((a, b) => a - b));

  // ---- Follow the bout open in the control panel (default) ----
  const FOLLOW_KEY = "ambrilindo.loKata.follow";
  function readFollow(): boolean {
    try {
      return localStorage.getItem(FOLLOW_KEY) !== "0";
    } catch {
      return true;
    }
  }
  let followTatami = $state(readFollow());

  // Bouts assigned to the tatami on screen
  let tatamiMatches = $state<any[]>([]);
  // Every bagan of the event (to turn the scheduled categories into bagans)
  let eventBagans = $state<any[]>([]);
  let eventDataFor: number | null = null;
  // Timetable: per tatami number, its categories in run order
  type SchedItem = { nomorTandingId: number; time: string };
  let schedule = $state<Record<number, SchedItem[]>>({});

  function isKataBagan(b: any): boolean {
    return !!b && (String(b.tipe_tanding) === "1" || /KATA/i.test(b.nama_bagan || ""));
  }

  type BaganChip = { id: number; nama_bagan: string; time: string; kata: boolean };
  /** Bagans of the tatami on screen: the schedule first, then bouts assigned to it, then the
   *  bagan its control panel has open */
  let tatamiBagans = $derived.by(() => {
    const out: BaganChip[] = [];
    const seen = new Set<number>();
    const byId = new Map<number, any>(eventBagans.map((b) => [Number(b.id), b]));
    const add = (id: number, nama: string | null | undefined, time = "") => {
      if (!id || seen.has(id)) return;
      seen.add(id);
      const b = byId.get(id);
      out.push({ id, nama_bagan: b?.nama_bagan || nama || `Bagan ${id}`, time, kata: isKataBagan(b ?? { nama_bagan: nama }) });
    };
    for (const s of schedule[viewTatami] ?? []) {
      eventBagans
        .filter((b) => Number(b.nomor_tanding_id) === s.nomorTandingId)
        // pools first, the final of the pools (pool 0) after them
        .sort((a, b) => Number(Number(a.pool) === 0) - Number(Number(b.pool) === 0) || String(a.nama_bagan || "").localeCompare(String(b.nama_bagan || "")))
        .forEach((b) => add(Number(b.id), b.nama_bagan, s.time));
    }
    for (const m of tatamiMatches) add(Number(m.bagan_id), m.nama_bagan);
    if (livePanelBaganId) add(livePanelBaganId, viewBagan?.id === livePanelBaganId ? viewBagan.nama_bagan : null);
    return out;
  });

  // Bracket on screen
  let viewBaganId = $state<number | null>(null);
  let viewBagan = $state<any | null>(null);
  let baganMatches = $state<any[]>([]);
  let isLoading = $state(false);
  let viewIsKata = $derived(isKataBagan(viewBagan));

  // Bout open in the control panel of the tatami on screen (from its live state)
  let livePanelMatchId = $state<number | null>(null);
  let livePanelBaganId = $state<number | null>(null);

  // Bout tapped (kata sheet, or the athletes of a kumite bout)
  let editMatchId = $state<number | null>(null);
  let editMatch = $derived(editMatchId ? baganMatches.find((m) => m.id === editMatchId) ?? null : null);
  let akaKata = $state("");
  let aoKata = $state("");

  type SendStatus = "idle" | "sending" | "sent" | "error";
  let sendStatus = $state<SendStatus>("idle");
  // Right after our own send, older live states may still be on their way: ignore them
  let ignoreLiveUntil = 0;

  // ---- Kata picker ----
  let isPickerOpen = $state(false);
  let pickerTarget = $state<"aka" | "ao">("aka");
  let searchQuery = $state("");
  let filteredKataList = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return KATA_ENTRIES;
    return KATA_ENTRIES.filter((k) => k.name.toLowerCase().includes(q) || k.code.toLowerCase().includes(q));
  });

  /** "4 - Aoyagi" and "Aoyagi" are the same kata */
  function kataKey(v: string | null | undefined): string {
    return String(v || "").replace(/^\s*\d+\s*-\s*/, "").trim().toLowerCase();
  }
  /** Kata this athlete already used in other bouts of the bracket */
  function previousKatas(atletId: number | null | undefined): string[] {
    if (!editMatch || !atletId) return [];
    return baganMatches
      .filter((m) => m.id !== editMatch.id && (m.atlet1_id === atletId || m.atlet2_id === atletId))
      .map((m) => (m.atlet1_id === atletId ? m.kata1 : m.kata2))
      .filter(Boolean);
  }
  let isAkaKataRepeated = $derived(!!akaKata && previousKatas(editMatch?.atlet1_id).some((k) => kataKey(k) === kataKey(akaKata)));
  let isAoKataRepeated = $derived(!!aoKata && previousKatas(editMatch?.atlet2_id).some((k) => kataKey(k) === kataKey(aoKata)));

  let toastMessage = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  function showToast(msg: string) {
    toastMessage = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toastMessage = null), 2200);
  }

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
    return json?.status === "success" ? json.data : (json?.data ?? null);
  }

  /** Bouts of the tatami on screen; returns the bagan of the bout its control panel has open */
  async function fetchTatamiMatches(): Promise<number | null> {
    const t = viewTatami;
    let currentBagan: number | null = null;
    try {
      const current = await getJson(`/api/tatami/${t}/current-match${eventQuery}`).catch(() => null);
      const all = await getJson(`/api/tatami/${t}/matches${eventQuery}`).catch(() => null);
      if (t !== viewTatami) return null; // switched tatami meanwhile
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

  /** Bagans, tatamis and timetable of the event (once per event, again on manual refresh) */
  async function fetchEventData(force = false) {
    const ev = eventId;
    if (!ev || (!force && eventDataFor === ev)) return;
    eventDataFor = ev;
    const [bagans, tatamis, timetable] = await Promise.all([
      getJson(`/api/bagans/${ev}`).catch(() => null),
      getJson(`/api/tatamis/${ev}`).catch(() => null),
      getJson(`/api/timetable/${ev}`).catch(() => null),
    ]);
    if (Array.isArray(bagans)) eventBagans = bagans;
    const nums = new Set<number>();
    for (const t of [...(Array.isArray(tatamis) ? tatamis : []), ...(timetable?.tatamis ?? [])]) {
      const n = Number(t?.tatami_number);
      if (n > 0) nums.add(n);
    }
    tatamiNumbers = [...nums];
    const sched: Record<number, SchedItem[]> = {};
    const days = [...(timetable?.days ?? [])].sort((a: any, b: any) => (a.order ?? 0) - (b.order ?? 0));
    for (const day of days) {
      const rows = [...(day.rows ?? [])].sort((a: any, b: any) => (a.order ?? 0) - (b.order ?? 0));
      for (const row of rows) {
        for (const cell of row.cells ?? []) {
          const n = Number(cell.tatami_number);
          const nt = Number(cell.nomor_tanding_id);
          if (!n || !nt) continue;
          (sched[n] ??= []).push({ nomorTandingId: nt, time: String(row.time_label || "").trim() });
        }
      }
    }
    schedule = sched;
  }

  async function showBagan(baganId: number | null) {
    if (!baganId) {
      viewBaganId = null;
      viewBagan = null;
      baganMatches = [];
      return;
    }
    const switching = baganId !== viewBaganId;
    viewBaganId = baganId;
    if (switching) {
      baganMatches = [];
      editMatchId = null;
    }
    isLoading = true;
    try {
      const [b, list] = await Promise.all([getJson(`/api/bagan/${baganId}`), getJson(`/api/detail-bagan/${baganId}`)]);
      if (viewBaganId !== baganId) return;
      viewBagan = b;
      baganMatches = (Array.isArray(list) ? list : []).filter((m: any) => !m.team);
      // Kata sheet open: keep it in step with the reloaded bout
      if (editMatch && Date.now() > ignoreLiveUntil) {
        akaKata = editMatch.kata1 || "";
        aoKata = editMatch.kata2 || "";
      }
    } catch (err) {
      console.warn("Gagal memuat bagan:", err);
    } finally {
      isLoading = false;
    }
  }

  /** Bagan of the bout open in the control panel of the tatami on screen */
  function panelBaganId(): number | null {
    if (!livePanelMatchId) return null;
    if (livePanelBaganId) return livePanelBaganId;
    const m = tatamiMatches.find((x) => x.id === livePanelMatchId) ?? baganMatches.find((x) => x.id === livePanelMatchId);
    return m?.bagan_id ? Number(m.bagan_id) : null;
  }

  async function refreshAll(force = false) {
    const currentBagan = await fetchTatamiMatches();
    await fetchEventData(force);
    const listed = viewBaganId && tatamiBagans.some((b) => b.id === viewBaganId) ? viewBaganId : null;
    const pick = followTatami
      ? panelBaganId() ?? listed ?? currentBagan ?? tatamiBagans[0]?.id ?? null
      : listed ?? currentBagan ?? tatamiBagans[0]?.id ?? null;
    await showBagan(pick);
  }

  // Many bracket updates arrive in a burst (and from two sockets): reload once
  let refreshTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleRefresh() {
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => refreshAll(), 300);
  }

  /** socket that hears the control panel of the tatami on screen */
  function liveSocket(): TatamiSocket | null {
    return isHomeTatami ? socket : viewSocket;
  }

  function setFollow(on: boolean) {
    followTatami = on;
    try {
      localStorage.setItem(FOLLOW_KEY, on ? "1" : "0");
    } catch {}
    if (on) {
      showBagan(panelBaganId() ?? viewBaganId ?? tatamiBagans[0]?.id ?? null);
      liveSocket()?.send({ type: "request_tatami_state", tatami: viewTatami });
    }
  }

  function switchTatami(t: number) {
    if (t === viewTatami) return;
    viewTatami = t;
    editMatchId = null;
    isPickerOpen = false;
    tatamiMatches = [];
    livePanelMatchId = null;
    livePanelBaganId = null;
    viewBaganId = null;
    viewBagan = null;
    baganMatches = [];
    viewSocket?.close();
    viewSocket = null;
    if (t !== tatamiNumber) {
      viewSocket = connectTatamiSocket(
        "lo-kata",
        t,
        onSocketMessage,
        (sock) => sock.send({ type: "request_tatami_state", tatami: t }),
        undefined,
        { staleMs: 15000, onStale: (sock) => sock.send({ type: "request_tatami_state", tatami: t }) }
      );
    } else {
      socket?.send({ type: "request_tatami_state", tatami: t });
    }
    refreshAll();
  }

  // ---- Sheet of the tapped bout ----
  function openSheet(m: any) {
    if (!m || (!m.atlet1_id && !m.atlet2_id)) return;
    editMatchId = m.id;
    akaKata = m.kata1 || "";
    aoKata = m.kata2 || "";
    sendStatus = "idle";
  }
  function closeSheet() {
    editMatchId = null;
    isPickerOpen = false;
  }

  function openPicker(target: "aka" | "ao") {
    pickerTarget = target;
    searchQuery = "";
    isPickerOpen = true;
  }
  function selectKata(kata: KataItem) {
    if (pickerTarget === "aka") akaKata = kata.label;
    else aoKata = kata.label;
    isPickerOpen = false;
    sendKata();
  }
  function clearKata(target: "aka" | "ao") {
    if (target === "aka") akaKata = "";
    else aoKata = "";
    sendKata();
  }

  function patchMatch(id: number, kata1: string | null, kata2: string | null) {
    baganMatches = baganMatches.map((x) => (x.id === id ? { ...x, kata1, kata2 } : x));
    tatamiMatches = tatamiMatches.map((x) => (x.id === id ? { ...x, kata1, kata2 } : x));
  }

  /** Save on the host (it tells the control panel) and send over the live socket */
  async function sendKata() {
    const m = editMatch;
    if (!m) return;
    const kata1 = akaKata || null;
    const kata2 = aoKata || null;
    // Bout of the tatami on screen: its control panel; any other bout: every control panel
    // (each one only takes kata for the bout it has open)
    const onViewTatami = tatamiMatches.some((x) => x.id === m.id) || m.id === livePanelMatchId;
    const target = onViewTatami ? viewTatami : 0;
    sendStatus = "sending";
    ignoreLiveUntil = Date.now() + 2500;
    patchMatch(m.id, kata1, kata2);
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/match/save-kata`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ match_id: m.id, tatami: target, kata1, kata2 }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const msg = { type: "lo_kata_broadcast", tatami: target, match_id: m.id, kata_aka: kata1 ?? "", kata_ao: kata2 ?? "" };
      (liveSocket() ?? socket)?.send(msg);
      // Same machine as the control panel: instant, also without the network
      try {
        const bc = new BroadcastChannel("ambrilindo_live_sync");
        bc.postMessage(msg);
        bc.close();
      } catch {}
      sendStatus = "sent";
      if (typeof navigator !== "undefined" && navigator.vibrate) navigator.vibrate(30);
    } catch (err) {
      console.error("Gagal mengirim kata:", err);
      sendStatus = "error";
      showToast("Gagal mengirim kata. Periksa koneksi lalu kirim ulang.");
    }
  }

  // ---- Realtime ----
  /** Control panel live state: which bout is open, and the kata it shows */
  function handleLiveState(d: any) {
    if (!d || Number(d.tatami) !== viewTatami) return;
    const mId = d.matchId ? Number(d.matchId) : null;
    const changed = mId !== livePanelMatchId;
    livePanelMatchId = mId;
    livePanelBaganId = mId && d.baganId ? Number(d.baganId) : null;
    if (followTatami && mId && changed) {
      const b = panelBaganId();
      if (b && b !== viewBaganId) showBagan(b);
      else if (!b) refreshAll(); // a bout we do not know yet
    }
    if (mId && d.isKata !== false && Date.now() > ignoreLiveUntil) {
      const a = typeof d.aka?.kataName === "string" ? d.aka.kataName : null;
      const o = typeof d.ao?.kataName === "string" ? d.ao.kataName : null;
      if (a !== null && o !== null) {
        const known = baganMatches.find((x) => x.id === mId) ?? tatamiMatches.find((x) => x.id === mId);
        if (known && ((known.kata1 || "") !== a || (known.kata2 || "") !== o)) patchMatch(mId, a || null, o || null);
        if (editMatchId === mId) {
          akaKata = a;
          aoKata = o;
        }
      }
    }
  }

  /** Kata sent by another LO tablet (or this one, echoed) */
  function handleKataBroadcast(d: any) {
    const mId = Number(d?.match_id) || null;
    if (!mId) return;
    const kata1 = d.kata_aka ? String(d.kata_aka) : null;
    const kata2 = d.kata_ao ? String(d.kata_ao) : null;
    patchMatch(mId, kata1, kata2);
    if (editMatchId === mId && Date.now() > ignoreLiveUntil) {
      akaKata = kata1 || "";
      aoKata = kata2 || "";
    }
  }

  function onSocketMessage(msg: any) {
    if (msg?.type === "tatami_live_state") handleLiveState(msg.payload || msg.data);
    else if (msg?.type === "lo_kata_broadcast") handleKataBroadcast(msg.payload || msg);
    else if (msg?.type === "bracket_updated") scheduleRefresh();
  }

  onMount(() => {
    refreshAll();
    socket = connectTatamiSocket(
      "lo-kata",
      tatamiNumber,
      (msg) => {
        linkStatus = "live";
        onSocketMessage(msg);
      },
      (sock) => {
        linkStatus = "live";
        // Whatever changed while offline: reload and ask the control panel for its state
        refreshAll();
        if (isHomeTatami) sock.send({ type: "request_tatami_state", tatami: tatamiNumber });
        else viewSocket?.send({ type: "request_tatami_state", tatami: viewTatami });
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
      viewSocket?.close();
      obs?.disconnect();
      if (toastTimer) clearTimeout(toastTimer);
      if (refreshTimer) clearTimeout(refreshTimer);
    };
  });

  function roundLabel(m: any): string {
    const r = Number(m?.round) || 1;
    return r === 4 ? "Final" : r === 3 ? "Semifinal" : `Babak ${r}`;
  }
</script>

<svelte:head>
  <title>LO / Guide • Tatami {tatamiNumber}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col select-none font-sans overflow-hidden touch-manipulation">
  <!-- Header -->
  <header class="bg-[#14161b] border-b border-white/10 px-4 py-2.5 flex items-center justify-between gap-2 shrink-0 shadow-lg relative z-20">
    <div class="flex items-center gap-2 min-w-0">
      <PortalExitButton title="Keluar dari LO / Guide?" />
      <span class="px-2.5 py-1 rounded-lg bg-purple-500/20 text-purple-300 font-mono font-black text-xs tracking-wider border border-purple-500/30 shrink-0">
        LO / GUIDE • TATAMI {tatamiNumber}
      </span>
      <div
        class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono shrink-0 {linkStatus === 'live'
          ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
          : linkStatus === 'connecting'
            ? 'bg-amber-500/10 text-amber-300 border border-amber-500/30'
            : 'bg-red-500/10 text-red-400 border border-red-500/20'}"
      >
        <span class="w-1.5 h-1.5 rounded-full {linkStatus === 'live' ? 'bg-emerald-400 animate-pulse' : linkStatus === 'connecting' ? 'bg-amber-300 animate-pulse' : 'bg-red-400'}"></span>
        <span class="hidden sm:inline">{linkStatus === "live" ? "ONLINE" : linkStatus === "connecting" ? "MENYAMBUNG ULANG…" : "OFFLINE"}</span>
      </div>
    </div>
    <button
      onclick={() => refreshAll(true)}
      class="p-1.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/10 transition shrink-0"
      title="Segarkan data"
      aria-label="Segarkan data"
    >
      <svg class="w-4 h-4 {isLoading ? 'animate-spin text-purple-400' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
    </button>
  </header>

  <!-- Which tatami (own or a neighbouring one) and which bagan, in the order of its schedule -->
  <div class="bg-[#111317] border-b border-white/[0.06] px-4 py-2 flex flex-col gap-2 shrink-0">
    <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
      <div class="flex items-center gap-1.5 overflow-x-auto scrollbar-thin min-w-0">
        <span class="text-[10px] font-mono font-bold text-zinc-500 tracking-wider shrink-0">TATAMI</span>
        {#each tatamiChoices as t (t)}
          <button
            type="button"
            onclick={() => switchTatami(t)}
            class="shrink-0 min-w-9 h-8 px-2.5 rounded-lg text-xs font-black font-mono transition border {viewTatami === t ? 'bg-purple-500/20 text-white border-purple-400' : 'bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white'}"
            title={t === tatamiNumber ? `Tatami ${t} (tatami Anda)` : `Lihat tatami ${t}`}
          >
            {t}{#if t === tatamiNumber}<span class="ml-1 text-[9px] text-purple-300">●</span>{/if}
          </button>
        {/each}
      </div>
      <button type="button" role="switch" aria-checked={followTatami} onclick={() => setFollow(!followTatami)} class="flex items-center gap-2 text-xs font-bold shrink-0 ml-auto">
        <span class="relative w-9 h-5 rounded-full transition {followTatami ? 'bg-purple-500' : 'bg-white/[0.12]'}">
          <span class="absolute top-0.5 w-4 h-4 rounded-full bg-white transition-all {followTatami ? 'left-[18px]' : 'left-0.5'}"></span>
        </span>
        <span class={followTatami ? "text-white" : "text-zinc-400"}>Ikut control panel</span>
      </button>
    </div>
    {#if !isHomeTatami}
      <div class="text-[11px] font-semibold text-amber-300">Melihat tatami {viewTatami} • tatami Anda {tatamiNumber}</div>
    {/if}
    <div class="flex items-center gap-1.5 overflow-x-auto scrollbar-thin">
      {#each tatamiBagans as b (b.id)}
        <button
          type="button"
          onclick={() => showBagan(b.id)}
          class="shrink-0 px-3 py-1.5 rounded-lg text-xs font-semibold transition border flex items-center gap-1.5 {viewBaganId === b.id ? 'bg-purple-500/20 text-white border-purple-400' : 'bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white'}"
        >
          {#if b.time}<span class="font-mono text-[10px] text-zinc-500">{b.time}</span>{/if}
          <span>{b.nama_bagan}</span>
          {#if b.id === panelBaganId()}<span class="inline-block w-1.5 h-1.5 rounded-full bg-emerald-400"></span>{/if}
        </button>
      {:else}
        <span class="text-xs text-zinc-500">Belum ada bagan di tatami {viewTatami}</span>
      {/each}
    </div>
  </div>

  {#if toastMessage}
    <div class="fixed top-14 left-1/2 -translate-x-1/2 z-[60] bg-red-600 text-white px-4 py-2 rounded-xl text-xs font-bold shadow-2xl">{toastMessage}</div>
  {/if}

  <!-- The bracket: tap a bout (kata: set its kata; kumite: its athletes) -->
  <main bind:this={stageEl} class="flex-1 min-h-0 overflow-auto p-2">
    {#if !viewBagan || !treeMeta}
      <div class="h-full flex items-center justify-center text-xs font-mono text-zinc-500">
        {isLoading ? "Memuat bagan…" : tatamiBagans.length ? "Pilih bagan" : `Belum ada bagan di tatami ${viewTatami}`}
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
            showKata={viewIsKata}
            showPodium={false}
            hideScores
            colWidth={treeFit.colWidth}
            cardScale={treeFit.cardScale}
            {skipRounds}
            highlightMatchId={livePanelMatchId}
            clickHint={viewIsKata ? "Ketuk untuk memilih kata" : "Ketuk untuk melihat atlet"}
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
            <div class="text-xs font-black text-red-300 truncate">{m.atlet1_nama || "-"} {#if viewIsKata}<span class="text-zinc-500 font-mono font-bold">{m.kata1 || ""}</span>{/if}</div>
            <div class="text-xs font-black text-blue-300 truncate">{m.atlet2_nama || "-"} {#if viewIsKata}<span class="text-zinc-500 font-mono font-bold">{m.kata2 || ""}</span>{/if}</div>
          </button>
        {/each}
      </div>
    {/if}
  </main>

  <!-- Legend -->
  <footer class="bg-[#14161b] border-t border-white/10 px-4 py-2 shrink-0 flex items-center gap-4 text-[11px] font-mono text-zinc-400">
    <span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded ring-2 ring-emerald-400"></span>Partai di control panel</span>
    <span>{viewIsKata ? "Ketuk partai untuk memilih kata" : "Ketuk partai untuk melihat atlet"}</span>
  </footer>

  <!-- Sheet of the tapped bout: kata entry (kata) or the athletes (kumite) -->
  {#if editMatch}
    <div class="fixed inset-0 z-40 flex items-end sm:items-center justify-center bg-black/60 backdrop-blur-xs" role="presentation" onclick={closeSheet}>
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
              {editMatch.id === livePanelMatchId ? "Sedang dibuka di control panel" : (viewBagan?.nama_bagan ?? "")}{isHomeTatami ? "" : ` • Tatami ${viewTatami}`}
            </div>
          </div>
          <button onclick={closeSheet} aria-label="Tutup" class="p-1.5 rounded-lg hover:bg-white/10 text-zinc-400 hover:text-white transition">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
          </button>
        </div>

        <div class="p-3 flex flex-col gap-3 overflow-y-auto">
          {#each [["aka", "AKA"], ["ao", "AO"]] as const as [side, label] (side)}
            {@const kata = side === "aka" ? akaKata : aoKata}
            {@const repeated = side === "aka" ? isAkaKataRepeated : isAoKataRepeated}
            {@const name = side === "aka" ? editMatch.atlet1_nama : editMatch.atlet2_nama}
            <div class="bg-[#16181f] border-2 rounded-2xl p-3 flex flex-col gap-2.5 {side === 'aka' ? 'border-red-600/70' : 'border-blue-600/70'}">
              <div class="min-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  <span class="px-2 py-0.5 rounded text-white font-mono font-black text-[10px] tracking-wider shrink-0 {side === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">{label}</span>
                  <h2 class="text-base font-black text-white truncate">{name || "BYE"}</h2>
                </div>
                <p class="text-xs text-zinc-400 mt-0.5 font-medium truncate">{(side === "aka" ? editMatch.atlet1_kontingen : editMatch.atlet2_kontingen) || "-"}</p>
              </div>
              {#if viewIsKata}
              <div class="flex items-center justify-between gap-2 bg-[#101216] border border-white/10 rounded-xl p-2.5">
                <span class="text-sm font-black font-mono truncate {kata ? (side === 'aka' ? 'text-red-400' : 'text-blue-400') : 'text-zinc-500'}">{kata || "Belum dipilih"}</span>
                <div class="flex items-center gap-1.5 shrink-0">
                  {#if kata}
                    <button type="button" onclick={() => clearKata(side)} class="h-9 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/10 font-semibold text-xs rounded-xl transition">
                      Hapus
                    </button>
                  {/if}
                  <button
                    type="button"
                    onclick={() => openPicker(side)}
                    disabled={!name}
                    class="h-9 px-3.5 active:scale-95 text-white font-bold text-xs rounded-xl shadow transition disabled:opacity-40 {side === 'aka' ? 'bg-red-600 hover:bg-red-500' : 'bg-blue-600 hover:bg-blue-500'}"
                  >
                    Pilih Kata
                  </button>
                </div>
              </div>
              {#if repeated}
                <div class="px-3 py-2 rounded-xl bg-amber-500/15 border border-amber-500/40 text-amber-300 text-xs font-semibold flex items-center gap-2">
                  <span>⚠️</span>
                  <span>Kata "{kata}" sudah dimainkan atlet ini di partai lain bagan ini</span>
                </div>
              {/if}
              {/if}
            </div>
          {/each}
        </div>

        <div class="px-4 py-3 border-t border-white/10 flex items-center gap-3">
          <div class="flex-1 min-w-0 text-xs font-bold font-mono flex items-center gap-2">
            {#if !viewIsKata}
              <span class="text-zinc-500 truncate">{(editMatch.atlet1_nama && editMatch.atlet2_nama) ? "Panggil kedua atlet ke tatami" : "Satu sudut BYE"}</span>
            {:else if sendStatus === "sending"}
              <span class="w-2 h-2 rounded-full bg-amber-300 animate-pulse shrink-0"></span><span class="text-amber-300 truncate">Mengirim…</span>
            {:else if sendStatus === "sent"}
              <span class="w-2 h-2 rounded-full bg-emerald-400 shrink-0"></span>
              <span class="text-emerald-300 truncate">{editMatch.id === livePanelMatchId ? "Terkirim ke control panel & papan skor" : "Tersimpan, muncul saat partai dibuka"}</span>
            {:else if sendStatus === "error"}
              <span class="w-2 h-2 rounded-full bg-red-400 shrink-0"></span><span class="text-red-300 truncate">Gagal terkirim</span>
            {:else}
              <span class="text-zinc-500 truncate">Kata langsung terkirim saat dipilih</span>
            {/if}
          </div>
          {#if sendStatus === "error"}
            <button onclick={sendKata} class="h-10 px-4 shrink-0 bg-red-600 hover:bg-red-500 text-white font-black text-xs rounded-xl shadow-xl">KIRIM ULANG</button>
          {/if}
          <button onclick={closeSheet} class="h-10 px-4 shrink-0 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md">Selesai</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Kata picker -->
  {#if isPickerOpen}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-5 shadow-2xl max-w-md w-full flex flex-col max-h-[85vh]">
        <div class="flex items-center justify-between pb-3 border-b border-white/10">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black uppercase text-white {pickerTarget === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">
              {pickerTarget === "aka" ? "SUDUT MERAH (AKA)" : "SUDUT BIRU (AO)"}
            </span>
            <h3 class="text-sm font-black text-white">Daftar Kata WKF/FORKI</h3>
          </div>
          <button onclick={() => (isPickerOpen = false)} aria-label="Tutup daftar kata" class="p-1 rounded-lg hover:bg-white/10 text-zinc-400 hover:text-white transition">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
          </button>
        </div>
        <div class="py-3">
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Cari kata (misal: Anan, Chatanyara, Suparinpei)"
            class="w-full bg-[#111317] border border-white/10 rounded-xl px-3 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-purple-400"
          />
        </div>
        <div class="flex-1 min-h-0 overflow-y-auto space-y-1 pr-1">
          {#each filteredKataList as item (item.code)}
            <button onclick={() => selectKata(item)} class="w-full px-3 py-2.5 rounded-xl text-left transition flex items-center justify-between hover:bg-white/[0.06] text-zinc-200">
              <div>
                <span class="font-bold text-xs text-white block">{item.name}</span>
                <span class="text-[10px] font-mono text-zinc-400 block">{item.code}</span>
              </div>
              <svg class="w-4 h-4 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" /></svg>
            </button>
          {:else}
            <div class="p-6 text-center text-zinc-500 text-xs">Tidak ada kata yang cocok dengan "{searchQuery}"</div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>
