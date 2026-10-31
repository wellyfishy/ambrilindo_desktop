<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";

  // Derive tatami number from route params (e.g. /jury/1 -> 1)
  let tatamiNumber = $derived(Number(page.params.tatami || "1"));

  // Judge seat state (1 to 5)
  let selectedJudge = $state<number>(1);
  let isJudgeSelectorOpen = $state(false);

  // Connection state
  let isWsConnected = $state(false);
  let wsClient: WebSocket | null = null;
  let reconnectInterval: any = null;

  // Match state received from Control Panel / Server
  let matchId = $state<number | null>(null);
  let eventName = $state("AMBRILINDO KARATE CHAMPIONSHIP");
  let categoryName = $state("MEMUAT PARTAI...");
  let matchRound = $state("BABAK PENYISIHAN");
  let matchNumber = $state<number | string>(1);
  let isKata = $state(true);
  let isMatchFinished = $state(false);

  // AKA (Red) combatant state
  let akaName = $state("SUDUT MERAH (AKA)");
  let akaContingent = $state("-");
  let akaKataName = $state("");
  let akaScore = $state<number>(7.5);

  // AO (Blue) combatant state
  let aoName = $state("SUDUT BIRU (AO)");
  let aoContingent = $state("-");
  let aoKataName = $state("");
  let aoScore = $state<number>(7.5);

  // Kumite Corner Flag State
  let activeKumiteFlag = $state<string | null>(null);

  // Submission lock state
  let isSubmitted = $state(false);
  let lastSubmittedMatchId = $state<number | null>(null);
  let isSending = $state(false);
  let toastMessage = $state<string | null>(null);
  let toastTimer: any = null;

  function showToast(msg: string) {
    toastMessage = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage = null;
    }, 2500);
  }

  // Score adjustments (Kata: 5.0 - 10.0 or 0.0 for DQ)
  function adjustScore(side: "aka" | "ao", delta: number) {
    if (isSubmitted) return;
    if (side === "aka") {
      let next = Math.round((akaScore + delta) * 10) / 10;
      if (next < 5.0 && next > 0) next = 5.0;
      if (next > 10.0) next = 10.0;
      akaScore = next;
    } else {
      let next = Math.round((aoScore + delta) * 10) / 10;
      if (next < 5.0 && next > 0) next = 5.0;
      if (next > 10.0) next = 10.0;
      aoScore = next;
    }
  }

  function setPresetScore(side: "aka" | "ao", val: number) {
    if (isSubmitted) return;
    if (side === "aka") akaScore = val;
    else aoScore = val;
  }

  // Kumite flag toggle
  function toggleKumiteFlag(flagName: string) {
    if (isSubmitted) return;
    if (activeKumiteFlag === flagName) {
      activeKumiteFlag = null;
    } else {
      activeKumiteFlag = flagName;
    }
    // Instant broadcast for kumite corner flags
    sendKumiteFlag(activeKumiteFlag);
  }

  async function sendKumiteFlag(flag: string | null) {
    const payload = {
      type: "judge_kumite_flag",
      tatami: tatamiNumber,
      juri: selectedJudge,
      flag: flag || "none",
    };

    if (wsClient && wsClient.readyState === WebSocket.OPEN) {
      wsClient.send(JSON.stringify(payload));
    }
  }

  // Submit Kata score
  async function submitKataScores() {
    if (isSending) return;
    isSending = true;

    const payload = {
      type: "judge_score",
      tatami: tatamiNumber,
      juri: selectedJudge,
      score_aka: akaScore.toFixed(1),
      score_ao: aoScore.toFixed(1),
    };

    let sent = false;

    // 1. Try sending over WebSocket
    if (wsClient && wsClient.readyState === WebSocket.OPEN) {
      try {
        wsClient.send(JSON.stringify(payload));
        sent = true;
      } catch (err) {
        console.warn("WebSocket send failed:", err);
      }
    }

    // 2. HTTP fallback if WebSocket send didn't succeed
    if (!sent) {
      try {
        const res = await fetch("/api/tatami/submit-judge-score", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
        });
        if (res.ok) sent = true;
      } catch (err) {
        console.error("HTTP score submit failed:", err);
      }
    }

    isSending = false;
    isSubmitted = true;
    lastSubmittedMatchId = matchId;
    showToast(`✓ Nilai Juri ${selectedJudge} Berhasil Terkirim!`);

    // Trigger haptic vibration if supported on mobile
    if (typeof navigator !== "undefined" && navigator.vibrate) {
      navigator.vibrate([40, 60, 40]);
    }
  }

  function unlockScores() {
    isSubmitted = false;
    showToast(`Koreksi nilai aktif untuk Juri ${selectedJudge}`);
  }

  function selectJudgeSeat(seatNum: number) {
    selectedJudge = seatNum;
    isJudgeSelectorOpen = false;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("ambrilindo_judge_seat", String(seatNum));
    }
    // Reset submission lock when switching seat
    isSubmitted = false;
    showToast(`Beralih ke Wasit / Juri #${seatNum}`);
  }

  // Apply match state from Live Sync / WebSocket
  function applyIncomingMatchState(data: any) {
    if (!data) return;

    if (data.matchId !== undefined && data.matchId !== matchId) {
      // New match loaded -> automatically unlock and reset defaults if new match
      if (lastSubmittedMatchId !== data.matchId) {
        isSubmitted = false;
        akaScore = 7.5;
        aoScore = 7.5;
        activeKumiteFlag = null;
      }
      matchId = data.matchId;
    }

    if (data.category) categoryName = data.category;
    if (data.round) matchRound = data.round;
    if (data.matchNumber !== undefined) matchNumber = data.matchNumber;
    if (data.isKata !== undefined) isKata = data.isKata;
    if (data.isFinished !== undefined) isMatchFinished = data.isFinished;

    if (data.aka) {
      if (data.aka.name) akaName = data.aka.name;
      if (data.aka.contingent) akaContingent = data.aka.contingent;
      if (data.aka.kataName !== undefined) akaKataName = data.aka.kataName;
      // Sync judge score if already set from table and we haven't edited
      if (!isSubmitted && data.aka.judgeScores && data.aka.judgeScores[selectedJudge - 1]) {
        const val = parseFloat(data.aka.judgeScores[selectedJudge - 1]);
        if (!isNaN(val) && val >= 5.0) akaScore = val;
      }
    }

    if (data.ao) {
      if (data.ao.name) aoName = data.ao.name;
      if (data.ao.contingent) aoContingent = data.ao.contingent;
      if (data.ao.kataName !== undefined) aoKataName = data.ao.kataName;
      if (!isSubmitted && data.ao.judgeScores && data.ao.judgeScores[selectedJudge - 1]) {
        const val = parseFloat(data.ao.judgeScores[selectedJudge - 1]);
        if (!isNaN(val) && val >= 5.0) aoScore = val;
      }
    }
  }

  // Initial HTTP fetch for tatami active match
  async function fetchInitialMatch() {
    try {
      const res = await fetch(`/api/tatami/${tatamiNumber}/current-match`);
      if (res.ok) {
        const json = await res.json();
        if (json.status === "success" && json.data) {
          const d = json.data;
          matchId = d.id;
          categoryName = d.kode || "KATA / KUMITE";
          matchRound = d.round ? `BABAK ${d.round}` : "BABAK PENYISIHAN";
          matchNumber = d.urutan || 1;
          akaName = d.atlet1_nama || "Sudut Merah (Aka)";
          akaContingent = d.atlet1_kontingen || "-";
          akaKataName = d.kata1 || "";
          aoName = d.atlet2_nama || "Sudut Biru (Ao)";
          aoContingent = d.atlet2_kontingen || "-";
          aoKataName = d.kata2 || "";
        }
      }
    } catch (err) {
      console.warn("Initial match fetch failed:", err);
    }
  }

  // WebSocket Connection Lifecycle
  function connectWebSocket() {
    if (typeof window === "undefined") return;

    try {
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const host = window.location.host;
      const wsUrl = `${protocol}//${host}/ws/jury-panel/${tatamiNumber}`;

      wsClient = new WebSocket(wsUrl);

      wsClient.onopen = () => {
        isWsConnected = true;
      };

      wsClient.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data);
          // If message is live tatami state
          if (parsed && (parsed.tatami === tatamiNumber || !parsed.tatami)) {
            applyIncomingMatchState(parsed);
          }
        } catch (e) {
          console.warn("WS Parse error:", e);
        }
      };

      wsClient.onclose = () => {
        isWsConnected = false;
      };

      wsClient.onerror = () => {
        isWsConnected = false;
      };
    } catch (e) {
      isWsConnected = false;
    }
  }

  onMount(() => {
    // 1. Restore seat from localStorage if saved
    if (typeof localStorage !== "undefined") {
      const savedSeat = localStorage.getItem("ambrilindo_judge_seat");
      if (savedSeat) {
        selectedJudge = Math.max(1, Math.min(5, parseInt(savedSeat, 10) || 1));
      }
    }

    // 2. Fetch initial match
    fetchInitialMatch();

    // 3. Connect WebSocket
    connectWebSocket();

    // 4. Auto-reconnect timer if disconnected
    reconnectInterval = setInterval(() => {
      if (!wsClient || wsClient.readyState === WebSocket.CLOSED) {
        connectWebSocket();
      }
    }, 3000);

    return () => {
      if (wsClient) wsClient.close();
      if (reconnectInterval) clearInterval(reconnectInterval);
      if (toastTimer) clearTimeout(toastTimer);
    };
  });
