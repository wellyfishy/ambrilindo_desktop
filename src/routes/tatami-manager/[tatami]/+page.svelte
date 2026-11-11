<script lang="ts">
  // Tatami Manager: the brackets of a tatami (kata and kumite), like LO Kata, to assign the
  // referee and judges of each bout. Every bout uses its tatami's team unless a seat is
  // changed for that bout; the team of any tatami can be set here too.
  // Realtime: follows the bout open in the control panel (live state); the shared socket
  // reconnects by itself and every reconnect reloads the brackets and asks for the state.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { apiCall, getHostBaseUrl } from "$lib/api_client";
  import type { BoutPanel, TatamiItem, TatamiPanelOverview, WasitItem } from "$lib/types";
  import {
    KATA_JUDGE_OPTIONS,
    KUMITE_JUDGE_OPTIONS,
    TEAM_SEATS,
    boutJudgeCount,
    boutSeats,
    conflictReason,
    effectivePanel,
    isKataCategory,
    type SeatHolder,
  } from "$lib/refereeSeats";
  import RefereeAvatar from "$lib/components/RefereeAvatar.svelte";
  import RefereePicker from "$lib/components/RefereePicker.svelte";
  import { computePodium, deriveBracketMeta, TREE_COL_GAP, TREE_COL_WIDTH, TREE_HEIGHT, treeWidthFor } from "$lib/bracket_layout";
  import BracketTree from "$lib/components/BracketTree.svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));
  let eventFromUrl = $derived(Number(page.url.searchParams.get("event")) || null);
  let eventFromMatch = $state<number | null>(null);
  let eventId = $derived(eventFromUrl ?? eventFromMatch);
  let eventQuery = $derived(eventId ? `?event=${eventId}` : "");

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
    // the bagan running in the control panel, also when its bouts are not assigned to this tatami
    if (livePanelBaganId && !seen.has(livePanelBaganId)) {
      seen.set(livePanelBaganId, viewBagan?.id === livePanelBaganId ? viewBagan.nama_bagan : `Bagan ${livePanelBaganId}`);
    }
    return [...seen].map(([id, nama_bagan]) => ({ id, nama_bagan }));
  });
  let eventBagans = $state<any[]>([]);

  // Bracket on screen
  let viewBaganId = $state<number | null>(null);
  let viewBagan = $state<any | null>(null);
  let baganMatches = $state<any[]>([]);
  let isLoading = $state(false);

  let livePanelMatchId = $state<number | null>(null);
  /** bagan of the bout open in the control panel, as sent in its live state */
  let livePanelBaganId = $state<number | null>(null);

  // Bout being viewed (assignment sheet)
  let editMatchId = $state<number | null>(null);
  let editMatch = $derived(editMatchId ? baganMatches.find((m) => m.id === editMatchId) ?? null : null);

  // Kata or kumite: decides the officials of a bout
  let isKataBagan = $derived(!!viewBagan && (String(viewBagan.tipe_tanding) === "1" || /KATA/i.test(viewBagan.nama_bagan || "")));

  // ---- Referees ----
  let wasits = $state<WasitItem[]>([]);
  let tatamiList = $state<TatamiItem[]>([]);
  /** kata judges of this tatami (3 / 5 / 7), from the control panel's live state */
  let kataJudges = $state(5);

  // Bout panel (sheet)
  let boutPanel = $state<BoutPanel | null>(null);
  let boutError = $state("");
  let boutSaving = $state(false);
  let boutIsKata = $derived(boutPanel ? isKataCategory(boutPanel.category) : isKataBagan);
  /** judges of the open bout: kumite 2/4, kata 3/5/7 */
  let judgeCount = $derived(
    boutJudgeCount(boutPanel ?? { category: isKataBagan ? "KATA" : "KUMITE", judges: null, tatami_kumite_judges: null, tatami_kata_judges: null }, kataJudges)
  );
  let seats = $derived(boutSeats(boutPanel?.category ?? (isKataBagan ? "KATA" : "KUMITE"), judgeCount));
  /** shown after a pick: who was placed, and a ⚠ when they conflict with AKA / AO */
  let boutNotice = $state<{ text: string; warn: boolean } | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | null = null;
  function showNotice(text: string, warn: boolean) {
    boutNotice = { text, warn };
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (boutNotice = null), warn ? 8000 : 3000);
  }

  // Landscape tablet / phone: the bout sheet is laid out side by side
  let wideSheet = $state(false);
  let effective = $derived(boutPanel ? effectivePanel(boutPanel, seats) : null);
  let filledCount = $derived(effective ? Object.values(effective.seats).filter(Boolean).length : 0);

  // Picker (bout seat or team seat)
  let pickFor = $state<{ kind: "bout" | "team"; seat: string; label: string } | null>(null);

  // Team sheet (any tatami)
  let teamOpen = $state(false);
  let teamTatamiNumber = $state<number | null>(null);
  let teamOverview = $state<TatamiPanelOverview | null>(null);
  let teamTatamiId = $derived(tatamiList.find((t) => t.tatami_number === (teamTatamiNumber ?? tatamiNumber))?.id ?? null);

  function errText(e: any): string {
    return typeof e === "string" ? e : e?.message || String(e);
  }

  async function loadWasits() {
    if (!eventId) return;
    try {
      wasits = await apiCall<WasitItem[]>("get_wasits", { eventId });
    } catch (e) {
      console.warn("Gagal memuat wasit:", e);
    }
  }

  async function loadTatamis() {
    if (!eventId) return;
    try {
      tatamiList = (await apiCall<TatamiItem[]>("get_tatamis", { eventId })).sort((a, b) => (a.tatami_number || 0) - (b.tatami_number || 0));
    } catch (e) {
      console.warn("Gagal memuat tatami:", e);
    }
  }

  async function loadBoutPanel() {
    const id = editMatchId;
    if (!id) {
      boutPanel = null;
      return;
    }
    try {
      const p = await apiCall<BoutPanel>("get_bout_panel", { matchId: id, tatamiNumber });
      if (editMatchId === id) {
        boutPanel = p;
        boutError = "";
      }
    } catch (e) {
      boutError = `Panel wasit tidak bisa dimuat: ${errText(e)}`;
    }
  }

  async function loadTeam() {
    const id = teamTatamiId;
    if (!id || !eventId) {
      teamOverview = null;
      return;
    }
    try {
      teamOverview = await apiCall<TatamiPanelOverview>("get_tatami_panel_overview", { eventId, tatamiId: id });
    } catch (e) {
      console.warn("Gagal memuat tim tatami:", e);
    }
  }

  function refreshReferees() {
    loadWasits();
    if (editMatchId) loadBoutPanel();
    if (teamOpen) loadTeam();
  }

  /** Save this bout's own panel (seat key → referee id) */
  async function saveBout(next: Record<string, number>, judges = judgeCount) {
    if (!editMatchId) return;
    boutSaving = true;
    try {
      await apiCall("save_bout_panel", {
        matchId: editMatchId,
        tatamiNumber,
        judges,
        seats: Object.entries(next).map(([posisi, wasit_id]) => ({ posisi, wasit_id })),
      });
      await loadBoutPanel();
    } catch (e) {
      boutError = `Gagal menyimpan: ${errText(e)}`;
    } finally {
      boutSaving = false;
    }
  }

  function currentSeatIds(): Record<string, number> {
    const next: Record<string, number> = {};
    for (const [k, h] of Object.entries(effective?.seats ?? {})) if (h) next[k] = h.wasit_id;
    return next;
  }

  /** One seat changed (a referee moved here leaves their old seat) */
  async function setBoutSeat(seatKey: string, holder: SeatHolder | null) {
    if (!effective || !editMatchId) return;
    const next = currentSeatIds();
    if (holder) {
      for (const k of Object.keys(next)) if (next[k] === holder.wasit_id) delete next[k];
      next[seatKey] = holder.wasit_id;
      const conflict = conflictReason(holder, boutPanel);
      const label = seats.find((s) => s.key === seatKey)?.label ?? seatKey;
      showNotice(conflict ? `⚠ ${holder.nama_wasit} (${label}): ${conflict}` : `${holder.nama_wasit} → ${label}`, !!conflict);
    } else {
      delete next[seatKey];
    }
    await saveBout(next);
  }

  /** Every seat of this bout empty (the tatami team no longer applies to it) */
  async function clearBout() {
    await saveBout({});
    showNotice("Semua kursi partai ini dikosongkan", false);
  }

  async function resetBout() {
    if (!editMatchId) return;
    boutSaving = true;
    try {
      await apiCall("reset_bout_panel", { matchId: editMatchId });
      await loadBoutPanel();
    } catch (e) {
      boutError = `Gagal: ${errText(e)}`;
    } finally {
      boutSaving = false;
    }
  }

  /** Kumite 2/4, kata 3/5/7 judges for this bout; also the tatami's default for the next bouts */
  async function setJudges(n: number) {
    if (!editMatchId || n === judgeCount) return;
    boutSaving = true;
    try {
      await apiCall("set_bout_judges", { matchId: editMatchId, tatamiNumber, judges: n, kata: boutIsKata });
      // a bout with its own panel drops the seats that no longer exist
      if (effective?.fromBout) {
        const keep = new Set(boutSeats(boutPanel?.category, n).map((s) => s.key));
        const next = Object.fromEntries(Object.entries(currentSeatIds()).filter(([k]) => keep.has(k)));
        await saveBout(next, n);
      } else {
        await loadBoutPanel();
      }
    } catch (e) {
      boutError = `Gagal: ${errText(e)}`;
    } finally {
      boutSaving = false;
    }
  }

  /** "Pakai wasit partai sebelumnya": the officials (and number of judges) of the previous bout */
  async function copyPreviousBout() {
    const prevId = boutPanel?.prev_match_id;
    if (!prevId || !boutPanel) return;
    boutSaving = true;
    try {
      const prev = await apiCall<BoutPanel>("get_bout_panel", { matchId: prevId, tatamiNumber });
      const sameKind = isKataCategory(prev.category) === boutIsKata;
      const prevJudges = boutJudgeCount(prev, kataJudges);
      const prevSeats = boutSeats(prev.category, prevJudges);
      const prevEff = effectivePanel(prev, prevSeats);
      const judges = sameKind ? prevJudges : judgeCount;
      const target = boutSeats(boutPanel.category, judges);
      const next: Record<string, number> = {};
      if (sameKind) {
        for (const s of target) {
          const h = prevEff.seats[s.key];
          if (h) next[s.key] = h.wasit_id;
        }
      } else {
        // kata ↔ kumite: the same people in seat order
        const holders = prevSeats.map((s) => prevEff.seats[s.key]).filter((h): h is SeatHolder => !!h);
        target.forEach((s, i) => {
          if (holders[i]) next[s.key] = holders[i].wasit_id;
        });
      }
      if (judges !== judgeCount) {
        await apiCall("set_bout_judges", { matchId: editMatchId, tatamiNumber, judges, kata: boutIsKata });
      }
      await saveBout(next, judges);
      showNotice(`Wasit ${boutPanel.prev_label ?? "partai sebelumnya"} dipakai`, false);
    } catch (e) {
      boutError = `Gagal: ${errText(e)}`;
    } finally {
      boutSaving = false;
    }
  }

  async function setTeamSeat(seatKey: string, w: WasitItem | null) {
    if (!teamTatamiId || !eventId) return;
    try {
      if (w) {
        await apiCall("assign_wasit_tatami", { input: { event_id: eventId, tatami_id: teamTatamiId, wasit_id: w.id, posisi: seatKey } });
      } else {
        const a = teamOverview?.assignments.find((x) => x.posisi === seatKey);
        if (a) await apiCall("remove_wasit_tatami", { assignmentId: a.id });
      }
      await Promise.all([loadTeam(), loadWasits()]);
      if (editMatchId) loadBoutPanel();
    } catch (e) {
      alert(`Gagal menyimpan tim tatami: ${errText(e)}`);
    }
  }

  function openTeam() {
    teamTatamiNumber = teamTatamiNumber ?? tatamiNumber;
    teamOpen = true;
    loadTatamis();
    loadTeam();
  }

  // load referee data once the event is known; reload the bout panel when another bout opens
  $effect(() => {
    if (eventId) {
      loadWasits();
      loadTatamis();
    }
  });
  $effect(() => {
    void editMatchId;
    boutPanel = null;
    boutError = "";
    boutNotice = null;
    loadBoutPanel();
  });
  $effect(() => {
    void teamTatamiId;
    if (teamOpen) loadTeam();
  });

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
      // the event of this tatami (URL has none): the bagan running in the control panel decides
      if (!eventFromUrl && b?.event_id && baganId === livePanelBaganId && Number(b.event_id) !== eventFromMatch) {
        eventFromMatch = Number(b.event_id);
        fetchTatamiMatches(); // this event's bouts on this tatami only
      }
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
    // bouts of a bagan not assigned to this tatami: the control panel says which bagan
    return m?.bagan_id ? Number(m.bagan_id) : livePanelBaganId;
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
    if ([3, 5, 7].includes(Number(d.judgeCount))) kataJudges = Number(d.judgeCount);
    const mId = d.matchId ? Number(d.matchId) : null;
    const changed = mId !== livePanelMatchId;
    livePanelMatchId = mId;
    livePanelBaganId = mId && d.baganId ? Number(d.baganId) : null;
    if (followTatami && mId && changed) {
      const b = panelBaganId();
      if (b && b !== viewBaganId) showBagan(b);
      else if (!b) refreshAll(); // a bout we do not know yet
    }
  }

  // ---- Link status ----
  // WebSocket first. If it cannot connect (some tablets / Wi-Fi setups block it) the page
  // keeps working over plain HTTP: it asks the host for the live state every 3 s.
  let httpOk = $state(false);
  let wsInfo = $state("belum tersambung");
  let showLinkInfo = $state(false);
  let viaHttp = $derived(linkStatus !== "live" && httpOk);
  let pollCount = 0;

  async function pollOverHttp() {
    if (linkStatus === "live") return;
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/tatami/${tatamiNumber}/live-state`, { cache: "no-store" });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = await res.json();
      httpOk = true;
      if (json?.data) handleLiveState(json.data);
      // no referee_updated pushes without the socket: reload what is open now and then
      if (++pollCount % 3 === 0) {
        if (editMatchId) loadBoutPanel();
        if (teamOpen) loadTeam();
      }
    } catch {
      httpOk = false;
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
        else if (msg?.type === "referee_updated") refreshReferees();
      },
      (sock) => {
        linkStatus = "live";
        wsInfo = "tersambung";
        refreshAll();
        refreshReferees();
        sock.send({ type: "request_tatami_state", tatami: tatamiNumber });
      },
      (ev) => {
        linkStatus = typeof navigator !== "undefined" && navigator.onLine === false ? "offline" : "connecting";
        wsInfo = ev ? `terputus (kode ${ev.code}${ev.reason ? `, ${ev.reason}` : ""})` : "tidak ada balasan, menyambung ulang";
      },
      { staleMs: 15000, onStale: (sock) => sock.send({ type: "request_tatami_state", tatami: tatamiNumber }) }
    );
    const poll = setInterval(pollOverHttp, 3000);
    const mq = window.matchMedia("(orientation: landscape) and (min-width: 700px)");
    const onMq = () => (wideSheet = mq.matches);
    onMq();
    mq.addEventListener("change", onMq);
    let obs: ResizeObserver | null = null;
    if (stageEl) {
      obs = new ResizeObserver(([e]) => (stageW = e.contentRect.width));
      obs.observe(stageEl);
    }
    return () => {
      socket?.close();
      clearInterval(poll);
      mq.removeEventListener("change", onMq);
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
      <button
        type="button"
        onclick={() => (showLinkInfo = !showLinkInfo)}
        title="Status koneksi"
        class="relative flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono shrink-0 {linkStatus === 'live'
          ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
          : viaHttp
            ? 'bg-emerald-500/10 text-emerald-300 border border-amber-400/40'
            : linkStatus === 'connecting'
              ? 'bg-amber-500/10 text-amber-300 border border-amber-500/30'
              : 'bg-red-500/10 text-red-400 border border-red-500/20'}"
      >
        <span class="w-1.5 h-1.5 rounded-full {linkStatus === 'live' || viaHttp ? 'bg-emerald-400 animate-pulse' : linkStatus === 'connecting' ? 'bg-amber-300 animate-pulse' : 'bg-red-400'}"></span>
        <span>{linkStatus === "live" ? "ONLINE" : viaHttp ? "ONLINE · HTTP" : linkStatus === "connecting" ? "MENYAMBUNG ULANG…" : "OFFLINE"}</span>
      </button>
      {#if showLinkInfo}
        <div class="absolute left-3 top-12 z-50 w-80 max-w-[calc(100vw-24px)] rounded-xl border border-white/10 bg-[#181a20] p-3 shadow-2xl text-[11px] font-mono text-zinc-300 space-y-1">
          <div>Host: <span class="text-white break-all">{getHostBaseUrl()}</span></div>
          <div>WebSocket: <span class="text-white">{wsInfo}</span></div>
          <div>HTTP: <span class="text-white">{httpOk ? "OK (cadangan aktif tiap 3 detik)" : linkStatus === "live" ? "tidak dipakai" : "gagal"}</span></div>
        </div>
      {/if}
    </div>
    <button
      type="button"
      onclick={openTeam}
      class="ml-auto h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl shrink-0"
    >
      Tim Tatami
    </button>
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
    <div
      class="fixed inset-0 z-40 flex justify-center bg-black/60 backdrop-blur-xs {wideSheet ? 'items-center p-3' : 'items-end sm:items-center'}"
      role="presentation"
      onclick={() => (editMatchId = null)}
    >
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="w-full bg-[#181a20] border border-white/10 shadow-2xl flex flex-col {wideSheet ? 'max-w-5xl rounded-2xl max-h-[96vh]' : 'sm:max-w-xl rounded-t-2xl sm:rounded-2xl max-h-[92vh]'}"
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

        {#snippet athletes()}
          <div class="grid grid-cols-2 gap-2">
            {#each [["ao", "AO", editMatch.atlet2_nama, editMatch.atlet2_kontingen], ["aka", "AKA", editMatch.atlet1_nama, editMatch.atlet1_kontingen]] as [side, label, name, club] (side)}
              <div class="rounded-xl border p-2.5 min-w-0 {side === 'aka' ? 'border-red-600/60 bg-red-950/20' : 'border-blue-600/60 bg-blue-950/20'}">
                <span class="px-1.5 py-0.5 rounded text-white font-mono font-black text-[10px] {side === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">{label}</span>
                <div class="text-sm font-black text-white truncate mt-1">{name || "BYE"}</div>
                <div class="text-[11px] text-zinc-400 truncate">{club || "-"}</div>
              </div>
            {/each}
          </div>
        {/snippet}

        {#snippet controls()}
          <div class="flex flex-col gap-2">
            <!-- number of judges: this bout, and the tatami's default for the next ones -->
            <div class="flex items-center gap-2">
              <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400 shrink-0">Jumlah juri</span>
              <div class="flex gap-1">
                {#each boutIsKata ? KATA_JUDGE_OPTIONS : KUMITE_JUDGE_OPTIONS as n (n)}
                  <button
                    type="button"
                    disabled={boutSaving || !boutPanel}
                    onclick={() => setJudges(n)}
                    class="h-9 min-w-11 px-3 rounded-lg text-sm font-black font-mono border transition {judgeCount === n
                      ? 'bg-white text-black border-white'
                      : 'bg-white/[0.04] text-zinc-300 border-white/10 hover:text-white hover:border-white/30'}"
                  >
                    {n}
                  </button>
                {/each}
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <button
                type="button"
                disabled={boutSaving || !boutPanel?.prev_match_id}
                onclick={copyPreviousBout}
                class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl disabled:opacity-40"
                title={boutPanel?.prev_label ?? "Tidak ada partai sebelumnya"}
              >
                Pakai wasit partai sebelumnya{boutPanel?.prev_label ? ` (${boutPanel.prev_label})` : ""}
              </button>
              <button
                type="button"
                disabled={boutSaving || !boutPanel || filledCount === 0}
                onclick={clearBout}
                class="h-9 px-3.5 bg-white/[0.04] text-red-300 border border-white/[0.1] font-semibold text-xs rounded-xl disabled:opacity-40"
              >
                Kosongkan semua
              </button>
              {#if effective?.fromBout}
                <button
                  type="button"
                  disabled={boutSaving}
                  onclick={resetBout}
                  class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
                >
                  Kembali ke tim tatami
                </button>
              {/if}
            </div>
            {#if boutNotice}
              <div class="rounded-xl px-3 py-2 text-xs font-semibold {boutNotice.warn ? 'bg-amber-500/10 border border-amber-500/30 text-amber-200' : 'bg-emerald-500/10 border border-emerald-500/25 text-emerald-200'}">
                {boutNotice.text}
              </div>
            {/if}
          </div>
        {/snippet}

        {#snippet officials()}
          <div class="rounded-2xl border border-white/10 bg-[#13151b] p-3 flex flex-col gap-2 min-h-0">
            <div class="flex items-center justify-between gap-2">
              <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400">
                {boutIsKata ? `Panel Juri Kata (${judgeCount})` : `Wasit & Juri Kumite (${judgeCount} juri)`} · {filledCount}/{seats.length}
              </span>
              {#if effective?.fromBout}
                <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/30">KHUSUS PARTAI INI</span>
              {:else if boutPanel}
                <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/10">TIM TATAMI {tatamiNumber}</span>
              {/if}
            </div>
            {#if boutError}
              <div class="text-xs text-red-300">{boutError}</div>
            {/if}
            {#if !boutPanel && !boutError}
              <div class="text-xs font-mono text-zinc-500 py-2">Memuat panel…</div>
            {:else if effective}
              <div class="grid gap-2 {wideSheet ? 'grid-cols-2' : 'grid-cols-1'}">
                {#each seats as seat (seat.key)}
                  {@const h = effective.seats[seat.key]}
                  {@const conflict = h ? conflictReason(h, boutPanel) : null}
                  <button
                    type="button"
                    disabled={boutSaving}
                    onclick={() => (pickFor = { kind: "bout", seat: seat.key, label: seat.label })}
                    class="text-left flex items-center gap-3 rounded-xl border px-3 py-2 transition {conflict
                      ? 'bg-amber-500/[0.05] border-amber-500/30'
                      : 'bg-[#101216] border-white/10 hover:border-white/30'}"
                  >
                    <RefereeAvatar foto={h?.foto} nama={h?.nama_wasit ?? seat.short} size={44} />
                    <div class="min-w-0 flex-1">
                      <div class="text-[10px] font-mono font-bold uppercase text-zinc-500">{seat.label}</div>
                      {#if h}
                        <div class="text-sm font-black text-white truncate">{h.nama_wasit}</div>
                        <div class="text-[11px] text-zinc-400 truncate">{h.nama_perguruan || "PB / Umum"}{h.kab_kota ? ` • ${h.kab_kota}` : ""}</div>
                        {#if conflict}<div class="text-[10px] font-mono font-bold text-amber-300 leading-tight">⚠ {conflict}</div>{/if}
                      {:else}
                        <div class="text-sm font-semibold text-zinc-500">Belum ditentukan</div>
                      {/if}
                    </div>
                    <span class="text-xs font-semibold text-zinc-300 shrink-0">{h ? "Ganti" : "Pilih"}</span>
                  </button>
                {/each}
              </div>
              {#if !effective.fromBout && boutPanel && boutPanel.team.length === 0}
                <p class="text-[11px] text-zinc-500 px-1">Tim Tatami {tatamiNumber} belum diisi. Atur lewat tombol "Tim Tatami", atau pilih per kursi untuk partai ini saja.</p>
              {/if}
            {/if}
          </div>
        {/snippet}

        <!-- landscape: athletes and actions on the left, the seats on the right -->
        {#if wideSheet}
          <div class="p-3 grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-3 overflow-y-auto min-h-0">
            <div class="flex flex-col gap-3">
              {@render athletes()}
              {@render controls()}
            </div>
            {@render officials()}
          </div>
        {:else}
          <div class="p-3 flex flex-col gap-3 overflow-y-auto">
            {@render athletes()}
            {@render controls()}
            {@render officials()}
          </div>
        {/if}

        <div class="px-4 py-3 border-t border-white/10 flex items-center justify-end">
          <button onclick={() => (editMatchId = null)} class="h-10 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md">Tutup</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Team of a tatami (any tatami) -->
  {#if teamOpen}
    <div class="fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-black/60 backdrop-blur-xs" role="presentation" onclick={() => (teamOpen = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="w-full sm:max-w-2xl bg-[#181a20] border border-white/10 rounded-t-2xl sm:rounded-2xl shadow-2xl flex flex-col max-h-[92vh]"
        role="dialog"
        aria-modal="true"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
      >
        <div class="px-4 py-3 border-b border-white/10 flex items-center gap-2">
          <div class="text-sm font-black text-white uppercase flex-1">Tim Tatami</div>
          <button onclick={() => (teamOpen = false)} aria-label="Tutup" class="p-1.5 rounded-lg hover:bg-white/10 text-zinc-400 hover:text-white transition">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
          </button>
        </div>
        <div class="px-4 py-2 border-b border-white/[0.06] flex gap-1.5 overflow-x-auto scrollbar-thin">
          {#each tatamiList as t (t.id)}
            <button
              type="button"
              onclick={() => (teamTatamiNumber = t.tatami_number)}
              class="shrink-0 px-3 py-1.5 rounded-lg text-xs font-semibold transition border {(teamTatamiNumber ?? tatamiNumber) === t.tatami_number
                ? 'bg-white/[0.1] text-white border-white/40'
                : 'bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white'}"
            >
              Tatami {t.tatami_number}
            </button>
          {/each}
        </div>
        <div class="p-3 grid grid-cols-1 sm:grid-cols-2 gap-2 overflow-y-auto">
          {#each TEAM_SEATS as seat (seat.key)}
            {@const a = teamOverview?.assignments.find((x) => x.posisi === seat.key)}
            <button
              type="button"
              onclick={() => (pickFor = { kind: "team", seat: seat.key, label: seat.label })}
              class="text-left flex items-center gap-3 rounded-xl border px-3 py-2 bg-[#101216] border-white/10 hover:border-white/30 transition"
            >
              <RefereeAvatar foto={a?.foto} nama={a?.nama_wasit ?? seat.short} size={44} />
              <div class="min-w-0 flex-1">
                <div class="text-[10px] font-mono font-bold uppercase text-zinc-500">{seat.label}</div>
                {#if a}
                  <div class="text-sm font-black text-white truncate">{a.nama_wasit}</div>
                  <div class="text-[11px] text-zinc-400 truncate">{a.nama_perguruan || "PB / Umum"}{a.kab_kota ? ` • ${a.kab_kota}` : ""}</div>
                {:else}
                  <div class="text-sm font-semibold text-zinc-500">Kosong</div>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  {#if pickFor}
    {@const isBout = pickFor.kind === "bout"}
    {@const currentId = isBout
      ? (effective?.seats[pickFor.seat]?.wasit_id ?? null)
      : (teamOverview?.assignments.find((x) => x.posisi === pickFor?.seat)?.wasit_id ?? null)}
    <RefereePicker
      title={isBout ? `Partai #${editMatch?.urutan || ""} · ${pickFor.label}` : `Tatami ${teamTatamiNumber ?? tatamiNumber} · ${pickFor.label}`}
      {wasits}
      tatamiId={isBout ? (boutPanel?.tatami_id ?? null) : teamTatamiId}
      panel={isBout ? boutPanel : null}
      takenIds={new Set(
        isBout
          ? Object.values(effective?.seats ?? {}).filter((h) => !!h).map((h) => h!.wasit_id)
          : (teamOverview?.assignments ?? []).map((x) => x.wasit_id)
      )}
      {currentId}
      onPick={(w) => {
        const seat = pickFor!.seat;
        const kind = pickFor!.kind;
        pickFor = null;
        if (kind === "bout") setBoutSeat(seat, { wasit_id: w.id, nama_wasit: w.nama_wasit, perguruan_id: w.perguruan_id, nama_perguruan: w.nama_perguruan, kab_kota: w.kab_kota, lisensi: w.lisensi, foto: w.foto ?? null });
        else setTeamSeat(seat, w);
      }}
      onClear={() => {
        const seat = pickFor!.seat;
        const kind = pickFor!.kind;
        pickFor = null;
        if (kind === "bout") setBoutSeat(seat, null);
        else setTeamSeat(seat, null);
      }}
      onClose={() => (pickFor = null)}
    />
  {/if}
</div>
