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

  // Timer state
  let timerSeconds = $state(120);
  let isTimerRunning = $state(false);

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

  // Formatting minutes:seconds
  let formattedTime = $derived.by(() => {
    const safeSec = Math.max(0, timerSeconds);
    const mins = Math.floor(safeSec / 60);
    const secs = safeSec % 60;
    return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  });

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
    if (data.category) categoryName = data.category;
    if (data.round) matchRound = data.round;
    if (data.matchNumber !== undefined) matchNumber = String(data.matchNumber);
    if (data.timerSeconds !== undefined) timerSeconds = data.timerSeconds;
    if (data.isTimerRunning !== undefined) isTimerRunning = data.isTimerRunning;
    if (data.isKata !== undefined) isKata = data.isKata;
    if (data.winner !== undefined) matchWinner = data.winner;
    if (data.isFinished !== undefined) isMatchFinished = data.isFinished;

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

    // Listen to local realtime desktop IPC event
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

    // Request current state immediately from open control panel
    emit("request_tatami_state", { tatami: tatamiNumber }).catch(() => {});

    return () => {
      if (unlisten) unlisten();
    };
  });
</script>

<div class="h-screen w-screen bg-[#07090e] text-white flex flex-col justify-between overflow-hidden select-none font-sans">
  <!-- Top Match Banner Bar -->
  <header class="bg-[#0f141f] border-b-2 border-white/10 px-6 py-2.5 flex items-center justify-between shadow-2xl shrink-0">
    <div class="flex items-center space-x-4">
      <div class="bg-gradient-to-r from-amber-400 to-amber-600 text-slate-950 font-black px-4 py-1.5 rounded-xl text-2xl tracking-wider shadow-lg flex items-center gap-1.5">
        <span>TATAMI {tatamiNumber}</span>
      </div>
      <div>
        <h1 class="text-lg font-black tracking-wide text-zinc-100 uppercase">{eventName}</h1>
        <p class="text-xs font-bold text-amber-400 tracking-wider uppercase flex items-center gap-2">
          <span class="px-1.5 py-0.2 rounded font-mono font-black {isKata ? 'bg-amber-500/20 text-amber-300 border border-amber-500/30' : 'bg-blue-500/20 text-blue-300 border border-blue-500/30'}">
            {isKata ? "KATA" : "KUMITE"}
          </span>
          <span>{categoryName} • {matchRound}</span>
        </p>
      </div>
    </div>

    <div class="flex items-center space-x-5">
      <div class="text-right">
        <span class="text-[10px] uppercase tracking-widest text-zinc-400 block font-bold">PARTAI</span>
        <span class="text-2xl font-black text-amber-400 font-mono">#{matchNumber}</span>
      </div>

      <div class="flex items-center space-x-2 bg-[#181a20] px-3 py-1.5 rounded-full border border-white/10 shadow-inner">
        <div class="w-2.5 h-2.5 rounded-full {isLocalSyncConnected ? 'bg-emerald-500 animate-pulse' : 'bg-amber-500'}"></div>
        <span class="text-xs font-mono font-bold tracking-wider {isLocalSyncConnected ? 'text-emerald-400' : 'text-amber-400'}">
          {isLocalSyncConnected ? "LOCAL LIVE SYNC" : "MENUNGGU CP"}
        </span>
      </div>
    </div>
  </header>

  {#if isKata}
    <!-- ==================== KATA ARENA SCORING BOARD (AKA DI ATAS, AO DI BAWAH) ==================== -->
    <main class="flex-1 flex flex-col justify-between p-3 gap-2 relative min-h-0">
      <!-- TOP CARD: AKA (RED) -->
      <section class="flex-1 bg-gradient-to-r from-red-950/80 via-[#181116] to-[#0c0e14] border-2 border-red-500/40 rounded-2xl p-5 flex items-center justify-between shadow-2xl relative overflow-hidden">
        <!-- Red top accent line -->
        <div class="absolute top-0 left-0 right-0 h-1.5 bg-gradient-to-r from-red-500 via-red-600 to-transparent"></div>

        <div class="flex-1 min-w-0 pr-4">
          <div class="flex items-center gap-2 mb-1">
            <span class="bg-red-600 text-white text-[11px] font-black tracking-widest px-2.5 py-0.5 rounded font-mono uppercase shadow">
              AKA • MERAH
            </span>
            {#if matchWinner === "1"}
              <span class="bg-gradient-to-r from-amber-400 to-amber-500 text-black text-xs font-black px-3 py-0.5 rounded-md shadow flex items-center gap-1 animate-pulse">
                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20"><path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" /></svg>
                <span>PEMENANG</span>
              </span>
            {/if}
          </div>

          <!-- Athlete Name (Athletic broadcast scale) -->
          <h2 class="text-4xl 2xl:text-5xl font-black tracking-tight text-white uppercase leading-none drop-shadow-md">
            {akaName}
          </h2>

          <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
          <p class="text-base 2xl:text-lg font-bold text-red-200 mt-2 whitespace-normal break-words leading-snug">
            {akaClub}
          </p>

          <!-- Selected Kata Name Display -->
          {#if akaKataName}
            <div class="inline-flex items-center gap-1.5 mt-2.5 px-3 py-1 bg-black/50 border border-amber-500/40 rounded-lg text-amber-300 font-mono font-bold text-sm tracking-wide">
              <span class="text-zinc-400 text-xs font-normal">KATA:</span>
              <span>{akaKataName}</span>
            </div>
          {/if}
        </div>

        <!-- Total Points Box (AKA) -->
        <div class="shrink-0 flex flex-col items-center justify-center pl-4 border-l border-white/10">
          <span class="text-[10px] font-mono font-bold text-red-300 uppercase tracking-widest mb-1">TOTAL BENDERA</span>
          <div class="w-32 h-32 2xl:w-36 2xl:h-36 bg-[#111216] border-4 {akaTotalPoints > aoTotalPoints ? 'border-red-500 text-red-400 shadow-[0_0_35px_rgba(239,68,68,0.4)]' : 'border-[#2b2d31] text-zinc-300'} font-mono font-black text-8xl flex items-center justify-center rounded-2xl shadow-inner">
            {akaTotalPoints}
          </div>
        </div>
      </section>

      <!-- CENTER: 5-JUDGE COMPARATIVE SCORING STRIP -->
      <section class="bg-[#10141f] border-2 border-white/15 rounded-2xl py-2.5 px-6 shadow-2xl shrink-0">
        <div class="flex items-center justify-between mb-2">
          <span class="text-xs font-mono font-bold uppercase tracking-wider text-amber-400 flex items-center gap-1.5">
            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20"><path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" /></svg>
            PENILAIAN JURI KATA (5 JURI)
          </span>
          <span class="text-[11px] font-mono text-zinc-400">
            KOMPARASI SKOR JURI 1 S/D 5
          </span>
        </div>

        <div class="grid grid-cols-5 gap-3 text-center">
          {#each [0, 1, 2, 3, 4] as idx}
            {@const decision = judgeDecisions[idx]}
            {@const sAka = akaJudgeScores[idx] || "-"}
            {@const sAo = aoJudgeScores[idx] || "-"}
            <div class="bg-[#181a20] border {decision === 'aka' ? 'border-red-500/80 shadow-[0_0_15px_rgba(239,68,68,0.25)]' : decision === 'ao' ? 'border-blue-500/80 shadow-[0_0_15px_rgba(59,130,246,0.25)]' : 'border-[#2b2d31]'} rounded-xl p-2 flex flex-col justify-between transition-all">
              <!-- AKA Judge Score -->
              <div class="py-1.5 rounded-lg font-mono font-black text-2xl transition-all {decision === 'aka' ? 'bg-gradient-to-r from-red-600 to-red-700 text-white shadow-md border-2 border-red-300 scale-102' : 'bg-[#111216] text-red-300/60 border border-red-500/20'}">
                {sAka}
              </div>

              <!-- Judge Label Badge -->
              <div class="my-1.5 flex items-center justify-center gap-1 font-mono font-bold text-xs {decision === 'aka' ? 'text-red-400' : decision === 'ao' ? 'text-blue-400' : 'text-zinc-500'}">
                <span>JURI {idx + 1}</span>
                {#if decision === "aka"}
                  <span class="text-[9px] font-black">▲ AKA</span>
                {:else if decision === "ao"}
                  <span class="text-[9px] font-black">▼ AO</span>
                {/if}
              </div>

              <!-- AO Judge Score -->
              <div class="py-1.5 rounded-lg font-mono font-black text-2xl transition-all {decision === 'ao' ? 'bg-gradient-to-r from-blue-600 to-blue-700 text-white shadow-md border-2 border-blue-300 scale-102' : 'bg-[#111216] text-blue-300/60 border border-blue-500/20'}">
                {sAo}
              </div>
            </div>
          {/each}
        </div>
      </section>

      <!-- BOTTOM CARD: AO (BLUE) -->
      <section class="flex-1 bg-gradient-to-r from-blue-950/80 via-[#10141e] to-[#0c0e14] border-2 border-blue-500/40 rounded-2xl p-5 flex items-center justify-between shadow-2xl relative overflow-hidden">
        <!-- Blue bottom accent line -->
        <div class="absolute bottom-0 left-0 right-0 h-1.5 bg-gradient-to-r from-blue-500 via-blue-600 to-transparent"></div>

        <div class="flex-1 min-w-0 pr-4">
          <div class="flex items-center gap-2 mb-1">
            <span class="bg-blue-600 text-white text-[11px] font-black tracking-widest px-2.5 py-0.5 rounded font-mono uppercase shadow">
              AO • BIRU
            </span>
            {#if matchWinner === "2"}
              <span class="bg-gradient-to-r from-amber-400 to-amber-500 text-black text-xs font-black px-3 py-0.5 rounded-md shadow flex items-center gap-1 animate-pulse">
                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20"><path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" /></svg>
                <span>PEMENANG</span>
              </span>
            {/if}
          </div>

          <!-- Athlete Name (Athletic broadcast scale) -->
          <h2 class="text-4xl 2xl:text-5xl font-black tracking-tight text-white uppercase leading-none drop-shadow-md">
            {aoName}
          </h2>

          <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
          <p class="text-base 2xl:text-lg font-bold text-blue-200 mt-2 whitespace-normal break-words leading-snug">
            {aoClub}
          </p>

          <!-- Selected Kata Name Display -->
          {#if aoKataName}
            <div class="inline-flex items-center gap-1.5 mt-2.5 px-3 py-1 bg-black/50 border border-amber-500/40 rounded-lg text-amber-300 font-mono font-bold text-sm tracking-wide">
              <span class="text-zinc-400 text-xs font-normal">KATA:</span>
              <span>{aoKataName}</span>
            </div>
          {/if}
        </div>

        <!-- Total Points Box (AO) -->
        <div class="shrink-0 flex flex-col items-center justify-center pl-4 border-l border-white/10">
          <span class="text-[10px] font-mono font-bold text-blue-300 uppercase tracking-widest mb-1">TOTAL BENDERA</span>
          <div class="w-32 h-32 2xl:w-36 2xl:h-36 bg-[#111216] border-4 {aoTotalPoints > akaTotalPoints ? 'border-blue-500 text-blue-400 shadow-[0_0_35px_rgba(59,130,246,0.4)]' : 'border-[#2b2d31] text-zinc-300'} font-mono font-black text-8xl flex items-center justify-center rounded-2xl shadow-inner">
            {aoTotalPoints}
          </div>
        </div>
      </section>
    </main>

  {:else}
    <!-- ==================== KUMITE SCORING BOARD ==================== -->
    <main class="flex-1 grid grid-cols-12 gap-0 relative min-h-0">
      <!-- AKA Side -->
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

        <div class="my-auto flex justify-center items-center py-4">
          <span class="text-[14rem] 2xl:text-[18rem] leading-none font-black text-white tabular-nums drop-shadow-[0_10px_20px_rgba(0,0,0,0.8)]">
            {akaScore}
          </span>
        </div>

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

      <!-- Center Timer -->
      <section class="col-span-2 bg-slate-950 flex flex-col justify-between items-center py-6 px-3 border-x-4 border-slate-800 z-10 shadow-2xl">
        <div class="w-full text-center">
          <span class="text-xs font-black uppercase tracking-widest text-slate-400 block mb-1">STATUS PERTANDINGAN</span>
          <div class="py-1 px-3 rounded-lg text-xs font-black tracking-wider uppercase border {isTimerRunning ? 'bg-emerald-950 text-emerald-400 border-emerald-600' : 'bg-slate-900 text-slate-400 border-slate-700'}">
            {isTimerRunning ? 'BERLANGSUNG' : 'BERHENTI (YAME)'}
          </div>
        </div>

        <div class="my-auto w-full text-center">
          <div class="bg-black/90 border-4 {timerSeconds <= 15 && timerSeconds > 0 ? 'border-amber-400 shadow-[0_0_40px_rgba(251,191,36,0.6)] animate-pulse' : 'border-slate-700'} rounded-2xl py-6 px-2">
            <span class="text-xs uppercase font-black tracking-widest text-slate-400 block mb-2">
              WAKTU PERTANDINGAN
            </span>
            <div class="text-6xl 2xl:text-7xl font-mono font-black tabular-nums tracking-tighter text-amber-300">
              {formattedTime}
            </div>
          </div>
        </div>

        <div class="text-center">
          <span class="text-[10px] uppercase font-bold text-slate-500 tracking-wider">AMBRILINDO LOCAL v3.0</span>
        </div>
      </section>

      <!-- AO Side -->
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

        <div class="my-auto flex justify-center items-center py-4">
          <span class="text-[14rem] 2xl:text-[18rem] leading-none font-black text-white tabular-nums drop-shadow-[0_10px_20px_rgba(0,0,0,0.8)]">
            {aoScore}
          </span>
        </div>

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
  {/if}

  <!-- Bottom Match Timer & Arena Status Footer Bar (Broadcast scale) -->
  <footer class="bg-[#0b0e14] border-t-2 border-white/10 px-8 py-2.5 flex items-center justify-between shadow-2xl shrink-0">
    <div class="flex items-center space-x-3">
      <span class="text-xs uppercase font-mono font-bold text-zinc-500 tracking-wider">AMBRILINDO ARENA BROADCAST</span>
    </div>

    <!-- Center Giant Digital Timer Display -->
    <div class="flex items-center space-x-4">
      <div class="bg-[#111216] border-2 border-amber-500/60 px-6 py-1.5 rounded-xl flex items-baseline space-x-1.5 shadow-inner">
        <span class="font-mono font-black text-4xl 2xl:text-5xl text-amber-300 tracking-wider">
          {formattedTime}
        </span>
        <span class="font-mono text-xs text-amber-500/70">.00</span>
      </div>

      <div class="py-1.5 px-3 rounded-lg text-xs font-mono font-black tracking-wider uppercase border {isTimerRunning ? 'bg-emerald-950 text-emerald-400 border-emerald-500/50' : 'bg-zinc-800 text-zinc-400 border-zinc-700'}">
        {isTimerRunning ? "BERLANGSUNG" : "BERHENTI (YAME)"}
      </div>
    </div>

    <div class="flex items-center space-x-2 text-xs font-mono text-zinc-400">
      <span>STANDAR WKF 5 JUDGES</span>
    </div>
  </footer>
</div>
