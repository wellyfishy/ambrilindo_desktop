<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import { listen, emit } from "@tauri-apps/api/event";

  // Tatami number from query string, defaults to 1
  let tatamiNumber = $derived(Number(page.url.searchParams.get("tatami") || "1"));
  let isLocalSyncConnected = $state(false);

  // Match information state
  let eventName = $state("AMBRILINDO KARATE CHAMPIONSHIP");
  let categoryName = $state("KATA PERORANGAN SENIOR PUTRA");
  let matchRound = $state("BABAK PENYISIHAN");
  let matchNumber = $state("1");
  let isKata = $state(true);
  let matchWinner = $state("");
  let isMatchFinished = $state(false);
  let showKataScoreOverlay = $state(false);

  // Timer state (with high-precision milliseconds)
  let timerMs = $state(120000);
  let isTimerRunning = $state(false);
  let animFrameId: number | null = null;
  let lastTickTimestamp: number = 0;

  // AKA (Red) combatant state
  let akaName = $state("SUDUT MERAH (AKA)");
  let akaClub = $state("KONTINGEN A");
  let akaKataName = $state("");
  let akaTotalPoints = $state(0);
  let akaJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let akaScore = $state(0);
  let akaSenshu = $state(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // AO (Blue) combatant state
  let aoName = $state("SUDUT BIRU (AO)");
  let aoClub = $state("KONTINGEN B");
  let aoKataName = $state("");
  let aoTotalPoints = $state(0);
  let aoJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let aoScore = $state(0);
  let aoSenshu = $state(false);
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Formatting minutes:seconds and milliseconds
  let timerFormatted = $derived.by(() => {
    const safeMs = Math.max(0, Math.floor(timerMs));
    const totalSec = Math.floor(safeMs / 1000);
    const mins = Math.floor(totalSec / 60);
    const secs = totalSec % 60;
    const hundredths = Math.floor((safeMs % 1000) / 10);
    const mStr = mins < 10 ? `0${mins}` : `${mins}`;
    const sStr = secs < 10 ? `0${secs}` : `${secs}`;
    const msStr = hundredths < 10 ? `0${hundredths}` : `${hundredths}`;
    return {
      mmss: `${mStr}:${sStr}`,
      msStr,
      full: `${mStr}:${sStr}.${msStr}`,
    };
  });

  function runScoreboardTimerLoop() {
    if (!isTimerRunning) return;
    const now = performance.now();
    const delta = now - lastTickTimestamp;
    lastTickTimestamp = now;

    timerMs = Math.max(0, timerMs - delta);
    if (timerMs <= 0) {
      timerMs = 0;
      isTimerRunning = false;
      return;
    }

    animFrameId = requestAnimationFrame(runScoreboardTimerLoop);
  }

  // Calculate judge highlights for 5-judge table
  let judgeDecisions = $derived.by(() => {
    return [0, 1, 2, 3, 4].map((idx) => {
      const rawAka = akaJudgeScores[idx]?.trim();
      const rawAo = aoJudgeScores[idx]?.trim();
      const vAka = rawAka !== "" && rawAka !== undefined ? parseFloat(rawAka) : NaN;
      const vAo = rawAo !== "" && rawAo !== undefined ? parseFloat(rawAo) : NaN;

      const hasAka = !isNaN(vAka) && ((vAka >= 5.0 && vAka <= 10.0) || vAka === 0);
      const hasAo = !isNaN(vAo) && ((vAo >= 5.0 && vAo <= 10.0) || vAo === 0);

      if (hasAka && hasAo) {
        if (vAka === 0 && vAo > 0) return "ao";
        if (vAo === 0 && vAka > 0) return "aka";
        if (vAka > vAo) return "aka";
        if (vAo > vAka) return "ao";
        return "seri";
      }
      return "empty";
    });
  });

  function applyLiveState(data: any) {
    if (!data) return;
    isLocalSyncConnected = true;
    if (data.category) categoryName = data.category;
    if (data.round) matchRound = data.round;
    if (data.matchNumber !== undefined) matchNumber = String(data.matchNumber);
    
    if (data.timerMs !== undefined) {
      if (isTimerRunning) {
        if (Math.abs(timerMs - data.timerMs) > 150) {
          timerMs = data.timerMs;
        }
      } else {
        timerMs = data.timerMs;
      }
    } else if (data.timerSeconds !== undefined) {
      if (!isTimerRunning) {
        timerMs = data.timerSeconds * 1000;
      }
    }

    if (data.isTimerRunning !== undefined) {
      const wasRunning = isTimerRunning;
      isTimerRunning = data.isTimerRunning;
      if (!wasRunning && isTimerRunning) {
        lastTickTimestamp = performance.now();
        animFrameId = requestAnimationFrame(runScoreboardTimerLoop);
      } else if (wasRunning && !isTimerRunning) {
        if (animFrameId) cancelAnimationFrame(animFrameId);
        animFrameId = null;
      }
    }

    if (data.isKata !== undefined) isKata = data.isKata;
    if (data.winner !== undefined) matchWinner = data.winner;
    if (data.isFinished !== undefined) isMatchFinished = data.isFinished;
    if (data.showKataScoreOverlay !== undefined) showKataScoreOverlay = data.showKataScoreOverlay;

    if (data.aka) {
      if (data.aka.name) akaName = data.aka.name;
      if (data.aka.contingent) akaClub = data.aka.contingent;
      if (data.aka.kataName !== undefined) akaKataName = data.aka.kataName;
      if (data.aka.totalPoints !== undefined) akaTotalPoints = data.aka.totalPoints;
      if (data.aka.judgeScores) akaJudgeScores = data.aka.judgeScores;
      if (data.aka.score !== undefined) akaScore = data.aka.score;
      if (data.aka.senshu !== undefined) akaSenshu = data.aka.senshu;
      if (data.aka.penalties) akaPenalties = { ...akaPenalties, ...data.aka.penalties };
    }

    if (data.ao) {
      if (data.ao.name) aoName = data.ao.name;
      if (data.ao.contingent) aoClub = data.ao.contingent;
      if (data.ao.kataName !== undefined) aoKataName = data.ao.kataName;
      if (data.ao.totalPoints !== undefined) aoTotalPoints = data.ao.totalPoints;
      if (data.ao.judgeScores) aoJudgeScores = data.ao.judgeScores;
      if (data.ao.score !== undefined) aoScore = data.ao.score;
      if (data.ao.senshu !== undefined) aoSenshu = data.ao.senshu;
      if (data.ao.penalties) aoPenalties = { ...aoPenalties, ...data.ao.penalties };
    }

    isLocalSyncConnected = true;
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let bc: BroadcastChannel | null = null;

    // 1. Listen to local realtime desktop IPC event
    listen("tatami_live_state", (event: any) => {
      const payload = event.payload;
      if (payload && (payload.tatami === tatamiNumber || !payload.tatami)) {
        applyLiveState(payload);
      }
    }).then((unsub) => {
      unlisten = unsub;
    }).catch((err) => {
      console.warn("Failed to listen to tatami_live_state:", err);
    });

    // 2. BroadcastChannel fallback for multi-window web / dev mode
    try {
      bc = new BroadcastChannel("ambrilindo_live_sync");
      bc.onmessage = (event) => {
        const payload = event.data;
        if (payload && (payload.tatami === tatamiNumber || !payload.tatami)) {
          applyLiveState(payload);
        }
      };
    } catch (e) {
      console.warn("BroadcastChannel not available:", e);
    }

    // Request current state immediately from open control panel
    emit("request_tatami_state", { tatami: tatamiNumber }).catch(() => {});

    return () => {
      if (unlisten) unlisten();
      if (bc) bc.close();
      if (animFrameId) cancelAnimationFrame(animFrameId);
    };
  });