</script>

<svelte:head>
  <title>Panel Wasit Juri • Tatami {tatamiNumber} • Juri #{selectedJudge}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<!-- Mobile Touch Screen Container (Fixed Fullscreen, Zero Accidental Scroll) -->
<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col justify-between select-none font-sans overflow-hidden touch-manipulation">
  
  <!-- Top Navigation & Judge Seat Selector Header -->
  <header class="bg-[#14161b] border-b border-white/10 px-3.5 py-2.5 flex items-center justify-between shrink-0 shadow-lg relative z-20">
    <div class="flex items-center gap-2">
      <!-- Tatami Badge -->
      <span class="px-2.5 py-1 rounded-lg bg-white/10 text-white font-mono font-black text-xs tracking-wider border border-white/10">
        TATAMI {tatamiNumber}
      </span>

      <!-- Online / Offline Status Dot -->
      <div class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono {isWsConnected ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-red-500/10 text-red-400 border border-red-500/20'}">
        <span class="w-1.5 h-1.5 rounded-full {isWsConnected ? 'bg-emerald-400 animate-pulse' : 'bg-red-400'}"></span>
        <span>{isWsConnected ? 'LIVE' : 'OFFLINE'}</span>
      </div>
    </div>

    <!-- Judge Seat Switcher Pill -->
    <div class="relative">
      <button
        onclick={() => isJudgeSelectorOpen = !isJudgeSelectorOpen}
        class="h-8 px-3 rounded-lg bg-[#1e222b] hover:bg-[#282d39] text-white border border-white/15 text-xs font-black flex items-center gap-1.5 shadow transition"
      >
        <span>JURI #{selectedJudge}</span>
        <svg class="w-3.5 h-3.5 text-zinc-400 transition transform {isJudgeSelectorOpen ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {#if isJudgeSelectorOpen}
        <!-- Dropdown Menu -->
        <div class="absolute right-0 top-10 w-44 bg-[#181a20] border border-white/20 rounded-xl shadow-2xl py-1.5 z-50 animate-in fade-in zoom-in-95 duration-100">
          <div class="px-3 py-1 text-[10px] font-mono uppercase tracking-wider text-zinc-400 font-bold border-b border-white/10">
            KURSI JURI
          </div>
          {#each [1, 2, 3, 4, 5] as seat}
            <button
              onclick={() => selectJudgeSeat(seat)}
              class="w-full px-3 py-2 text-left text-xs font-bold flex items-center justify-between transition {selectedJudge === seat ? 'bg-white text-black' : 'text-zinc-200 hover:bg-white/[0.08]'}"
            >
              <span>WASIT / JURI #{seat}</span>
              {#if selectedJudge === seat}
                <span class="text-[10px] font-mono font-black">AKTIF</span>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </header>

  <!-- Match Info Strip -->
  <div class="bg-[#111317] border-b border-white/[0.06] px-3.5 py-1.5 flex items-center justify-between text-[11px] shrink-0 font-medium">
    <div class="flex items-center gap-2 truncate">
      <span class="font-bold text-zinc-200 truncate">{categoryName}</span>
      <span class="text-zinc-500">•</span>
      <span class="font-mono text-zinc-400 shrink-0">{matchRound}</span>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      <span class="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold {isKata ? 'bg-purple-500/20 text-purple-300 border border-purple-500/30' : 'bg-amber-500/20 text-amber-300 border border-amber-500/30'}">
        {isKata ? 'KATA' : 'KUMITE'}
      </span>
      <span class="font-mono font-bold text-zinc-400 text-[10px]">#{matchNumber}</span>
    </div>
  </div>

  <!-- Toast Notification Overlay -->
  {#if toastMessage}
    <div class="fixed top-14 left-1/2 -translate-x-1/2 z-50 bg-emerald-500 text-black px-4 py-2 rounded-xl text-xs font-bold shadow-2xl animate-in slide-in-from-top-2 duration-150 flex items-center gap-2">
      <svg class="w-4 h-4 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
      </svg>
      <span>{toastMessage}</span>
    </div>
  {/if}

  <!-- Main Combatants Scoring Area (Split 50% / 50%) -->
  <main class="flex-1 min-h-0 p-2.5 flex flex-col gap-2.5 overflow-hidden">
    
    <!-- ==================== AKA CARD (RED) ==================== -->
    <section class="flex-1 min-h-0 bg-[#16181f] border-2 {isSubmitted ? 'border-red-500/40 opacity-90' : 'border-red-600 shadow-lg shadow-red-950/20'} rounded-2xl flex flex-col overflow-hidden relative">
      <!-- Card Header Strip -->
      <div class="bg-red-600 px-3 py-1.5 text-white flex items-center justify-between shrink-0 shadow">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-1.5">
            <span class="px-1.5 py-0.2 rounded bg-black/40 text-[10px] font-mono font-black tracking-wider uppercase">AKA</span>
            <h2 class="text-sm font-black truncate tracking-tight">{akaName}</h2>
          </div>
          <p class="text-[10px] text-red-100 font-medium truncate mt-0.5">{akaContingent}</p>
        </div>
        {#if isKata && akaKataName}
          <span class="px-2 py-0.5 rounded-full bg-black/35 text-[10px] font-mono font-bold text-white border border-white/20 ml-2 shrink-0">
            {akaKataName}
          </span>
        {/if}
      </div>

      <!-- Card Body / Controls -->
      <div class="flex-1 min-h-0 flex flex-col justify-between p-2.5">
        {#if isKata}
          <!-- Large Digital Score Display -->
          <div class="flex-1 flex items-center justify-center min-h-0">
            <div class="text-center">
              <span class="text-5xl sm:text-6xl font-black font-mono tracking-tight text-red-500 drop-shadow-md">
                {akaScore === 0 ? "0.0 (DQ)" : akaScore.toFixed(1)}
              </span>
            </div>
          </div>

          <!-- Steppers Row -->
          <div class="grid grid-cols-4 gap-1.5 shrink-0 my-1">
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("aka", -0.5)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-red-400 font-mono font-black text-sm border border-red-500/20 flex items-center justify-center transition"
            >-0.5</button>
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("aka", -0.1)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-red-400 font-mono font-black text-sm border border-red-500/20 flex items-center justify-center transition"
            >-0.1</button>
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("aka", 0.1)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-red-400 font-mono font-black text-sm border border-red-500/20 flex items-center justify-center transition"
            >+0.1</button>
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("aka", 0.5)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-red-400 font-mono font-black text-sm border border-red-500/20 flex items-center justify-center transition"
            >+0.5</button>
          </div>

          <!-- Presets Quick Bar -->
          <div class="grid grid-cols-5 gap-1 shrink-0">
            {#each [0.0, 7.0, 7.5, 8.0, 8.5] as preset}
              <button
                disabled={isSubmitted}
                onclick={() => setPresetScore("aka", preset)}
                class="h-7 rounded-lg text-[10px] font-mono font-bold transition border active:scale-95 disabled:opacity-40 {akaScore === preset ? 'bg-red-600 text-white border-red-500 font-black' : 'bg-white/[0.03] text-zinc-400 border-white/[0.06] hover:bg-white/[0.08]'}"
              >
                {preset === 0 ? 'DQ' : preset.toFixed(1)}
              </button>
            {/each}
          </div>
        {:else}
          <!-- Kumite Flags Mode -->
          <div class="flex-1 flex flex-col justify-center">
            <div class="grid grid-cols-3 gap-2">
              <button
                onclick={() => toggleKumiteFlag("aka_yuko")}
                class="h-14 rounded-xl font-black text-xs font-mono transition border active:scale-95 {activeKumiteFlag === 'aka_yuko' ? 'bg-red-600 text-white border-white shadow-lg' : 'bg-red-950/40 text-red-300 border-red-500/30'}"
              >
                YUKO (1)
              </button>
              <button
                onclick={() => toggleKumiteFlag("aka_wazaari")}
                class="h-14 rounded-xl font-black text-xs font-mono transition border active:scale-95 {activeKumiteFlag === 'aka_wazaari' ? 'bg-red-600 text-white border-white shadow-lg' : 'bg-red-950/40 text-red-300 border-red-500/30'}"
              >
                WAZA-ARI (2)
              </button>
              <button
                onclick={() => toggleKumiteFlag("aka_ippon")}
                class="h-14 rounded-xl font-black text-xs font-mono transition border active:scale-95 {activeKumiteFlag === 'aka_ippon' ? 'bg-red-600 text-white border-white shadow-lg' : 'bg-red-950/40 text-red-300 border-red-500/30'}"
              >
                IPPON (3)
              </button>
            </div>
          </div>
        {/if}
      </div>
    </section>

    <!-- ==================== AO CARD (BLUE) ==================== -->
    <section class="flex-1 min-h-0 bg-[#16181f] border-2 {isSubmitted ? 'border-blue-500/40 opacity-90' : 'border-blue-600 shadow-lg shadow-blue-950/20'} rounded-2xl flex flex-col overflow-hidden relative">
      <!-- Card Header Strip -->
      <div class="bg-blue-600 px-3 py-1.5 text-white flex items-center justify-between shrink-0 shadow">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-1.5">
            <span class="px-1.5 py-0.2 rounded bg-black/40 text-[10px] font-mono font-black tracking-wider uppercase">AO</span>
            <h2 class="text-sm font-black truncate tracking-tight">{aoName}</h2>
          </div>
          <p class="text-[10px] text-blue-100 font-medium truncate mt-0.5">{aoContingent}</p>
        </div>
        {#if isKata && aoKataName}
          <span class="px-2 py-0.5 rounded-full bg-black/35 text-[10px] font-mono font-bold text-white border border-white/20 ml-2 shrink-0">
            {aoKataName}
          </span>
        {/if}
      </div>

      <!-- Card Body / Controls -->
      <div class="flex-1 min-h-0 flex flex-col justify-between p-2.5">
        {#if isKata}
          <!-- Large Digital Score Display -->
          <div class="flex-1 flex items-center justify-center min-h-0">
            <div class="text-center">
              <span class="text-5xl sm:text-6xl font-black font-mono tracking-tight text-blue-500 drop-shadow-md">
                {aoScore === 0 ? "0.0 (DQ)" : aoScore.toFixed(1)}
              </span>
            </div>
          </div>

          <!-- Steppers Row -->
          <div class="grid grid-cols-4 gap-1.5 shrink-0 my-1">
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("ao", -0.5)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-blue-400 font-mono font-black text-sm border border-blue-500/20 flex items-center justify-center transition"
            >-0.5</button>
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("ao", -0.1)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-blue-400 font-mono font-black text-sm border border-blue-500/20 flex items-center justify-center transition"
            >-0.1</button>
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("ao", 0.1)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-blue-400 font-mono font-black text-sm border border-blue-500/20 flex items-center justify-center transition"
            >+0.1</button>
            <button
              disabled={isSubmitted}
              onclick={() => adjustScore("ao", 0.5)}
              class="h-10 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] active:scale-95 disabled:opacity-40 text-blue-400 font-mono font-black text-sm border border-blue-500/20 flex items-center justify-center transition"
            >+0.5</button>
          </div>

          <!-- Presets Quick Bar -->
          <div class="grid grid-cols-5 gap-1 shrink-0">
            {#each [0.0, 7.0, 7.5, 8.0, 8.5] as preset}
              <button
                disabled={isSubmitted}
                onclick={() => setPresetScore("ao", preset)}
                class="h-7 rounded-lg text-[10px] font-mono font-bold transition border active:scale-95 disabled:opacity-40 {aoScore === preset ? 'bg-blue-600 text-white border-blue-500 font-black' : 'bg-white/[0.03] text-zinc-400 border-white/[0.06] hover:bg-white/[0.08]'}"
              >
                {preset === 0 ? 'DQ' : preset.toFixed(1)}
              </button>
            {/each}
          </div>
        {:else}
          <!-- Kumite Flags Mode -->
          <div class="flex-1 flex flex-col justify-center">
            <div class="grid grid-cols-3 gap-2">
              <button
                onclick={() => toggleKumiteFlag("ao_yuko")}
                class="h-14 rounded-xl font-black text-xs font-mono transition border active:scale-95 {activeKumiteFlag === 'ao_yuko' ? 'bg-blue-600 text-white border-white shadow-lg' : 'bg-blue-950/40 text-blue-300 border-blue-500/30'}"
              >
                YUKO (1)
              </button>
              <button
                onclick={() => toggleKumiteFlag("ao_wazaari")}
                class="h-14 rounded-xl font-black text-xs font-mono transition border active:scale-95 {activeKumiteFlag === 'ao_wazaari' ? 'bg-blue-600 text-white border-white shadow-lg' : 'bg-blue-950/40 text-blue-300 border-blue-500/30'}"
              >
                WAZA-ARI (2)
              </button>
              <button
                onclick={() => toggleKumiteFlag("ao_ippon")}
                class="h-14 rounded-xl font-black text-xs font-mono transition border active:scale-95 {activeKumiteFlag === 'ao_ippon' ? 'bg-blue-600 text-white border-white shadow-lg' : 'bg-blue-950/40 text-blue-300 border-blue-500/30'}"
              >
                IPPON (3)
              </button>
            </div>
          </div>
        {/if}
      </div>
    </section>

  </main>

  <!-- Sticky Bottom Submission Bar -->
  <footer class="bg-[#14161b] border-t border-white/10 p-3 shrink-0 shadow-2xl">
    {#if !isSubmitted}
      <button
        onclick={submitKataScores}
        disabled={isSending}
        class="w-full h-12 bg-white text-black hover:bg-zinc-200 active:scale-[0.98] font-black text-sm rounded-xl shadow-xl flex items-center justify-center gap-2 transition"
      >
        {#if isSending}
          <svg class="animate-spin h-5 w-5 text-black" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          <span>MENGIRIM NILAI...</span>
        {:else}
          <svg class="w-5 h-5 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
          </svg>
          <span>KIRIM NILAI WASIT #{selectedJudge}</span>
        {/if}
      </button>
    {:else}
      <!-- Locked State with Correction Option -->
      <div class="flex items-center gap-2">
        <div class="flex-1 h-12 bg-emerald-500/15 border-2 border-emerald-500/40 rounded-xl flex items-center justify-center gap-2 text-emerald-400 font-mono font-black text-xs">
          <svg class="w-4 h-4 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
          </svg>
          <span>NILAI JURI #{selectedJudge} TERKIRIM (TERKUNCI)</span>
        </div>
        <button
          onclick={unlockScores}
          class="h-12 px-4 rounded-xl bg-white/[0.06] hover:bg-white/[0.12] text-zinc-300 border border-white/15 text-xs font-bold transition active:scale-95 shrink-0"
        >
          Koreksi
        </button>
      </div>
    {/if}
  </footer>

</div>
