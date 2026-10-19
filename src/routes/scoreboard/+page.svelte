<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";

  // Tatami number from query string, defaults to 1
  let tatamiNumber = $state(1);
  let wsConnected = $state(false);
  let ws: WebSocket | null = null;

  // Match information state
  let eventName = $state("AMBRILINDO KARATE CHAMPIONSHIP");
  let categoryName = $state("KUMITE PERORANGAN PUTRA");
  let matchRound = $state("BABAK PENYISIHAN");
  let matchNumber = $state("1");

  // Timer state
  let timerSeconds = $state(120);
  let isTimerRunning = $state(false);
  let timerInterval: any = null;

  // AKA (Red) combatant state
  let akaName = $state("SUDUT MERAH (AKA)");
  let akaClub = $state("KONTINGEN A");
  let akaScore = $state(0);
  let akaSenshu = $state(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // AO (Blue) combatant state
  let aoName = $state("SUDUT BIRU (AO)");
  let aoClub = $state("KONTINGEN B");
  let aoScore = $state(0);
  let aoSenshu = $state(false);
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Formatting minutes:seconds
  let formattedTime = $derived.by(() => {
    const mins = Math.floor(timerSeconds / 60);
    const secs = timerSeconds % 60;
    return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  });

  // Is last 15 seconds warning active?
  let isWarningTime = $derived(timerSeconds <= 15 && timerSeconds > 0);

  function connectWebSocket() {
    const host = window.location.hostname || "127.0.0.1";
    const port = 8000;
    const wsUrl = `ws://${host}:${port}/ws/scoring-board/${tatamiNumber}`;

    try {
      ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        wsConnected = true;
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          handleArenaMessage(data);
        } catch (err) {
          console.error("WS Parse Error:", err);
        }
      };

      ws.onclose = () => {
        wsConnected = false;
        // Auto-reconnect after 2 seconds
        setTimeout(connectWebSocket, 2000);
      };

      ws.onerror = () => {
        wsConnected = false;
        ws?.close();
      };
    } catch (e) {
      setTimeout(connectWebSocket, 3000);
    }
  }

  function handleArenaMessage(msg: any) {
    if (msg.event_name) eventName = msg.event_name;
    if (msg.category) categoryName = msg.category;
    if (msg.round) matchRound = msg.round;
    if (msg.match_num) matchNumber = msg.match_num;

    if (msg.aka) {
      if (msg.aka.name !== undefined) akaName = msg.aka.name;
      if (msg.aka.club !== undefined) akaClub = msg.aka.club;
      if (msg.aka.score !== undefined) akaScore = msg.aka.score;
      if (msg.aka.senshu !== undefined) akaSenshu = msg.aka.senshu;
      if (msg.aka.penalties) akaPenalties = { ...akaPenalties, ...msg.aka.penalties };
    }

    if (msg.ao) {
      if (msg.ao.name !== undefined) aoName = msg.ao.name;
      if (msg.ao.club !== undefined) aoClub = msg.ao.club;
      if (msg.ao.score !== undefined) aoScore = msg.ao.score;
      if (msg.ao.senshu !== undefined) aoSenshu = msg.ao.senshu;
      if (msg.ao.penalties) aoPenalties = { ...aoPenalties, ...msg.ao.penalties };
    }

    if (msg.timer !== undefined) {
      timerSeconds = msg.timer;
    }
    if (msg.timer_running !== undefined) {
      isTimerRunning = msg.timer_running;
      if (isTimerRunning) startLocalTimer();
      else stopLocalTimer();
    }
  }

  function startLocalTimer() {
    stopLocalTimer();
    timerInterval = setInterval(() => {
      if (timerSeconds > 0) {
        timerSeconds -= 1;
      } else {
        stopLocalTimer();
      }
    }, 1000);
  }

  function stopLocalTimer() {
    if (timerInterval) {
      clearInterval(timerInterval);
      timerInterval = null;
    }
  }

  onMount(() => {
    const param = page.url.searchParams.get("tatami");
    if (param) {
      tatamiNumber = parseInt(param, 10) || 1;
    }
    connectWebSocket();
  });

  onDestroy(() => {
    stopLocalTimer();
    if (ws) {
      ws.close();
    }
  });