</script>

<div class="h-screen w-screen bg-[#06090e] text-white flex flex-col justify-between overflow-hidden select-none font-sans">
  <!-- Top Match Banner Bar with Scrolling Marquee Ticker -->
  <header class="bg-[#0b0e14] border-b-2 border-white/10 px-6 py-2.5 flex items-center justify-between shadow-2xl shrink-0 gap-4 overflow-hidden">
    <!-- Tatami Badge -->
    <div class="bg-gradient-to-r from-amber-400 to-amber-600 text-black font-black px-6 py-2 rounded-xl text-2xl 2xl:text-3xl font-mono tracking-wider shadow-lg shrink-0">
      TATAMI {tatamiNumber}
    </div>

    <!-- Scrolling Marquee Ticker (Ambrilindo Local Broadcast Standard) -->
    <div class="flex-1 min-w-0 overflow-hidden relative mx-2">
      <!-- svelte-ignore a11y_distracting_elements -->
      <marquee behavior="scroll" direction="left" scrollamount="11" class="w-full font-mono text-xl 2xl:text-2xl font-black text-amber-300 tracking-wider uppercase leading-tight">
        <span class="text-white mx-3">•</span>
        <span>{categoryName}</span>
        <span class="text-white mx-3">•</span>
        <span>{matchRound}</span>
        <span class="text-white mx-3">•</span>
        <span>PARTAI #{matchNumber}</span>
        <span class="text-white mx-3">•</span>
        <span class="text-zinc-200">{eventName}</span>
      </marquee>
    </div>

    <!-- Right Slot: Match Number & Local Live Sync -->
    <div class="flex items-center space-x-4 shrink-0">
      <div class="text-right">
        <span class="text-[10px] uppercase tracking-widest text-zinc-400 block font-bold">PARTAI</span>
        <span class="text-2xl 2xl:text-3xl font-black text-amber-400 font-mono leading-none">#{matchNumber}</span>
      </div>

      <div class="flex items-center space-x-2 bg-[#181a20] px-3.5 py-1.5 rounded-full border border-white/10 shadow-inner">
        <div class="w-2.5 h-2.5 rounded-full {isLocalSyncConnected ? 'bg-emerald-500 animate-pulse' : 'bg-amber-500'}"></div>
        <span class="text-xs font-mono font-bold tracking-wider {isLocalSyncConnected ? 'text-emerald-400' : 'text-amber-400'}">
          {isLocalSyncConnected ? "LIVE SYNC" : "MENUNGGU CP"}
        </span>
      </div>
    </div>
  </header>

  <!-- ==================== ARENA SCORING BOARD (AKA DI ATAS, AO DI BAWAH, FULL-HEIGHT SCALE) ==================== -->
  <main class="flex-1 flex flex-col justify-between p-3 gap-3 relative min-h-0">
    <!-- TOP CARD: AKA (RED) -->
    <section class="flex-1 min-h-0 bg-gradient-to-r from-red-950/90 via-[#180f12] to-[#0c0e14] border-2 border-red-500/50 rounded-3xl p-6 2xl:p-8 flex items-center justify-between shadow-[0_12px_45px_rgba(0,0,0,0.85)] relative overflow-hidden">
      <!-- Red top accent neon line -->
      <div class="absolute top-0 left-0 right-0 h-2 bg-gradient-to-r from-red-500 via-red-600 to-transparent"></div>

      <div class="flex items-center flex-1 min-w-0 pr-6">
        <!-- Flag / Delegation Logo Box -->
        <div class="w-28 h-28 md:w-36 md:h-36 2xl:w-44 2xl:h-44 bg-black/60 border-2 border-red-500/40 rounded-2xl flex items-center justify-center p-2.5 shrink-0 mr-6 shadow-inner">
          <svg class="w-16 h-16 md:w-20 md:h-20 2xl:w-24 2xl:h-24 text-red-500 drop-shadow-[0_0_12px_rgba(239,68,68,0.7)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" />
          </svg>
        </div>

        <!-- Competitor Details (Giant Arena Scale) -->
        <div class="flex-1 min-w-0">
          {#if matchWinner === "1"}
            <div class="inline-flex items-center gap-1.5 mb-2 px-4 py-1 bg-gradient-to-r from-amber-400 to-amber-500 text-slate-950 text-xs 2xl:text-sm font-black uppercase rounded-lg shadow-[0_0_20px_rgba(245,158,11,0.8)] animate-pulse">
              <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" /></svg>
              <span>PEMENANG</span>
            </div>
          {/if}

          <!-- Athlete Name (Big enough to read from 20 meters on 27" TV) -->
          <h2 class="text-5xl md:text-6xl 2xl:text-7xl 3xl:text-8xl font-black uppercase text-white tracking-tight leading-none drop-shadow-[0_4px_18px_rgba(0,0,0,0.95)] break-words">
            {akaName}
          </h2>

          <!-- Perguruan & Kontingen (Full, No Ellipsis) -->
          <p class="text-2xl md:text-3xl 2xl:text-4xl font-bold text-red-100 mt-2.5 uppercase tracking-wide leading-snug break-words drop-shadow-md">
            {akaClub}
          </p>

          <!-- Discipline Specific Info -->
          {#if isKata}
            {#if akaKataName}
              <div class="inline-flex items-center gap-2 mt-3 px-4 py-1.5 bg-black/60 border border-amber-500/50 rounded-xl text-amber-300 font-mono font-black text-xl 2xl:text-2xl tracking-wider shadow">
                <span class="text-zinc-400 font-bold text-sm">KATA:</span>
                <span>{akaKataName}</span>
              </div>
            {/if}
          {:else}
            <!-- Kumite Criteria Penalties -->
            <div class="flex items-center gap-2 mt-3">
              <span class="text-xs font-mono font-bold text-red-300 uppercase tracking-wider mr-1">PELANGGARAN:</span>
              {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
                <span class="px-3 py-1 rounded-lg font-mono font-black text-sm uppercase border transition-all {akaPenalties[penalty as keyof typeof akaPenalties] ? 'bg-amber-400 text-slate-950 border-amber-300 shadow-[0_0_15px_rgba(251,191,36,0.8)] font-bold scale-105' : 'bg-black/50 text-zinc-500 border-white/10'}">
                  {penalty.toUpperCase()}
                </span>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Score Pod (Right) -->
      <div class="shrink-0 flex flex-col items-center justify-center pl-6 border-l border-white/10 relative">
        {#if !isKata && akaSenshu}
          <span class="absolute -top-3.5 -right-2 bg-gradient-to-r from-amber-400 to-amber-500 text-slate-950 font-mono font-black text-xs 2xl:text-sm px-3 py-1 rounded-lg shadow-[0_0_20px_rgba(245,158,11,0.9)] uppercase tracking-wider -rotate-2 border-2 border-white animate-bounce z-20">
            SENSHU ★
          </span>
        {/if}
        <span class="text-xs 2xl:text-sm font-mono font-bold text-red-300 uppercase tracking-widest mb-1.5">
          {isKata ? "TOTAL BENDERA" : "POIN KUMITE"}
        </span>
        <div class="min-w-[190px] 2xl:min-w-[260px] h-[160px] 2xl:h-[220px] bg-black/70 border-4 {isKata ? (akaTotalPoints > aoTotalPoints ? 'border-red-500 shadow-[0_0_40px_rgba(239,68,68,0.5)]' : 'border-[#2b2d31]') : (akaScore > aoScore ? 'border-red-500 shadow-[0_0_40px_rgba(239,68,68,0.5)]' : 'border-[#2b2d31]')} rounded-3xl relative flex items-center justify-center shadow-inner">
          <span class="font-mono font-black text-8xl md:text-9xl 2xl:text-[12rem] text-white leading-none drop-shadow-[0_0_35px_rgba(239,68,68,0.85)]">
            {isKata ? akaTotalPoints : akaScore}
          </span>
        </div>
      </div>
    </section>

    <!-- BOTTOM CARD: AO (BLUE) -->
    <section class="flex-1 min-h-0 bg-gradient-to-r from-blue-950/90 via-[#0e1422] to-[#0c0e14] border-2 border-blue-500/50 rounded-3xl p-6 2xl:p-8 flex items-center justify-between shadow-[0_12px_45px_rgba(0,0,0,0.85)] relative overflow-hidden">
      <!-- Blue bottom accent neon line -->
      <div class="absolute bottom-0 left-0 right-0 h-2 bg-gradient-to-r from-blue-500 via-blue-600 to-transparent"></div>

      <div class="flex items-center flex-1 min-w-0 pr-6">
        <!-- Flag / Delegation Logo Box -->
        <div class="w-28 h-28 md:w-36 md:h-36 2xl:w-44 2xl:h-44 bg-black/60 border-2 border-blue-500/40 rounded-2xl flex items-center justify-center p-2.5 shrink-0 mr-6 shadow-inner">
          <svg class="w-16 h-16 md:w-20 md:h-20 2xl:w-24 2xl:h-24 text-blue-500 drop-shadow-[0_0_12px_rgba(59,130,246,0.7)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" />
          </svg>
        </div>

        <!-- Competitor Details (Giant Arena Scale) -->
        <div class="flex-1 min-w-0">
          {#if matchWinner === "2"}
            <div class="inline-flex items-center gap-1.5 mb-2 px-4 py-1 bg-gradient-to-r from-amber-400 to-amber-500 text-slate-950 text-xs 2xl:text-sm font-black uppercase rounded-lg shadow-[0_0_20px_rgba(245,158,11,0.8)] animate-pulse">
              <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" /></svg>
              <span>PEMENANG</span>
            </div>
          {/if}

          <!-- Athlete Name (Big enough to read from 20 meters on 27" TV) -->
          <h2 class="text-5xl md:text-6xl 2xl:text-7xl 3xl:text-8xl font-black uppercase text-white tracking-tight leading-none drop-shadow-[0_4px_18px_rgba(0,0,0,0.95)] break-words">
            {aoName}
          </h2>

          <!-- Perguruan & Kontingen (Full, No Ellipsis) -->
          <p class="text-2xl md:text-3xl 2xl:text-4xl font-bold text-blue-100 mt-2.5 uppercase tracking-wide leading-snug break-words drop-shadow-md">
            {aoClub}
          </p>

          <!-- Discipline Specific Info -->
          {#if isKata}
            {#if aoKataName}
              <div class="inline-flex items-center gap-2 mt-3 px-4 py-1.5 bg-black/60 border border-amber-500/50 rounded-xl text-amber-300 font-mono font-black text-xl 2xl:text-2xl tracking-wider shadow">
                <span class="text-zinc-400 font-bold text-sm">KATA:</span>
                <span>{aoKataName}</span>
              </div>
            {/if}
          {:else}
            <!-- Kumite Criteria Penalties -->
            <div class="flex items-center gap-2 mt-3">
              <span class="text-xs font-mono font-bold text-blue-300 uppercase tracking-wider mr-1">PELANGGARAN:</span>
              {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
                <span class="px-3 py-1 rounded-lg font-mono font-black text-sm uppercase border transition-all {aoPenalties[penalty as keyof typeof aoPenalties] ? 'bg-amber-400 text-slate-950 border-amber-300 shadow-[0_0_15px_rgba(251,191,36,0.8)] font-bold scale-105' : 'bg-black/50 text-zinc-500 border-white/10'}">
                  {penalty.toUpperCase()}
                </span>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Score Pod (Right) -->
      <div class="shrink-0 flex flex-col items-center justify-center pl-6 border-l border-white/10 relative">
        {#if !isKata && aoSenshu}
          <span class="absolute -top-3.5 -right-2 bg-gradient-to-r from-amber-400 to-amber-500 text-slate-950 font-mono font-black text-xs 2xl:text-sm px-3 py-1 rounded-lg shadow-[0_0_20px_rgba(245,158,11,0.9)] uppercase tracking-wider -rotate-2 border-2 border-white animate-bounce z-20">
            SENSHU ★
          </span>
        {/if}
        <span class="text-xs 2xl:text-sm font-mono font-bold text-blue-300 uppercase tracking-widest mb-1.5">
          {isKata ? "TOTAL BENDERA" : "POIN KUMITE"}
        </span>
        <div class="min-w-[190px] 2xl:min-w-[260px] h-[160px] 2xl:h-[220px] bg-black/70 border-4 {isKata ? (aoTotalPoints > akaTotalPoints ? 'border-blue-500 shadow-[0_0_40px_rgba(59,130,246,0.5)]' : 'border-[#2b2d31]') : (aoScore > akaScore ? 'border-blue-500 shadow-[0_0_40px_rgba(59,130,246,0.5)]' : 'border-[#2b2d31]')} rounded-3xl relative flex items-center justify-center shadow-inner">
          <span class="font-mono font-black text-8xl md:text-9xl 2xl:text-[12rem] text-white leading-none drop-shadow-[0_0_35px_rgba(59,130,246,0.85)]">
            {isKata ? aoTotalPoints : aoScore}
          </span>
        </div>
      </div>
    </section>
  </main>

  <!-- ==================== KATA 5-JUDGE SCOREBOARD TABLE OVERLAY (TOGGLEABLE DARI CONTROL PANEL) ==================== -->
  {#if isKata && showKataScoreOverlay}
    <div
      class="fixed inset-0 z-50 bg-black/85 backdrop-blur-md flex items-center justify-center p-6 animate-in fade-in zoom-in-95 duration-200"
      role="dialog"
      aria-modal="true"
      aria-label="Tabel Penilaian 5 Juri Kata"
    >
      <div class="bg-[#0f141f] border-4 border-amber-500/70 rounded-3xl p-8 shadow-2xl max-w-5xl w-full text-center relative overflow-hidden">
        <!-- Header -->
        <div class="flex items-center justify-between pb-4 mb-6 border-b border-white/10">
          <div class="flex items-center gap-3">
            <span class="w-3 h-3 rounded-full bg-amber-400 animate-pulse"></span>
            <h3 class="font-mono font-black text-2xl 2xl:text-3xl text-amber-300 uppercase tracking-wider">
              PENILAIAN JURI KATA (5 JURI)
            </h3>
          </div>
          <span class="text-xs font-mono font-bold text-zinc-400 tracking-wider uppercase bg-[#181a20] px-4 py-1.5 rounded-full border border-white/10">
            5 JUDGES COMPARISON
          </span>
        </div>

        <!-- 5 Judge Columns -->
        <div class="grid grid-cols-5 gap-4 mb-6">
          {#each [0, 1, 2, 3, 4] as idx}
            {@const decision = judgeDecisions[idx]}
            {@const sAka = akaJudgeScores[idx] || "0.0"}
            {@const sAo = aoJudgeScores[idx] || "0.0"}
            <div class="bg-[#181a20] border-2 {decision === 'aka' ? 'border-red-500 shadow-[0_0_20px_rgba(239,68,68,0.4)]' : decision === 'ao' ? 'border-blue-500 shadow-[0_0_20px_rgba(59,130,246,0.4)]' : 'border-[#2b2d31]'} rounded-2xl p-3 flex flex-col justify-between transition-all">
              <!-- AKA Judge Score -->
              <div class="py-2.5 rounded-xl font-mono font-black text-3xl 2xl:text-4xl transition-all {decision === 'aka' ? 'bg-red-600 text-white shadow-md border-2 border-red-300' : 'bg-[#111216] text-red-300/60 border border-red-500/20'}">
                {sAka}
              </div>

              <!-- Judge Label Badge -->
              <div class="my-2.5 flex items-center justify-center gap-1 font-mono font-bold text-sm 2xl:text-base {decision === 'aka' ? 'text-red-400' : decision === 'ao' ? 'text-blue-400' : 'text-zinc-500'}">
                <span>JURI {idx + 1}</span>
                {#if decision === "aka"}
                  <span class="text-xs font-black">▲ AKA</span>
                {:else if decision === "ao"}
                  <span class="text-xs font-black">▼ AO</span>
                {/if}
              </div>

              <!-- AO Judge Score -->
              <div class="py-2.5 rounded-xl font-mono font-black text-3xl 2xl:text-4xl transition-all {decision === 'ao' ? 'bg-blue-600 text-white shadow-md border-2 border-blue-300' : 'bg-[#111216] text-blue-300/60 border border-blue-500/20'}">
                {sAo}
              </div>
            </div>
          {/each}
        </div>

        <!-- Total Flags Comparison Summary -->
        <div class="flex items-center justify-around pt-5 border-t border-white/10 font-mono text-2xl 2xl:text-3xl font-black">
          <span class="text-red-400 drop-shadow-[0_0_15px_rgba(239,68,68,0.6)]">AKA: {akaTotalPoints} BENDERA</span>
          <span class="text-zinc-500">•</span>
          <span class="text-blue-400 drop-shadow-[0_0_15px_rgba(59,130,246,0.6)]">AO: {aoTotalPoints} BENDERA</span>
        </div>
      </div>
    </div>
  {/if}

  <!-- ==================== BOTTOM MATCH TIMER & ARENA STATUS FOOTER BAR (BROADCAST GIANT SCALE) ==================== -->
  <footer class="bg-[#0b0e14] border-t-2 border-white/10 px-8 py-3 flex items-center justify-between shadow-2xl shrink-0 gap-6">
    <div class="flex items-center space-x-3 shrink-0">
      <span class="text-xs 2xl:text-sm uppercase font-mono font-bold text-zinc-400 tracking-wider">AMBRILINDO ARENA BROADCAST</span>
    </div>

    <!-- Center Giant Digital Timer Display (Visible 20m away) -->
    <div class="flex items-center space-x-6">
      <div class="bg-[#111216] border-2 border-amber-500/60 px-8 py-2 rounded-2xl flex items-baseline space-x-2 shadow-[inset_0_0_25px_rgba(0,0,0,0.8)]">
        <span class="font-mono font-black text-6xl md:text-7xl 2xl:text-8xl 3xl:text-9xl {isTimerRunning ? 'text-amber-300 drop-shadow-[0_0_30px_rgba(251,191,36,0.7)]' : 'text-white drop-shadow-md'} tracking-wider tabular-nums leading-none">
          {timerFormatted.mmss}
        </span>
        <span class="font-mono text-xl 2xl:text-3xl font-bold {isTimerRunning ? 'text-amber-400' : 'text-zinc-400'}">
          .{timerFormatted.msStr}
        </span>
      </div>

      <div class="py-2.5 px-4 rounded-xl text-xs 2xl:text-sm font-mono font-black tracking-wider uppercase border {isTimerRunning ? 'bg-emerald-950 text-emerald-400 border-emerald-500/50 shadow-[0_0_20px_rgba(16,185,129,0.3)]' : 'bg-zinc-800 text-zinc-400 border-zinc-700'}">
        {isTimerRunning ? "BERLANGSUNG" : "BERHENTI (YAME)"}
      </div>
    </div>

    <div class="flex items-center space-x-2 text-xs 2xl:text-sm font-mono text-zinc-400 shrink-0">
      <span>STANDAR WKF 5 JUDGES</span>
    </div>
  </footer>
</div>
