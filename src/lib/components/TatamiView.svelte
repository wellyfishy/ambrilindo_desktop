<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface Props {
    tatamiNumber: number;
    hostAddress: string;
    onOpenMonitorPicker: () => void;
    onTriggerBuzzer: (sound: "warning" | "finish") => void;
  }

  let { tatamiNumber, hostAddress, onOpenMonitorPicker, onTriggerBuzzer }: Props = $props();

  // Match state
  let matchNumber = $state(1);
  let categoryName = $state("KUMITE PERORANGAN PUTRA");
  let roundName = $state("BABAK PENYISIHAN");

  // AKA (Red)
  let akaName = $state("ATLET SUDUT MERAH");
  let akaClub = $state("KONTINGEN A");
  let akaScore = $state(0);
  let akaSenshu = $state(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // AO (Blue)
  let aoName = $state("ATLET SUDUT BIRU");
  let aoClub = $state("KONTINGEN B");
  let aoScore = $state(0);
  let aoSenshu = $state(false);
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Timer
  let timerSeconds = $state(120);
  let isTimerRunning = $state(false);
  let timerInterval: any = null;

  let formattedTime = $derived.by(() => {
    const mins = Math.floor(timerSeconds / 60);
    const secs = timerSeconds % 60;
    return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  });

  let isWarningTime = $derived(timerSeconds <= 15 && timerSeconds > 0);

  function toggleTimer() {
    if (isTimerRunning) {
      clearInterval(timerInterval);
      timerInterval = null;
      isTimerRunning = false;
    } else {
      isTimerRunning = true;
      timerInterval = setInterval(() => {
        if (timerSeconds > 0) {
          timerSeconds -= 1;
          if (timerSeconds === 15) {
            onTriggerBuzzer("warning");
          }
        } else {
          clearInterval(timerInterval);
          timerInterval = null;
          isTimerRunning = false;
          onTriggerBuzzer("finish");
        }
      }, 1000);
    }
  }

  function resetTimer(seconds: number = 120) {
    if (timerInterval) clearInterval(timerInterval);
    timerInterval = null;
    isTimerRunning = false;
    timerSeconds = seconds;
  }

  function addScore(side: 'aka' | 'ao', points: number) {
    if (side === 'aka') {
      akaScore = Math.max(0, akaScore + points);
    } else {
      aoScore = Math.max(0, aoScore + points);
    }
  }

  function toggleSenshu(side: 'aka' | 'ao') {
    if (side === 'aka') {
      akaSenshu = !akaSenshu;
      if (akaSenshu) aoSenshu = false;
    } else {
      aoSenshu = !aoSenshu;
      if (aoSenshu) akaSenshu = false;
    }
  }

  function togglePenalty(side: 'aka' | 'ao', penalty: 'c1' | 'c2' | 'c3' | 'hc' | 'h') {
    if (side === 'aka') {
      akaPenalties[penalty] = !akaPenalties[penalty];
    } else {
      aoPenalties[penalty] = !aoPenalties[penalty];
    }
  }

  function resetMatch() {
    akaScore = 0;
    akaSenshu = false;
    akaPenalties = { c1: false, c2: false, c3: false, hc: false, h: false };

    aoScore = 0;
    aoSenshu = false;
    aoPenalties = { c1: false, c2: false, c3: false, hc: false, h: false };

    resetTimer(120);
    matchNumber += 1;
  }

  onDestroy(() => {
    if (timerInterval) clearInterval(timerInterval);
  });
</script>

<div class="h-full flex flex-col p-6 space-y-6 overflow-y-auto select-none bg-[#313338] text-[#dbdee1]">
  <!-- Top Match Info & Projection Banner -->
  <div class="bg-[#2b2d31] border border-[#3f4147] rounded-2xl p-4 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 shadow-xl">
    <div class="flex items-center space-x-4">
      <div class="w-12 h-12 rounded-xl bg-amber-500 text-slate-950 font-black flex items-center justify-center text-xl shadow">
        T{tatamiNumber}
      </div>
      <div>
        <div class="flex items-center space-x-2">
          <span class="text-base font-black text-white">TATAMI {tatamiNumber}</span>
          <span class="bg-[#35373c] text-amber-300 text-xs font-mono font-bold px-2 py-0.5 rounded">
            PARTAI #{matchNumber}
          </span>
        </div>
        <p class="text-xs text-[#949ba4] font-medium mt-0.5">
          {categoryName} • {roundName}
        </p>
      </div>
    </div>

    <!-- Quick Action / Projection Button -->
    <div class="flex items-center space-x-2 w-full sm:w-auto">
      <button
        onclick={onOpenMonitorPicker}
        class="flex-1 sm:flex-initial px-4 py-2 bg-gradient-to-r from-amber-500 to-amber-400 hover:from-amber-400 hover:to-amber-300 text-slate-950 font-black text-xs rounded-xl shadow transition active:scale-95 flex items-center justify-center space-x-1.5"
      >
        <span>🖥️ Proyeksikan Layar Ini</span>
      </button>

      <button
        onclick={resetMatch}
        class="px-3 py-2 bg-[#35373c] hover:bg-[#404249] text-[#dbdee1] text-xs font-bold rounded-xl transition"
        title="Reset skor dan lanjut ke partai berikutnya"
      >
        Partai Baru
      </button>
    </div>
  </div>

  <!-- Arena Combatant Cards Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-6 flex-1 min-h-[460px]">
    <!-- AKA (Red) Panel - 5 Cols -->
    <div class="lg:col-span-5 bg-gradient-to-br from-red-900/40 via-[#2b2d31] to-[#2b2d31] border-2 border-red-700/60 rounded-3xl p-6 flex flex-col justify-between shadow-2xl">
      <div>
        <div class="flex items-center justify-between mb-3">
          <span class="bg-red-600 text-white font-black text-xs px-3 py-1 rounded-lg uppercase tracking-wider shadow">
            AKA (MERAH)
          </span>

          <button
            onclick={() => toggleSenshu('aka')}
            class="px-2.5 py-1 rounded-lg text-xs font-bold uppercase transition {akaSenshu ? 'bg-amber-400 text-slate-950 font-black ring-2 ring-amber-300 shadow' : 'bg-[#1e1f22] text-[#949ba4] hover:text-white'}"
          >
            {akaSenshu ? 'SENSHU ★' : 'Senshu'}
          </button>
        </div>

        <input
          type="text"
          bind:value={akaName}
          class="w-full bg-transparent text-2xl font-black text-white uppercase focus:bg-[#1e1f22]/50 px-2 py-1 rounded-lg outline-none border-b border-transparent focus:border-red-500"
        />
        <input
          type="text"
          bind:value={akaClub}
          class="w-full bg-transparent text-sm font-bold text-red-300 uppercase focus:bg-[#1e1f22]/50 px-2 py-0.5 rounded outline-none"
        />
      </div>

      <!-- Huge Score Counter -->
      <div class="my-auto py-4 text-center">
        <span class="text-8xl 2xl:text-9xl font-black text-white tabular-nums drop-shadow-lg">
          {akaScore}
        </span>

        <!-- Point Buttons (+1, +2, +3, -1) -->
        <div class="flex items-center justify-center space-x-2 mt-4">
          <button
            onclick={() => addScore('aka', 1)}
            class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white font-black text-xs rounded-xl shadow transition active:scale-95"
          >
            +1 YUKO
          </button>
          <button
            onclick={() => addScore('aka', 2)}
            class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white font-black text-xs rounded-xl shadow transition active:scale-95"
          >
            +2 WAZA-ARI
          </button>
          <button
            onclick={() => addScore('aka', 3)}
            class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white font-black text-xs rounded-xl shadow transition active:scale-95"
          >
            +3 IPPON
          </button>
          <button
            onclick={() => addScore('aka', -1)}
            class="px-3 py-2 bg-[#1e1f22] hover:bg-[#35373c] text-red-300 font-bold text-xs rounded-xl transition"
          >
            -1
          </button>
        </div>
      </div>

      <!-- Penalties (C1, C2, C3, HC, H) -->
      <div class="bg-[#1e1f22]/80 rounded-2xl p-3 border border-[#3f4147]">
        <span class="text-[10px] font-extrabold uppercase text-[#949ba4] block mb-2">PELANGGARAN (PENALTIES)</span>
        <div class="grid grid-cols-5 gap-2">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            {@const pKey = penalty as keyof typeof akaPenalties}
            <button
              onclick={() => togglePenalty('aka', pKey)}
              class="py-2 text-center rounded-lg font-black text-xs uppercase border transition {akaPenalties[pKey] ? 'bg-amber-400 text-slate-950 border-amber-300 shadow' : 'bg-[#2b2d31] text-[#949ba4] border-[#3f4147] hover:text-white'}"
            >
              {penalty.toUpperCase()}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <!-- Center Timer & Arena Actions - 2 Cols -->
    <div class="lg:col-span-2 bg-[#2b2d31] border border-[#3f4147] rounded-3xl p-4 flex flex-col justify-between items-center shadow-xl">
      <div class="text-center w-full">
        <span class="text-[10px] font-black uppercase text-[#949ba4] block mb-1">STATUS PERTANDINGAN</span>
        <div class="py-1 px-2 rounded-lg text-xs font-black uppercase border {isTimerRunning ? 'bg-emerald-500/20 text-emerald-400 border-emerald-500/40' : 'bg-[#1e1f22] text-[#949ba4] border-[#3f4147]'}">
          {isTimerRunning ? 'BERLANGSUNG' : 'BERHENTI (YAME)'}
        </div>
      </div>

      <!-- Large Digital Timer -->
      <div class="my-auto text-center w-full">
        <div class="bg-[#1e1f22] border-2 {isWarningTime ? 'border-amber-400 animate-pulse' : 'border-[#3f4147]'} rounded-2xl py-6 px-2">
          <span class="text-[10px] uppercase font-bold text-[#949ba4] block mb-1">
            {isWarningTime ? 'ATOSHI BARAKU (15s)' : 'WAKTU'}
          </span>
          <div class="text-4xl 2xl:text-5xl font-mono font-black tabular-nums tracking-tighter {isWarningTime ? 'text-amber-400' : 'text-white'}">
            {formattedTime}
          </div>
        </div>

        <!-- Timer Controls -->
        <div class="grid grid-cols-2 gap-2 mt-3 w-full">
          <button
            onclick={toggleTimer}
            class="py-2.5 rounded-xl font-black text-xs transition shadow {isTimerRunning ? 'bg-amber-600 hover:bg-amber-500 text-white' : 'bg-emerald-600 hover:bg-emerald-500 text-white'}"
          >
            {isTimerRunning ? 'JEDA (YAME)' : 'MULAI (HAJIME)'}
          </button>
          <button
            onclick={() => resetTimer(120)}
            class="py-2.5 bg-[#35373c] hover:bg-[#404249] text-white font-bold text-xs rounded-xl transition"
          >
            Reset (2:00)
          </button>
        </div>
      </div>

      <div class="text-center text-[10px] text-[#949ba4]">
        <span>Tatami #{tatamiNumber} Active</span>
      </div>
    </div>

    <!-- AO (Blue) Panel - 5 Cols -->
    <div class="lg:col-span-5 bg-gradient-to-bl from-blue-900/40 via-[#2b2d31] to-[#2b2d31] border-2 border-blue-700/60 rounded-3xl p-6 flex flex-col justify-between shadow-2xl">
      <div>
        <div class="flex items-center justify-between mb-3">
          <span class="bg-blue-600 text-white font-black text-xs px-3 py-1 rounded-lg uppercase tracking-wider shadow">
            AO (BIRU)
          </span>

          <button
            onclick={() => toggleSenshu('ao')}
            class="px-2.5 py-1 rounded-lg text-xs font-bold uppercase transition {aoSenshu ? 'bg-amber-400 text-slate-950 font-black ring-2 ring-amber-300 shadow' : 'bg-[#1e1f22] text-[#949ba4] hover:text-white'}"
          >
            {aoSenshu ? 'SENSHU ★' : 'Senshu'}
          </button>
        </div>

        <input
          type="text"
          bind:value={aoName}
          class="w-full bg-transparent text-2xl font-black text-white uppercase focus:bg-[#1e1f22]/50 px-2 py-1 rounded-lg outline-none border-b border-transparent focus:border-blue-500"
        />
        <input
          type="text"
          bind:value={aoClub}
          class="w-full bg-transparent text-sm font-bold text-blue-300 uppercase focus:bg-[#1e1f22]/50 px-2 py-0.5 rounded outline-none"
        />
      </div>

      <!-- Huge Score Counter -->
      <div class="my-auto py-4 text-center">
        <span class="text-8xl 2xl:text-9xl font-black text-white tabular-nums drop-shadow-lg">
          {aoScore}
        </span>

        <!-- Point Buttons (+1, +2, +3, -1) -->
        <div class="flex items-center justify-center space-x-2 mt-4">
          <button
            onclick={() => addScore('ao', 1)}
            class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-black text-xs rounded-xl shadow transition active:scale-95"
          >
            +1 YUKO
          </button>
          <button
            onclick={() => addScore('ao', 2)}
            class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-black text-xs rounded-xl shadow transition active:scale-95"
          >
            +2 WAZA-ARI
          </button>
          <button
            onclick={() => addScore('ao', 3)}
            class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-black text-xs rounded-xl shadow transition active:scale-95"
          >
            +3 IPPON
          </button>
          <button
            onclick={() => addScore('ao', -1)}
            class="px-3 py-2 bg-[#1e1f22] hover:bg-[#35373c] text-blue-300 font-bold text-xs rounded-xl transition"
          >
            -1
          </button>
        </div>
      </div>

      <!-- Penalties (C1, C2, C3, HC, H) -->
      <div class="bg-[#1e1f22]/80 rounded-2xl p-3 border border-[#3f4147]">
        <span class="text-[10px] font-extrabold uppercase text-[#949ba4] block mb-2">PELANGGARAN (PENALTIES)</span>
        <div class="grid grid-cols-5 gap-2">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            {@const pKey = penalty as keyof typeof aoPenalties}
            <button
              onclick={() => togglePenalty('ao', pKey)}
              class="py-2 text-center rounded-lg font-black text-xs uppercase border transition {aoPenalties[pKey] ? 'bg-amber-400 text-slate-950 border-amber-300 shadow' : 'bg-[#2b2d31] text-[#949ba4] border-[#3f4147] hover:text-white'}"
            >
              {penalty.toUpperCase()}
            </button>
          {/each}
        </div>
      </div>
    </div>
  </div>
</div>