</script>

<svelte:head>
  <title>Scoreboard Tatami {tatamiNumber} — Ambrilindo</title>
</svelte:head>

<div class="h-screen w-screen bg-slate-950 text-white flex flex-col justify-between overflow-hidden select-none font-sans">
  <!-- Top Match Banner -->
  <header class="bg-slate-900 border-b-4 border-slate-800 px-8 py-3 flex items-center justify-between shadow-2xl">
    <div class="flex items-center space-x-4">
      <div class="bg-amber-500 text-slate-950 font-black px-4 py-1.5 rounded-lg text-2xl tracking-wider shadow">
        TATAMI {tatamiNumber}
      </div>
      <div>
        <h1 class="text-xl font-black tracking-wide text-slate-100 uppercase">{eventName}</h1>
        <p class="text-sm font-bold text-amber-400 tracking-wider uppercase">{categoryName} • {matchRound}</p>
      </div>
    </div>

    <div class="flex items-center space-x-6">
      <div class="text-right">
        <span class="text-xs uppercase tracking-widest text-slate-400 block font-semibold">PARTAI</span>
        <span class="text-2xl font-black text-amber-400">#{matchNumber}</span>
      </div>

      <div class="flex items-center space-x-2 bg-slate-950/80 px-3 py-1.5 rounded-full border border-slate-700">
        <div class="w-3 h-3 rounded-full {wsConnected ? 'bg-emerald-500 animate-pulse' : 'bg-red-500'}"></div>
        <span class="text-xs font-bold tracking-wider {wsConnected ? 'text-emerald-400' : 'text-red-400'}">
          {wsConnected ? 'LIVE SYNC' : 'OFFLINE'}
        </span>
      </div>
    </div>
  </header>

  <!-- Central Arena Scoreboard Grid -->
  <main class="flex-1 grid grid-cols-12 gap-0 relative">
    <!-- AKA (Red) Side - 5 Columns -->
    <section class="col-span-5 bg-gradient-to-br from-red-700 via-red-800 to-red-950 p-6 flex flex-col justify-between border-r-4 border-slate-950 shadow-inner">
      <div>
        <div class="flex items-center justify-between mb-2">
          <span class="bg-red-950/80 text-red-200 border border-red-500/40 text-xs font-black tracking-widest px-3 py-1 rounded uppercase">
            AKA (MERAH)
          </span>
          {#if akaSenshu}
            <span class="bg-amber-400 text-slate-950 text-xs font-black px-3 py-1 rounded shadow-lg uppercase tracking-wider animate-bounce">
              SENSHU ★
            </span>
          {/if}
        </div>
        <h2 class="text-4xl 2xl:text-5xl font-black tracking-tight text-white leading-tight uppercase truncate drop-shadow-md">
          {akaName}
        </h2>
        <p class="text-xl 2xl:text-2xl font-bold text-red-200 uppercase truncate mt-1 tracking-wide">
          {akaClub}
        </p>
      </div>

      <!-- AKA Huge Score Display -->
      <div class="my-auto flex justify-center items-center py-4">
        <span class="text-[14rem] 2xl:text-[18rem] leading-none font-black text-white tabular-nums drop-shadow-[0_10px_20px_rgba(0,0,0,0.8)]">
          {akaScore}
        </span>
      </div>

      <!-- AKA Penalties (C1, C2, C3, HC, H) -->
      <div class="bg-red-950/60 rounded-xl p-4 border border-red-500/30">
        <span class="text-xs uppercase font-extrabold tracking-widest text-red-300 block mb-2">PELANGGARAN (PENALTIES)</span>
        <div class="grid grid-cols-5 gap-2">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            <div class="py-2 text-center rounded font-black text-sm tracking-wider uppercase border transition-all {akaPenalties[penalty as keyof typeof akaPenalties] ? 'bg-amber-400 text-slate-950 border-amber-300 shadow-md scale-105' : 'bg-slate-900/60 text-slate-500 border-slate-700'}">
              {penalty.toUpperCase()}
            </div>
          {/each}
        </div>
      </div>
    </section>

    <!-- Center Timer & Arena Control - 2 Columns -->
    <section class="col-span-2 bg-slate-950 flex flex-col justify-between items-center py-6 px-3 border-x-4 border-slate-800 z-10 shadow-2xl">
      <!-- Top Indicator -->
      <div class="w-full text-center">
        <span class="text-xs font-black uppercase tracking-widest text-slate-400 block mb-1">STATUS PERTANDINGAN</span>
        <div class="py-1 px-3 rounded-lg text-xs font-black tracking-wider uppercase border {isTimerRunning ? 'bg-emerald-950 text-emerald-400 border-emerald-600' : 'bg-slate-900 text-slate-400 border-slate-700'}">
          {isTimerRunning ? 'BERLANGSUNG' : 'BERHENTI (YAME)'}
        </div>
      </div>

      <!-- Digital Match Timer -->
      <div class="my-auto w-full text-center">
        <div class="bg-black/90 border-4 {isWarningTime ? 'border-amber-400 shadow-[0_0_40px_rgba(251,191,36,0.6)] animate-pulse' : 'border-slate-700'} rounded-2xl py-6 px-2">
          <span class="text-xs uppercase font-black tracking-widest {isWarningTime ? 'text-amber-400' : 'text-slate-400'} block mb-2">
            {isWarningTime ? 'ATOSHI BARAKU (15s)' : 'WAKTU PERTANDINGAN'}
          </span>
          <div class="text-6xl 2xl:text-7xl font-mono font-black tabular-nums tracking-tighter {isWarningTime ? 'text-amber-400' : 'text-slate-100'}">
            {formattedTime}
          </div>
        </div>
      </div>

      <!-- Bottom Info -->
      <div class="text-center">
        <span class="text-[10px] uppercase font-bold text-slate-500 tracking-wider">AMBRILINDO LOCAL v3.0</span>
      </div>
    </section>

    <!-- AO (Blue) Side - 5 Columns -->
    <section class="col-span-5 bg-gradient-to-bl from-blue-700 via-blue-800 to-blue-950 p-6 flex flex-col justify-between border-l-4 border-slate-950 shadow-inner">
      <div>
        <div class="flex items-center justify-between mb-2">
          <span class="bg-blue-950/80 text-blue-200 border border-blue-500/40 text-xs font-black tracking-widest px-3 py-1 rounded uppercase">
            AO (BIRU)
          </span>
          {#if aoSenshu}
            <span class="bg-amber-400 text-slate-950 text-xs font-black px-3 py-1 rounded shadow-lg uppercase tracking-wider animate-bounce">
              SENSHU ★
            </span>
          {/if}
        </div>
        <h2 class="text-4xl 2xl:text-5xl font-black tracking-tight text-white leading-tight uppercase truncate drop-shadow-md">
          {aoName}
        </h2>
        <p class="text-xl 2xl:text-2xl font-bold text-blue-200 uppercase truncate mt-1 tracking-wide">
          {aoClub}
        </p>
      </div>

      <!-- AO Huge Score Display -->
      <div class="my-auto flex justify-center items-center py-4">
        <span class="text-[14rem] 2xl:text-[18rem] leading-none font-black text-white tabular-nums drop-shadow-[0_10px_20px_rgba(0,0,0,0.8)]">
          {aoScore}
        </span>
      </div>

      <!-- AO Penalties (C1, C2, C3, HC, H) -->
      <div class="bg-blue-950/60 rounded-xl p-4 border border-blue-500/30">
        <span class="text-xs uppercase font-extrabold tracking-widest text-blue-300 block mb-2">PELANGGARAN (PENALTIES)</span>
        <div class="grid grid-cols-5 gap-2">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            <div class="py-2 text-center rounded font-black text-sm tracking-wider uppercase border transition-all {aoPenalties[penalty as keyof typeof aoPenalties] ? 'bg-amber-400 text-slate-950 border-amber-300 shadow-md scale-105' : 'bg-slate-900/60 text-slate-500 border-slate-700'}">
              {penalty.toUpperCase()}
            </div>
          {/each}
        </div>
      </div>
    </section>
  </main>
</div>
