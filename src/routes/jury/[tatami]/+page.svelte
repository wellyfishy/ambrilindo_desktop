<!--
  Jury tablet (browser, via http://<host-ip>:<port>/jury/<tatami>): one judge scores the kata
  of AKA and AO, the scores go to the tatami's control panel.

  The control panel gives this judge's flag to the higher score; equal scores give no flag,
  so a tie asks for confirmation. A submitted score shows "diterima meja" only once the
  control panel's live state carries it back.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { getHostBaseUrl } from "$lib/api_client";

  type Side = "aka" | "ao";
  /** One side's score: integer part 5..10 plus a tenth, or 0 (DQ / kiken); null = not entered. */
  interface Entry {
    whole: number | null;
    tenth: number;
  }

  // Seats follow the number of kata judges set on the control panel (3 / 5 / 7)
  // Until the control panel's setting is known (panel not open yet), every seat up to the
  // maximum of 7 can be chosen; the last known count is remembered per tatami.
  const MAX_JUDGES = 7;
  let judgeCount = $state<number>(MAX_JUDGES);
  let JUDGE_SEATS = $derived(Array.from({ length: judgeCount }, (_, i) => i + 1));
  const WHOLES = [5, 6, 7, 8, 9, 10];
  const TENTHS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));
  /** Event of this tatami (from the tatami picker); every event has its own tatami 1, 2, ... */
  let eventId = $derived(Number(page.url.searchParams.get("event")) || null);
  let seatKey = $derived(`ambrilindo_judge_seat_t${tatamiNumber}`);
  let judgeCountKey = $derived(`ambrilindo_judge_count_t${tatamiNumber}`);

  // --- seat ---
  let seat = $state<number | null>(null);
  let seatPickerOpen = $state(false);
  let exitConfirmOpen = $state(false);

  function exitToLanding() {
    window.location.href = "/";
  }

  // --- seats used by the other jury tablets of this tatami (they announce themselves) ---
  const PRESENCE_EVERY_MS = 10000;
  const PRESENCE_STALE_MS = 25000;
  let tabletId = "";
  let otherTablets = $state<Record<string, { seat: number; at: number }>>({});
  let now = $state(Date.now());
  let seatsTakenElsewhere = $derived(
    new Set(
      Object.values(otherTablets)
        .filter((t) => now - t.at < PRESENCE_STALE_MS)
        .map((t) => t.seat)
    )
  );
  let seatClash = $derived(seat !== null && seatsTakenElsewhere.has(seat));

  // --- connection & safeguards ---
  let socket: TatamiSocket | null = null;
  let connection = $state<"connecting" | "live" | "offline">("connecting");
  let lastMessageAt = $state(Date.now());
  let isFetchingHttp = false;

  // --- match (from the control panel's live state) ---
  let matchId = $state<number | null>(null);
  let categoryName = $state("");
  let roundLabel = $state("");
  let matchNumber = $state<number | string>("");
  let isKata = $state(true);
  let isFinished = $state(false);
  let akaName = $state("");
  let akaContingent = $state("");
  let akaKata = $state("");
  let aoName = $state("");
  let aoContingent = $state("");
  let aoKata = $state("");
  let deskAka = $state<string[]>([]);
  let deskAo = $state<string[]>([]);

  // --- scoring ---
  let aka = $state<Entry>({ whole: null, tenth: 0 });
  let ao = $state<Entry>({ whole: null, tenth: 0 });
  let dqConfirm = $state<Side | null>(null);
  let tieConfirm = $state(false);
  let sending = $state(false);
  let sent = $state<{ matchId: number | null; aka: string; ao: string } | null>(null);
  let sentAt = 0;
  let toast = $state<{ text: string; kind: "ok" | "error" } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  function value(e: Entry): number | null {
    if (e.whole === null) return null;
    if (e.whole === 0) return 0;
    return e.whole === 10 ? 10 : e.whole + e.tenth / 10;
  }

  function fmt(v: number | null): string {
    return v === null ? "—" : v.toFixed(1);
  }

  let akaValue = $derived(value(aka));
  let aoValue = $derived(value(ao));
  let ready = $derived(akaValue !== null && aoValue !== null);
  let isTie = $derived(ready && akaValue === aoValue);
  let locked = $derived(sent !== null && sent.matchId === matchId);
  let hasMatch = $derived(Boolean(matchId !== null || akaName || aoName));

  /** The control panel echoed this judge's scores back in its live state. */
  let confirmedByDesk = $derived(
    !!sent &&
      seat !== null &&
      deskAka[seat - 1] !== undefined &&
      Number(deskAka[seat - 1]) === Number(sent.aka) &&
      Number(deskAo[seat - 1]) === Number(sent.ao)
  );

  function showToast(text: string, kind: "ok" | "error" = "ok") {
    toast = { text, kind };
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), kind === "error" ? 4500 : 2500);
  }

  function setEntry(side: Side, next: Entry) {
    if (locked) return;
    if (side === "aka") aka = next;
    else ao = next;
    tieConfirm = false;
  }

  function pickWhole(side: Side, whole: number) {
    const cur = side === "aka" ? aka : ao;
    setEntry(side, { whole, tenth: whole === 10 || cur.whole === 0 || cur.whole === null ? 0 : cur.tenth });
  }

  function pickTenth(side: Side, tenth: number) {
    const cur = side === "aka" ? aka : ao;
    if (cur.whole === null || cur.whole === 0 || cur.whole === 10) return;
    setEntry(side, { whole: cur.whole, tenth });
  }

  function requestDq(side: Side) {
    if (locked) return;
    if (dqConfirm === side) {
      setEntry(side, { whole: 0, tenth: 0 });
      dqConfirm = null;
    } else {
      dqConfirm = side;
    }
  }

  function chooseSeat(n: number) {
    seat = n;
    seatPickerOpen = false;
    sent = null;
    try {
      localStorage.setItem(seatKey, String(n));
    } catch (_) {}
    announceSeat();
  }

  function announceSeat() {
    if (seat === null || !tabletId) return;
    socket?.send({ type: "jury_presence", tatami: tatamiNumber, seat, tablet: tabletId });
  }

  function resetScores() {
    aka = { whole: null, tenth: 0 };
    ao = { whole: null, tenth: 0 };
    dqConfirm = null;
    tieConfirm = false;
  }

  async function submit() {
    if (!ready || seat === null || sending || locked) return;
    if (isTie && !tieConfirm) {
      tieConfirm = true;
      return;
    }
    if (connection === "offline") {
      showToast("Koneksi sedang offline. Nilai Anda tetap tersimpan, tunggu jaringan terhubung.", "error");
      return;
    }
    sending = true;
    const payload = {
      type: "judge_score",
      tatami: tatamiNumber,
      juri: seat,
      score_aka: fmt(akaValue),
      score_ao: fmt(aoValue),
    };
    let ok = false;
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/tatami/submit-judge-score`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      ok = res.ok;
      if (ok) {
        connection = "live";
        lastMessageAt = Date.now();
      }
    } catch (_) {
      ok = false;
    }
    sending = false;
    tieConfirm = false;
    if (!ok) {
      showToast("Nilai belum terkirim ke meja. Nilai Anda aman di layar, coba kirim lagi.", "error");
      return;
    }
    sent = { matchId, aka: payload.score_aka, ao: payload.score_ao };
    sentAt = Date.now();
    showToast(`Nilai Juri ${seat} terkirim`);
    try {
      navigator.vibrate?.([40, 60, 40]);
    } catch (_) {}
  }

  function unlock() {
    sent = null;
    showToast(`Koreksi nilai Juri ${seat}`);
  }

  // --- live state from the control panel ---
  function applyLiveState(raw: any) {
    if (!raw) return;
    const d = raw.payload || raw.data || raw;
    const tNum = Number(d.tatami ?? raw.tatami ?? raw.tatami_pk);
    if (tNum && tNum !== tatamiNumber) return;

    const nextMatchId = d.matchId !== undefined ? d.matchId : (d.id !== undefined ? d.id : null);
    const nextAkaName = d.aka?.name || d.atlet1_nama || "";
    const nextAoName = d.ao?.name || d.atlet2_nama || "";
    const nextAkaContingent = d.aka?.contingent || d.atlet1_kontingen || "";
    const nextAoContingent = d.ao?.contingent || d.atlet2_kontingen || "";
    const nextAkaKata = d.aka?.kataName || d.kata1 || "";
    const nextAoKata = d.ao?.kataName || d.kata2 || "";
    const nextCategory = d.category || d.kategori || "";
    const nextRound = d.round ? (d.round === 4 ? "PARTAI FINAL" : d.round === 3 ? "SEMIFINAL" : (typeof d.round === "number" ? `BABAK ${d.round}` : String(d.round))) : "";
    const nextMatchNumber = d.matchNumber ?? d.urutan ?? "";

    // Comprehensive detection of match change:
    // 1. matchId explicitly changed (e.g. 101 -> 102)
    // 2. OR matchId is null/0 (e.g. Festival match) but athlete names changed
    const hadActiveMatch = matchId !== null || !!akaName || !!aoName;
    const isDifferentMatch =
      hadActiveMatch &&
      ((nextMatchId !== null && matchId !== null && nextMatchId !== matchId) ||
        (nextAkaName && akaName && nextAkaName !== akaName) ||
        (nextAoName && aoName && nextAoName !== aoName));

    if (isDifferentMatch) {
      resetScores();
      sent = null;
      try {
        navigator.vibrate?.([60, 80, 60]);
      } catch (_) {}
      const label = nextCategory ? `${nextCategory} ${nextRound}` : (nextRound || "Partai Baru");
      showToast(`Partai baru dibuka: ${label}`, "ok");
    }

    matchId = nextMatchId;
    categoryName = nextCategory;
    roundLabel = nextRound;
    matchNumber = nextMatchNumber;
    isKata = d.isKata !== false;
    isFinished = !!d.isFinished || !!d.selesai;
    const jc = Number(d.judgeCount);
    if ([3, 5, 7].includes(jc) && jc !== judgeCount) {
      judgeCount = jc;
      try {
        localStorage.setItem(judgeCountKey, String(jc));
      } catch (_) {}
      // This seat no longer exists (e.g. panel switched from 5 to 3 judges): choose again.
      if (seat !== null && seat > jc) {
        showToast(`Panel kini ${jc} juri — pilih ulang kursi juri`, "error");
        seat = null;
        seatPickerOpen = true;
      }
    }
    akaName = nextAkaName;
    akaContingent = nextAkaContingent;
    akaKata = nextAkaKata;
    aoName = nextAoName;
    aoContingent = nextAoContingent;
    aoKata = nextAoKata;
    deskAka = Array.isArray(d.aka?.judgeScores) ? d.aka.judgeScores : [];
    deskAo = Array.isArray(d.ao?.judgeScores) ? d.ao.judgeScores : [];

    // The table cleared this judge's scores (e.g. reset button pressed): allow a new submission.
    if (sent && seat !== null && !deskAka[seat - 1] && !deskAo[seat - 1] && sent.matchId === matchId) {
      const age = Date.now() - sentAt;
      if (age > 2500) sent = null;
    }
  }

  // --- HTTP fallback for polling & wakeup ---
  async function fetchLiveStateHttp() {
    if (isFetchingHttp) return;
    isFetchingHttp = true;
    try {
      const url = `${getHostBaseUrl()}/api/tatami/${tatamiNumber}/live-state?_t=${Date.now()}`;
      const res = await fetch(url, {
        cache: "no-store",
        headers: {
          "Cache-Control": "no-cache, no-store, must-revalidate",
          "Pragma": "no-cache",
        },
      });
      if (res.ok) {
        // The host answered: the link is up even when the tatami has no match yet.
        lastMessageAt = Date.now();
        if (connection !== "live" && navigator.onLine !== false) {
          connection = "live";
        }
        const json = await res.json();
        if (json?.data) applyLiveState(json.data);
      }
    } catch (_) {
      // Server unreachable
      if (navigator.onLine === false) {
        connection = "offline";
      }
    } finally {
      isFetchingHttp = false;
    }
  }

  function handleManualReconnect() {
    connection = "connecting";
    fetchLiveStateHttp();
    socket?.send({ type: "request_tatami_state", tatami: tatamiNumber });
    announceSeat();
    showToast("Mencoba menyambungkan kembali...", "ok");
  }

  onMount(() => {
    try {
      const knownCount = Number(localStorage.getItem(judgeCountKey));
      if ([3, 5, 7].includes(knownCount)) judgeCount = knownCount;
      const saved = Number(localStorage.getItem(seatKey));
      // checked against the panel's judge count once its live state arrives
      if (Number.isInteger(saved) && saved >= 1 && saved <= judgeCount) seat = saved;
    } catch (_) {}
    if (seat === null) seatPickerOpen = true;
    try {
      tabletId = sessionStorage.getItem("ambrilindo_tablet_id") || "";
      if (!tabletId) {
        tabletId = Math.random().toString(36).slice(2, 10);
        sessionStorage.setItem("ambrilindo_tablet_id", tabletId);
      }
    } catch (_) {
      tabletId = Math.random().toString(36).slice(2, 10);
    }

    // Initial load: fast HTTP query + WebSocket setup
    fetchLiveStateHttp();

    socket = connectTatamiSocket(
      "jury-panel",
      tatamiNumber,
      (msg) => {
        lastMessageAt = Date.now();
        connection = "live";
        if (msg?.type === "tatami_live_state") {
          applyLiveState(msg.payload || msg.data);
        } else if (
          msg?.type === "jury_presence" &&
          msg.tablet &&
          msg.tablet !== tabletId &&
          Number(msg.tatami) === tatamiNumber
        ) {
          otherTablets = { ...otherTablets, [msg.tablet]: { seat: Number(msg.seat), at: Date.now() } };
        }
      },
      (sock) => {
        connection = "live";
        lastMessageAt = Date.now();
        // A tablet that just (re)connected asks the control panel for the current match.
        sock.send({ type: "request_tatami_state", tatami: tatamiNumber });
        announceSeat();
      },
      () => {
        connection = navigator.onLine === false ? "offline" : "connecting";
      }
    );

    // Safeguard heartbeat & adaptive polling:
    // If connection dropped or quiet > 6s, poll HTTP live-state.
    const health = setInterval(() => {
      const silence = Date.now() - lastMessageAt;
      if (silence > 8000 && connection === "live") {
        connection = "connecting";
      }
      if (connection !== "live" || silence > 12000) {
        fetchLiveStateHttp();
        socket?.send({ type: "request_tatami_state", tatami: tatamiNumber });
      }
    }, 4000);

    // Active safety net: poll HTTP live-state every 2500ms
    // Ensures automatic synchronization in max 2.5s even if WS drops/misses
    const safetyNet = setInterval(() => {
      fetchLiveStateHttp();
    }, 2500);

    const presence = setInterval(() => {
      now = Date.now();
      announceSeat();
    }, PRESENCE_EVERY_MS);

    const onOffline = () => (connection = "offline");
    const onOnline = () => {
      connection = "connecting";
      fetchLiveStateHttp();
    };

    // Wakeup from sleep / tab focus
    const onWakeup = () => {
      if (document.visibilityState === "visible") {
        fetchLiveStateHttp();
        socket?.send({ type: "request_tatami_state", tatami: tatamiNumber });
        announceSeat();
      }
    };

    window.addEventListener("offline", onOffline);
    window.addEventListener("online", onOnline);
    document.addEventListener("visibilitychange", onWakeup);
    window.addEventListener("focus", onWakeup);

    return () => {
      clearInterval(health);
      clearInterval(safetyNet);
      clearInterval(presence);
      window.removeEventListener("offline", onOffline);
      window.removeEventListener("online", onOnline);
      document.removeEventListener("visibilitychange", onWakeup);
      window.removeEventListener("focus", onWakeup);
      socket?.close();
      if (toastTimer) clearTimeout(toastTimer);
    };
  });
</script>

<svelte:head>
  <title>Juri {seat ?? "-"} • Tatami {tatamiNumber}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

{#snippet scorePanel(side: Side)}
  {@const isAka = side === "aka"}
  {@const entry = isAka ? aka : ao}
  {@const v = isAka ? akaValue : aoValue}
  {@const name = isAka ? akaName : aoName}
  {@const contingent = isAka ? akaContingent : aoContingent}
  {@const kata = isAka ? akaKata : aoKata}
  <section
    class="min-h-0 flex flex-col rounded-2xl border-2 overflow-hidden bg-[#15171d] {isAka
      ? 'border-red-600/70'
      : 'border-blue-600/70'} {locked ? 'opacity-70' : ''}"
  >
    <div class="px-3 py-2 flex items-center gap-2 {isAka ? 'bg-red-600' : 'bg-blue-600'} text-white shrink-0">
      <span class="px-1.5 py-0.5 rounded bg-black/30 text-[11px] font-mono font-black tracking-wider">{isAka ? "AKA" : "AO"}</span>
      <div class="min-w-0 flex-1">
        <div class="text-base font-black leading-tight truncate">{name || (isAka ? "Sudut Merah" : "Sudut Biru")}</div>
        <div class="text-[11px] font-semibold opacity-85 truncate">{contingent || "-"}</div>
      </div>
      {#if kata}
        <span class="px-2 py-0.5 rounded-full bg-black/30 border border-white/25 text-[11px] font-bold shrink-0 max-w-[40%] truncate">{kata}</span>
      {/if}
    </div>

    <div class="flex-1 min-h-0 flex flex-col p-2.5 gap-2 overflow-y-auto">
      <div class="flex items-center justify-center py-1 shrink-0">
        <span
          class="font-black font-mono tabular-nums leading-none text-5xl sm:text-7xl {v === null
            ? 'text-zinc-600'
            : isAka
              ? 'text-red-500'
              : 'text-blue-500'}"
        >
          {fmt(v)}
        </span>
        {#if v === 0}
          <span class="ml-2 px-2 py-0.5 rounded bg-white/10 text-xs font-black text-zinc-200">DQ / KIKEN</span>
        {/if}
      </div>

      <div class="grid grid-cols-6 gap-1.5 flex-[1.1] min-h-11">
        {#each WHOLES as w (w)}
          <button
            type="button"
            disabled={locked}
            onclick={() => pickWhole(side, w)}
            class="h-full min-h-11 rounded-xl text-2xl sm:text-3xl font-black font-mono transition active:scale-95 disabled:opacity-40 border {entry.whole === w
              ? isAka
                ? 'bg-red-600 border-red-400 text-white'
                : 'bg-blue-600 border-blue-400 text-white'
              : 'bg-white/[0.05] border-white/10 text-zinc-100'}"
          >
            {w}
          </button>
        {/each}
      </div>

      <div class="grid grid-cols-5 grid-rows-2 gap-1.5 flex-[2] min-h-20">
        {#each TENTHS as t (t)}
          {@const usable = entry.whole !== null && entry.whole !== 0 && entry.whole !== 10}
          <button
            type="button"
            disabled={locked || !usable}
            onclick={() => pickTenth(side, t)}
            class="h-full min-h-9 rounded-xl text-xl sm:text-2xl font-black font-mono transition active:scale-95 disabled:opacity-30 border {usable &&
            entry.tenth === t
              ? isAka
                ? 'bg-red-600/80 border-red-400 text-white'
                : 'bg-blue-600/80 border-blue-400 text-white'
              : 'bg-white/[0.03] border-white/[0.08] text-zinc-300'}"
          >
            .{t}
          </button>
        {/each}
      </div>

      <button
        type="button"
        disabled={locked}
        onclick={() => requestDq(side)}
        class="h-10 shrink-0 rounded-lg text-xs font-bold border transition disabled:opacity-40 {dqConfirm === side
          ? 'bg-rose-600 border-rose-400 text-white'
          : 'bg-transparent border-white/10 text-zinc-400'}"
      >
        {dqConfirm === side ? "Ketuk sekali lagi: nilai 0.0 (DQ / Kiken)" : "0.0 — DQ / Kiken"}
      </button>
    </div>
  </section>
{/snippet}

<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col select-none font-sans overflow-hidden touch-manipulation">
  <!-- Header -->
  <div class="bg-[#14161b] border-b border-white/10 px-3 py-2 flex items-center gap-2 shrink-0">
    <button
      type="button"
      onclick={() => (exitConfirmOpen = true)}
      class="h-10 px-3 rounded-xl bg-white/[0.06] hover:bg-rose-500/20 active:bg-rose-500/30 border border-white/10 hover:border-rose-500/40 text-zinc-300 hover:text-rose-200 flex items-center gap-1.5 text-xs font-bold transition active:scale-95 shrink-0"
      title="Keluar ke Landing Page Website"
    >
      <svg class="w-4 h-4 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"/>
      </svg>
      <span>Keluar</span>
    </button>
    <span class="px-2.5 py-1 rounded-lg bg-white/10 border border-white/10 font-mono font-black text-sm shrink-0">TATAMI {tatamiNumber}</span>
    <span
      class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-bold font-mono border {connection === 'live'
        ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/25'
        : connection === 'connecting'
          ? 'bg-amber-500/10 text-amber-300 border-amber-500/25'
          : 'bg-rose-500/10 text-rose-400 border-rose-500/25'}"
    >
      <span
        class="w-1.5 h-1.5 rounded-full {connection === 'live'
          ? 'bg-emerald-400'
          : connection === 'connecting'
            ? 'bg-amber-300 animate-pulse'
            : 'bg-rose-400'}"
      ></span>
      {connection === "live" ? "TERHUBUNG" : connection === "connecting" ? "MENYAMBUNG" : "OFFLINE"}
    </span>
    <div class="flex-1"></div>
    <button
      type="button"
      onclick={() => (seatPickerOpen = true)}
      class="h-10 px-4 rounded-xl bg-white text-black font-black text-sm"
    >
      JURI {seat ?? "?"}
    </button>
  </div>

  <!-- Disconnect Safeguard Alert Banners -->
  {#if connection === "offline"}
    <div class="bg-rose-600 text-white px-3 py-2 flex items-center justify-between gap-2 shrink-0 shadow-lg z-30">
      <div class="flex items-center gap-2 min-w-0">
        <span class="text-base shrink-0">⚠️</span>
        <span class="text-xs font-bold truncate">Jaringan offline. Nilai yang Anda pilih tersimpan aman di layar.</span>
      </div>
      <button
        type="button"
        onclick={handleManualReconnect}
        class="h-7 px-2.5 rounded bg-white text-rose-700 text-xs font-black shrink-0 active:scale-95 transition"
      >
        Coba Sambung
      </button>
    </div>
  {:else if connection === "connecting"}
    <div class="bg-amber-500/20 border-b border-amber-500/30 text-amber-200 px-3 py-1.5 flex items-center justify-between gap-2 shrink-0 z-30">
      <div class="flex items-center gap-2 min-w-0 text-xs font-medium">
        <span class="w-2 h-2 rounded-full bg-amber-400 animate-ping shrink-0"></span>
        <span class="truncate">Menyambungkan kembali ke Meja Tatami {tatamiNumber}...</span>
      </div>
      <button
        type="button"
        onclick={handleManualReconnect}
        class="h-6 px-2 rounded bg-amber-400 text-black text-[11px] font-bold shrink-0 active:scale-95 transition"
      >
        Segarkan
      </button>
    </div>
  {/if}

  {#if seatClash}
    <div class="bg-rose-600 text-white px-3 py-2 flex items-center gap-3 shrink-0">
      <span class="text-sm font-bold flex-1">Kursi Juri {seat} juga dipakai tablet lain. Nilai salah satu tablet akan tertimpa.</span>
      <button type="button" onclick={() => (seatPickerOpen = true)} class="h-9 px-3 rounded-lg bg-white text-rose-700 font-black text-xs shrink-0">
        Ganti kursi
      </button>
    </div>
  {/if}

  <!-- Match strip -->
  <div class="bg-[#111317] border-b border-white/[0.06] px-3 py-1.5 flex items-center gap-2 text-xs shrink-0">
    {#if hasMatch}
      <span class="font-bold text-zinc-100 truncate">{categoryName || "Partai aktif"}</span>
      {#if roundLabel}<span class="text-zinc-500 shrink-0">• {roundLabel}</span>{/if}
      <div class="flex-1"></div>
      {#if matchNumber !== ""}<span class="font-mono font-bold text-zinc-400 shrink-0">Partai #{matchNumber}</span>{/if}
    {:else}
      <span class="text-zinc-400">Menunggu partai dari Meja Tatami {tatamiNumber}…</span>
    {/if}
  </div>

  {#if toast}
    <div
      class="fixed top-16 left-1/2 -translate-x-1/2 z-40 px-4 py-2.5 rounded-xl text-sm font-bold shadow-2xl max-w-[92vw] text-center {toast.kind === 'ok'
        ? 'bg-emerald-500 text-black'
        : 'bg-rose-600 text-white'}"
    >
      {toast.text}
    </div>
  {/if}

  <!-- Main Body -->
  {#if !hasMatch}
    <!-- Empty State: Waiting for table to open a match -->
    <div class="flex-1 flex flex-col items-center justify-center p-6 text-center">
      <div class="w-16 h-16 rounded-2xl bg-white/[0.04] border border-white/10 flex items-center justify-center mb-4 text-3xl">
        🥋
      </div>
      <h3 class="text-xl font-black text-white">Menunggu Partai Dimulai</h3>
      <p class="text-sm text-zinc-400 max-w-sm mt-1.5">
        Operator Meja Tatami {tatamiNumber} belum membuka partai. Layar penilaian juri akan muncul secara otomatis begitu partai dibuka.
      </p>
      <div class="mt-6 flex flex-col sm:flex-row items-center gap-3">
        <button
          type="button"
          onclick={() => (seatPickerOpen = true)}
          class="h-11 px-5 rounded-xl bg-white/[0.08] border border-white/15 text-white font-bold text-xs active:scale-95 transition"
        >
          Kursi Saat Ini: Juri {seat ?? "?"} (Ganti)
        </button>
        <button
          type="button"
          onclick={handleManualReconnect}
          class="h-11 px-5 rounded-xl bg-white text-black font-black text-xs active:scale-95 transition"
        >
          Muat Ulang Data Meja
        </button>
      </div>
    </div>
  {:else if !isKata}
    <div class="flex-1 flex items-center justify-center p-6 text-center">
      <div>
        <div class="text-lg font-black text-white">Partai Ini Adalah Kumite</div>
        <div class="text-sm text-zinc-400 mt-1">Panel nilai Kata otomatis aktif kembali saat meja membuka partai Kata.</div>
      </div>
    </div>
  {:else if isFinished}
    <div class="flex-1 flex items-center justify-center p-6 text-center">
      <div>
        <div class="text-lg font-black text-white">Partai Sudah Selesai</div>
        <div class="text-sm text-zinc-400 mt-1">Tunggu operator meja membuka partai berikutnya. Layar akan berganti otomatis.</div>
      </div>
    </div>
  {:else}
    <!-- Score panels: stacked on a portrait tablet, side by side in landscape -->
    <main class="flex-1 min-h-0 p-2.5 grid grid-rows-2 landscape:grid-rows-1 landscape:grid-cols-2 gap-2.5 overflow-y-auto">
      {@render scorePanel("aka")}
      {@render scorePanel("ao")}
    </main>

    <!-- Submit bar -->
    <div class="bg-[#14161b] border-t border-white/10 p-2.5 shrink-0">
      {#if locked && sent}
        <div class="flex items-center gap-2">
          <div
            class="flex-1 h-14 rounded-xl border-2 flex items-center justify-center gap-2 font-black text-sm {confirmedByDesk
              ? 'bg-emerald-500/15 border-emerald-500/50 text-emerald-300'
              : 'bg-amber-500/10 border-amber-500/40 text-amber-200'}"
          >
            {#if confirmedByDesk}
              ✓ Diterima meja — AKA {sent.aka} • AO {sent.ao}
            {:else}
              Terkirim, menunggu meja… AKA {sent.aka} • AO {sent.ao}
            {/if}
          </div>
          <button
            type="button"
            onclick={unlock}
            class="h-14 px-5 rounded-xl bg-white/[0.06] border border-white/15 text-zinc-200 font-bold text-sm active:scale-95 transition"
          >
            Koreksi
          </button>
        </div>
      {:else}
        {#if tieConfirm}
          <div class="mb-2 px-3 py-2 rounded-xl bg-amber-500/15 border border-amber-500/40 text-amber-200 text-xs font-semibold">
            Nilai AKA dan AO sama ({fmt(akaValue)}). Juri ini tidak akan memberi bendera ke siapa pun.
          </div>
        {/if}
        <button
          type="button"
          onclick={submit}
          disabled={!ready || seat === null || sending || !hasMatch || connection === "offline"}
          class="w-full h-14 rounded-xl font-black text-base transition active:scale-[0.98] disabled:opacity-40 {connection ===
          'offline'
            ? 'bg-rose-900/50 text-rose-300 border border-rose-500/30'
            : tieConfirm
              ? 'bg-amber-400 text-black'
              : 'bg-white text-black'}"
        >
          {#if sending}
            Mengirim…
          {:else if connection === "offline"}
            Koneksi Terputus — Menunggu Jaringan...
          {:else if !hasMatch}
            Menunggu partai
          {:else if !ready}
            Isi nilai AKA dan AO
          {:else if tieConfirm}
            Tetap kirim nilai sama
          {:else}
            Kirim nilai Juri {seat} — AKA {fmt(akaValue)} • AO {fmt(aoValue)}
          {/if}
        </button>
      {/if}
    </div>
  {/if}
</div>

<!-- Seat picker: shown on first open of this tatami and from the header -->
{#if seatPickerOpen}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-5">
    <div class="w-full max-w-md bg-[#181a20] border border-white/10 rounded-2xl p-5">
      <h2 class="text-lg font-black text-white">Tatami {tatamiNumber} — pilih kursi juri</h2>
      <div class="grid {judgeCount === 3 ? 'grid-cols-3' : judgeCount === 7 ? 'grid-cols-4' : 'grid-cols-5'} gap-2 mt-4">
        {#each JUDGE_SEATS as n (n)}
          {@const taken = seatsTakenElsewhere.has(n)}
          <button
            type="button"
            onclick={() => chooseSeat(n)}
            class="aspect-square rounded-2xl flex flex-col items-center justify-center border-2 transition active:scale-95 {seat === n
              ? 'bg-white text-black border-white'
              : taken
                ? 'bg-white/[0.02] text-zinc-500 border-white/10'
                : 'bg-white/[0.04] text-white border-white/15'}"
          >
            <span class="text-3xl font-black font-mono leading-none">{n}</span>
            {#if taken}<span class="text-[10px] font-bold mt-1 text-rose-400">dipakai</span>{/if}
          </button>
        {/each}
      </div>
      {#if seat !== null}
        <button
          type="button"
          onclick={() => (seatPickerOpen = false)}
          class="mt-4 w-full h-11 rounded-xl bg-white/[0.06] border border-white/10 text-zinc-200 font-bold text-sm"
        >
          Batal
        </button>
      {/if}
      <button
        type="button"
        onclick={exitToLanding}
        class="mt-2 w-full h-10 rounded-xl bg-transparent hover:bg-white/[0.04] text-zinc-400 hover:text-zinc-200 text-xs font-semibold flex items-center justify-center gap-1.5 transition"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"/>
        </svg>
        <span>Keluar ke Beranda Website</span>
      </button>
    </div>
  </div>
{/if}

<!-- Exit confirmation modal -->
{#if exitConfirmOpen}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-5">
    <div class="w-full max-w-sm bg-[#181a20] border border-white/10 rounded-2xl p-5 shadow-2xl">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl bg-rose-500/15 border border-rose-500/30 text-rose-400 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"/>
          </svg>
        </div>
        <div>
          <h3 class="text-base font-black text-white">Keluar dari Panel Juri?</h3>
          <p class="text-xs text-zinc-400 mt-0.5">Kembali ke landing page portal website</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-white/[0.03] border border-white/10 text-xs text-zinc-300 space-y-1">
        <div class="flex justify-between font-mono">
          <span class="text-zinc-500">Posisi:</span>
          <span class="font-bold text-white">Tatami {tatamiNumber} — {seat ? `Juri ${seat}` : "Kursi Belum Dipilih"}</span>
        </div>
        {#if hasMatch && ready && !locked}
          <div class="flex items-center gap-1.5 text-amber-400 pt-1 font-semibold">
            <span>⚠ Nilai yang diisi belum dikirim ke meja!</span>
          </div>
        {/if}
      </div>

      <div class="grid grid-cols-2 gap-2">
        <button
          type="button"
          onclick={() => (exitConfirmOpen = false)}
          class="h-11 rounded-xl bg-white/[0.08] hover:bg-white/15 border border-white/10 text-zinc-200 font-bold text-sm transition active:scale-95"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={exitToLanding}
          class="h-11 rounded-xl bg-rose-600 hover:bg-rose-500 text-white font-black text-sm shadow-lg shadow-rose-950/40 transition active:scale-95 flex items-center justify-center gap-1.5"
        >
          <span>Ya, Keluar</span>
        </button>
      </div>
    </div>
  </div>
{/if}
