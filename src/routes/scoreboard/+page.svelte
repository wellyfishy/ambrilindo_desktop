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
  let akaLogo = $state<string | null>(null);
  let akaTotalPoints = $state(0);
  let akaJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let akaScore = $state(0);
  let akaSenshu = $state(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // AO (Blue) combatant state
  let aoName = $state("SUDUT BIRU (AO)");
  let aoClub = $state("KONTINGEN B");
  let aoKataName = $state("");
  let aoLogo = $state<string | null>(null);
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
      if (data.aka.logo !== undefined) akaLogo = data.aka.logo;
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
      if (data.ao.logo !== undefined) aoLogo = data.ao.logo;
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
  <header class="bg-[#0b0e14] border-b-2 border-white/10 px-6 py-3 flex items-center justify-between shadow-2xl shrink-0 gap-6 overflow-hidden">
    <!-- Tatami Badge (Way bigger!) -->
    <div class="bg-gradient-to-r from-amber-400 to-amber-600 text-black font-black px-8 py-2.5 rounded-2xl text-3xl md:text-4xl 2xl:text-5xl font-mono tracking-wider shadow-xl shrink-0 select-none">
      TATAMI {tatamiNumber}
    </div>

    <!-- Scrolling Marquee Ticker (Way bigger & takes up full available space) -->
    <div class="flex-1 min-w-0 overflow-hidden relative">
      <!-- svelte-ignore a11y_distracting_elements -->
      <marquee behavior="scroll" direction="left" scrollamount="12" class="w-full font-mono text-2xl md:text-3xl 2xl:text-4xl font-black text-amber-300 tracking-wider uppercase leading-none drop-shadow-[0_0_16px_rgba(251,191,36,0.55)]">
        <span class="text-white mx-4">•</span>
        <span>{categoryName}</span>
        <span class="text-white mx-4">•</span>
        <span>{matchRound}</span>
        <span class="text-white mx-4">•</span>
        <span>PARTAI #{matchNumber}</span>
        <span class="text-white mx-4">•</span>
        <span class="text-zinc-200">{eventName}</span>
      </marquee>
    </div>
  </header>

  <!-- ==================== ARENA SCORING BOARD (AKA DI ATAS, AO DI BAWAH, FULL-HEIGHT SCALE) ==================== -->
  <main class="flex-1 flex flex-col justify-between p-3 gap-3 relative min-h-0">
    <!-- TOP CARD: AKA (RED) -->
    <section class="flex-1 min-h-0 bg-gradient-to-r from-red-950/90 via-[#180f12] to-[#0c0e14] border-2 border-red-500/50 rounded-3xl p-5 2xl:p-7 flex flex-col justify-between shadow-[0_12px_45px_rgba(0,0,0,0.85)] relative overflow-hidden">
      <!-- Red top accent neon line -->
      <div class="absolute top-0 left-0 right-0 h-2 bg-gradient-to-r from-red-500 via-red-600 to-transparent"></div>

      <div class="flex items-center justify-between flex-1 min-h-0">
        <div class="flex items-center flex-1 min-w-0 pr-6">
          <!-- Flag / Delegation Logo Box (Indonesian flag as placeholder if no logo) -->
          <div class="w-36 h-36 md:w-44 md:h-44 2xl:w-52 2xl:h-52 bg-black/60 border-2 border-red-500/40 rounded-2xl flex items-center justify-center p-3 shrink-0 mr-6 shadow-inner overflow-hidden">
            <img
              src={akaLogo || "/img/idn-flag.png"}
              alt="Logo Kontingen / Bendera IDN"
              class="max-w-full max-h-full object-contain rounded-xl drop-shadow-[0_4px_16px_rgba(0,0,0,0.8)]"
              onerror={(e) => { (e.currentTarget as HTMLImageElement).src = "/img/idn-flag.png"; }}
            />
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
            <h2 class="text-6xl md:text-7xl 2xl:text-8xl 3xl:text-9xl font-black uppercase text-white tracking-tight leading-none drop-shadow-[0_4px_24px_rgba(0,0,0,0.95)] break-words">
              {akaName}
            </h2>

            <!-- Perguruan & Kontingen (Full, No Ellipsis) -->
            <p class="text-3xl md:text-4xl 2xl:text-5xl font-extrabold text-red-100 mt-3 uppercase tracking-wide leading-tight break-words drop-shadow-md">
              {akaClub}
            </p>

            <!-- Kata text: ONLY the kata itself, NO "KATA:" label, NO glowing blur, High contrast, BIGGER -->
            {#if isKata && akaKataName}
              <div class="mt-3 text-4xl md:text-5xl 2xl:text-6xl font-black text-amber-400 uppercase tracking-wide drop-shadow-[0_3px_8px_rgba(0,0,0,0.95)]">
                {akaKataName}
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
          <div class="min-w-[190px] 2xl:min-w-[260px] h-[160px] 2xl:h-[220px] bg-black/70 border-4 {isKata ? (akaTotalPoints > aoTotalPoints ? 'border-red-500 shadow-[0_0_40px_rgba(239,68,68,0.5)]' : 'border-[#2b2d31]') : (akaScore > aoScore ? 'border-red-500 shadow-[0_0_40px_rgba(239,68,68,0.5)]' : 'border-[#2b2d31]')} rounded-3xl relative flex items-center justify-center shadow-inner">
            <span class="font-mono font-black text-8xl md:text-9xl 2xl:text-[12rem] text-white leading-none drop-shadow-[0_0_35px_rgba(239,68,68,0.85)]">
              {isKata ? akaTotalPoints : akaScore}
            </span>
          </div>
        </div>
      </div>

      <!-- Kumite Criteria Penalties -->
      {#if !isKata}
        <div class="w-full flex items-center justify-around gap-3 pt-3 mt-2 border-t-2 border-white/10 shrink-0">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            {@const active = akaPenalties[penalty as keyof typeof akaPenalties]}
            <div class="criteria-pill flex-1 py-3 md:py-4 2xl:py-5 rounded-2xl font-mono font-black text-2xl md:text-3xl 2xl:text-4xl text-center uppercase border-2 transition-all shadow-md {active ? 'bg-gradient-to-r from-red-600 to-red-700 text-white border-red-300 shadow-[0_0_25px_rgba(239,68,68,0.95)] scale-[1.02]' : 'bg-black/50 text-white/30 border-white/10'}">
              {penalty.toUpperCase()}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- BOTTOM CARD: AO (BLUE) -->
    <section class="flex-1 min-h-0 bg-gradient-to-r from-blue-950/90 via-[#0e1422] to-[#0c0e14] border-2 border-blue-500/50 rounded-3xl p-5 2xl:p-7 flex flex-col justify-between shadow-[0_12px_45px_rgba(0,0,0,0.85)] relative overflow-hidden">
      <!-- Blue bottom accent neon line -->
      <div class="absolute bottom-0 left-0 right-0 h-2 bg-gradient-to-r from-blue-500 via-blue-600 to-transparent"></div>

      <div class="flex items-center justify-between flex-1 min-h-0">
        <div class="flex items-center flex-1 min-w-0 pr-6">
          <!-- Flag / Delegation Logo Box (Indonesian flag as placeholder if no logo) -->
          <div class="w-36 h-36 md:w-44 md:h-44 2xl:w-52 2xl:h-52 bg-black/60 border-2 border-blue-500/40 rounded-2xl flex items-center justify-center p-3 shrink-0 mr-6 shadow-inner overflow-hidden">
            <img
              src={aoLogo || "/img/idn-flag.png"}
              alt="Logo Kontingen / Bendera IDN"
              class="max-w-full max-h-full object-contain rounded-xl drop-shadow-[0_4px_16px_rgba(0,0,0,0.8)]"
              onerror={(e) => { (e.currentTarget as HTMLImageElement).src = "/img/idn-flag.png"; }}
            />
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
            <h2 class="text-6xl md:text-7xl 2xl:text-8xl 3xl:text-9xl font-black uppercase text-white tracking-tight leading-none drop-shadow-[0_4px_24px_rgba(0,0,0,0.95)] break-words">
              {aoName}
            </h2>

            <!-- Perguruan & Kontingen (Full, No Ellipsis) -->
            <p class="text-3xl md:text-4xl 2xl:text-5xl font-extrabold text-blue-100 mt-3 uppercase tracking-wide leading-tight break-words drop-shadow-md">
              {aoClub}
            </p>

            <!-- Kata text: ONLY the kata itself, NO "KATA:" label, NO glowing blur, High contrast, BIGGER -->
            {#if isKata && aoKataName}
              <div class="mt-3 text-4xl md:text-5xl 2xl:text-6xl font-black text-amber-400 uppercase tracking-wide drop-shadow-[0_3px_8px_rgba(0,0,0,0.95)]">
                {aoKataName}
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
          <div class="min-w-[190px] 2xl:min-w-[260px] h-[160px] 2xl:h-[220px] bg-black/70 border-4 {isKata ? (aoTotalPoints > akaTotalPoints ? 'border-blue-500 shadow-[0_0_40px_rgba(59,130,246,0.5)]' : 'border-[#2b2d31]') : (aoScore > akaScore ? 'border-blue-500 shadow-[0_0_40px_rgba(59,130,246,0.5)]' : 'border-[#2b2d31]')} rounded-3xl relative flex items-center justify-center shadow-inner">
            <span class="font-mono font-black text-8xl md:text-9xl 2xl:text-[12rem] text-white leading-none drop-shadow-[0_0_35px_rgba(59,130,246,0.85)]">
              {isKata ? aoTotalPoints : aoScore}
            </span>
          </div>
        </div>
      </div>

      <!-- Kumite Criteria Penalties -->
      {#if !isKata}
        <div class="w-full flex items-center justify-around gap-3 pt-3 mt-2 border-t-2 border-white/10 shrink-0">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            {@const active = aoPenalties[penalty as keyof typeof aoPenalties]}
            <div class="criteria-pill flex-1 py-3 md:py-4 2xl:py-5 rounded-2xl font-mono font-black text-2xl md:text-3xl 2xl:text-4xl text-center uppercase border-2 transition-all shadow-md {active ? 'bg-gradient-to-r from-blue-600 to-blue-700 text-white border-blue-300 shadow-[0_0_25px_rgba(59,130,246,0.95)] scale-[1.02]' : 'bg-black/50 text-white/30 border-white/10'}">
              {penalty.toUpperCase()}
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </main>

  <!-- ==================== KATA 5-JUDGE SCORE POPUP (NO BACKGROUND DIMMING, ONLY SCORE OUTPUTTED, POP ANIMATION STARTING FROM JURI 1, WHITE TEXT CONTRAST) ==================== -->
  {#if isKata && showKataScoreOverlay}
    <div
      class="fixed inset-0 z-50 pointer-events-none flex items-center justify-center p-6"
      role="region"
      aria-label="Skor 5 Juri Kata"
    >
      <!-- 5 Judge Columns floating directly in center with NO dimming backdrop -->
      <div class="pointer-events-auto flex items-center justify-center gap-4 md:gap-6 max-w-6xl w-full">
        {#each [0, 1, 2, 3, 4] as idx}
          {@const decision = judgeDecisions[idx]}
          {@const sAka = akaJudgeScores[idx] || "0.0"}
          {@const sAo = aoJudgeScores[idx] || "0.0"}
          <div
            class="judge-pop-pod flex-1 bg-[#090d16]/95 border-3 {decision === 'aka' ? 'border-red-500 shadow-[0_0_35px_rgba(239,68,68,0.7)]' : decision === 'ao' ? 'border-blue-500 shadow-[0_0_35px_rgba(59,130,246,0.7)]' : 'border-amber-400/60 shadow-[0_20px_50px_rgba(0,0,0,0.95)]'} rounded-3xl p-4 md:p-5 flex flex-col justify-between items-center gap-3 backdrop-blur-sm"
            style="animation-delay: {idx * 160}ms;"
          >
            <!-- AKA Judge Score (Top) - High contrast pure WHITE text -->
            <div class="w-full py-3 md:py-4 rounded-2xl font-mono font-black text-4xl md:text-5xl 2xl:text-6xl text-center text-white transition-all drop-shadow-[0_2px_8px_rgba(0,0,0,0.95)] {decision === 'aka' ? 'bg-red-600 border-2 border-red-300 shadow-[0_0_25px_rgba(239,68,68,0.9)] scale-[1.03]' : 'bg-[#181a20] border border-white/10 opacity-90'}">
              {sAka}
            </div>

            <!-- Judge Label (Middle) -->
            <div class="font-mono font-black text-lg md:text-xl 2xl:text-2xl text-amber-400 tracking-wider uppercase">
              JURI {idx + 1}
            </div>

            <!-- AO Judge Score (Bottom) - High contrast pure WHITE text -->
            <div class="w-full py-3 md:py-4 rounded-2xl font-mono font-black text-4xl md:text-5xl 2xl:text-6xl text-center text-white transition-all drop-shadow-[0_2px_8px_rgba(0,0,0,0.95)] {decision === 'ao' ? 'bg-blue-600 border-2 border-blue-300 shadow-[0_0_25px_rgba(59,130,246,0.9)] scale-[1.03]' : 'bg-[#181a20] border border-white/10 opacity-90'}">
              {sAo}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- ==================== BOTTOM MATCH TIMER & ARENA STATUS FOOTER BAR (BROADCAST GIANT SCALE) ==================== -->
  <footer class="bg-[#0b0e14] border-t-2 border-white/10 px-8 py-3 2xl:py-4 grid grid-cols-3 items-center shadow-2xl shrink-0">
    <!-- Left: Ambrilindo Sponsor Logo from Ambrilindo Local (BIGGER) -->
    <div class="flex items-center justify-start">
      <img
        src="/img/ambrilindo-logo-dark.png"
        alt="Ambrilindo Logo"
        class="h-20 md:h-24 2xl:h-32 max-w-[380px] object-contain drop-shadow-[0_0_24px_rgba(255,255,255,0.75)]"
      />
    </div>

    <!-- Center: Giant Digital Timer Display (BIGGER) -->
    <div class="flex items-center justify-center">
      <div class="bg-[#111216] border-4 border-amber-500/80 px-10 py-2.5 rounded-3xl flex items-center justify-center shadow-[inset_0_0_40px_rgba(0,0,0,0.9)]">
        <span class="font-mono font-black text-7xl md:text-8xl 2xl:text-[9.5rem] 3xl:text-[11.5rem] {isTimerRunning ? 'text-amber-300 drop-shadow-[0_0_35px_rgba(251,191,36,0.85)]' : 'text-white drop-shadow-md'} tracking-wider tabular-nums leading-none">
          {timerFormatted.full}
        </span>
      </div>
    </div>

    <!-- Right: Empty slot to keep center timer mathematically balanced -->
    <div class="flex items-center justify-end"></div>
  </footer>
</div>

<style>
  @keyframes judgePopAnim {
    0% {
      opacity: 0;
      transform: scale(0.35) translateY(40px);
    }
    65% {
      opacity: 1;
      transform: scale(1.08) translateY(-6px);
    }
    100% {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .judge-pop-pod {
    animation: judgePopAnim 0.45s cubic-bezier(0.175, 0.885, 0.32, 1.275) both;
  }
</style>
