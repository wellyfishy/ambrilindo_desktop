<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { DetailBaganItem, BaganItem, SaveMatchInput } from "$lib/types";
  import { KATA_ENTRIES, type KataItem } from "$lib/kata_data";

  interface Props {
    match: DetailBaganItem;
    baganInfo?: BaganItem | null;
    baganName?: string;
    eventName?: string;
    tatamiNumber?: number;
    hostUrl?: string;
    logoMap?: Record<string, string>;
    onBack: () => void;
    onSave: (input: SaveMatchInput) => Promise<void> | void;
  }

  let {
    match,
    baganInfo = null,
    baganName = "",
    eventName = "AMBRILINDO KARATE CHAMPIONSHIP",
    tatamiNumber = 1,
    hostUrl = "",
    logoMap = {},
    onBack,
    onSave,
  }: Props = $props();

  // Mode detection
  let detectedIsKata = $derived(
    (baganInfo?.tipe_tanding?.toLowerCase() || "").includes("kata") ||
    baganName.toLowerCase().includes("kata")
  );
  let activeTab = $state<"kumite" | "kata">("kumite");

  // Common Match State
  let matchWinner = $state<string>("");
  let isMatchFinished = $state<boolean>(false);
  let isSaving = $state<boolean>(false);
  let hasVr1 = $state<boolean>(false);
  let hasVr2 = $state<boolean>(false);
  let isHantei = $state<boolean>(false);

  // Kumite Score State
  let akaScore = $state<number>(0);
  let aoScore = $state<number>(0);
  let akaSenshu = $state<boolean>(false);
  let aoSenshu = $state<boolean>(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Kata 5-Judges Comparative Scoring State
  let kataAkaName = $state<string>("");
  let kataAoName = $state<string>("");
  let akaJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let aoJudgeScores = $state<string[]>(["", "", "", "", ""]);

  // Kata Discord-style Selection Modal State
  let isKataModalOpen = $state<boolean>(false);
  let kataModalTarget = $state<"aka" | "ao">("aka");
  let kataSearchQuery = $state<string>("");
  let kataModalSelectedIndex = $state<number>(0);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  let filteredKataList = $derived.by(() => {
    const q = kataSearchQuery.trim().toLowerCase();
    if (!q) return KATA_ENTRIES;
    return KATA_ENTRIES.filter(
      (k) =>
        k.code.includes(q) ||
        k.name.toLowerCase().includes(q) ||
        k.label.toLowerCase().includes(q)
    );
  });

  function openKataModal(target: "aka" | "ao") {
    kataModalTarget = target;
    kataSearchQuery = "";
    kataModalSelectedIndex = 0;
    isKataModalOpen = true;
    setTimeout(() => {
      searchInputEl?.focus();
    }, 50);
  }

  function selectKataItem(item: KataItem) {
    if (kataModalTarget === "aka") {
      kataAkaName = item.label;
    } else {
      kataAoName = item.label;
    }
    isKataModalOpen = false;
  }

  function handleKataModalKeydown(e: KeyboardEvent) {
    if (!isKataModalOpen) return;
    if (e.key === "Escape") {
      isKataModalOpen = false;
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (filteredKataList.length > 0) {
        kataModalSelectedIndex = (kataModalSelectedIndex + 1) % filteredKataList.length;
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (filteredKataList.length > 0) {
        kataModalSelectedIndex =
          (kataModalSelectedIndex - 1 + filteredKataList.length) % filteredKataList.length;
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filteredKataList[kataModalSelectedIndex]) {
        selectKataItem(filteredKataList[kataModalSelectedIndex]);
      }
    }
  }

  // ==================== MATCH TIMER STATE & PRESETS ====================
  // Presets requested: 30, 60, 90, 120, 150, 180, 300
  const TIMER_PRESETS = [30, 60, 90, 120, 150, 180, 300];
  let selectedPresetSeconds = $state<number>(120);
  let timerSeconds = $state<number>(120);
  let isTimerRunning = $state<boolean>(false);
  let timerInterval: any = null;

  function formatTimeMMSS(sec: number): string {
    const safeSec = Math.max(0, sec);
    const m = Math.floor(safeSec / 60);
    const s = safeSec % 60;
    const mStr = m < 10 ? `0${m}` : `${m}`;
    const sStr = s < 10 ? `0${s}` : `${s}`;
    return `${mStr}:${sStr}`;
  }

  function formatPresetLabel(preset: number): string {
    if (preset < 60) return `${preset}s`;
    const m = Math.floor(preset / 60);
    const s = preset % 60;
    if (s === 0) return `${m}m`;
    return `${m}:${s < 10 ? "0" + s : s}`;
  }

  function selectTimerPreset(seconds: number) {
    selectedPresetSeconds = seconds;
    timerSeconds = seconds;
    if (isTimerRunning) {
      clearInterval(timerInterval);
      isTimerRunning = false;
    }
  }

  function toggleTimer() {
    if (isTimerRunning) {
      clearInterval(timerInterval);
      isTimerRunning = false;
    } else {
      if (timerSeconds <= 0) {
        timerSeconds = selectedPresetSeconds;
      }
      isTimerRunning = true;
      timerInterval = setInterval(() => {
        if (timerSeconds > 0) {
          timerSeconds -= 1;
        } else {
          clearInterval(timerInterval);
          isTimerRunning = false;
        }
      }, 1000);
    }
  }

  function resetTimer() {
    if (isTimerRunning) {
      clearInterval(timerInterval);
      isTimerRunning = false;
    }
    timerSeconds = selectedPresetSeconds;
  }

  function adjustTimer(delta: number) {
    timerSeconds = Math.max(0, timerSeconds + delta);
  }

  // ==================== 5-JUDGE COMPARATIVE SCORING LOGIC ====================
  // For each of 5 judges:
  // If AKA > AO: J1 highlights red, AKA gets +1 flag point
  // If AO > AKA: J1 highlights blue, AO gets +1 flag point
  // If equal / unfilled: neutral
  let judgeComparisons = $derived.by(() => {
    let akaPoints = 0;
    let aoPoints = 0;
    let decisions: Array<{
      winner: "aka" | "ao" | "seri" | "empty";
      scoreAka: number | null;
      scoreAo: number | null;
    }> = [];

    for (let i = 0; i < 5; i++) {
      const rawAka = akaJudgeScores[i]?.trim();
      const rawAo = aoJudgeScores[i]?.trim();
      const vAka = rawAka !== "" ? parseFloat(rawAka) : NaN;
      const vAo = rawAo !== "" ? parseFloat(rawAo) : NaN;

      const hasAka = !isNaN(vAka) && ((vAka >= 5.0 && vAka <= 10.0) || vAka === 0);
      const hasAo = !isNaN(vAo) && ((vAo >= 5.0 && vAo <= 10.0) || vAo === 0);

      if (hasAka && hasAo) {
        if (vAka === 0 && vAo > 0) {
          aoPoints += 1;
          decisions.push({ winner: "ao", scoreAka: vAka, scoreAo: vAo });
        } else if (vAo === 0 && vAka > 0) {
          akaPoints += 1;
          decisions.push({ winner: "aka", scoreAka: vAka, scoreAo: vAo });
        } else if (vAka > vAo) {
          akaPoints += 1;
          decisions.push({ winner: "aka", scoreAka: vAka, scoreAo: vAo });
        } else if (vAo > vAka) {
          aoPoints += 1;
          decisions.push({ winner: "ao", scoreAka: vAka, scoreAo: vAo });
        } else {
          decisions.push({ winner: "seri", scoreAka: vAka, scoreAo: vAo });
        }
      } else if (hasAka && !hasAo) {
        decisions.push({ winner: "empty", scoreAka: vAka, scoreAo: null });
      } else if (!hasAka && hasAo) {
        decisions.push({ winner: "empty", scoreAka: null, scoreAo: vAo });
      } else {
        decisions.push({ winner: "empty", scoreAka: null, scoreAo: null });
      }
    }

    return { akaPoints, aoPoints, decisions };
  });

  // Reactive auto winner for Kata
  $effect(() => {
    if (activeTab === "kata") {
      const { akaPoints, aoPoints } = judgeComparisons;
      if (akaPoints > aoPoints) {
        matchWinner = "1";
      } else if (aoPoints > akaPoints) {
        matchWinner = "2";
      }
    }
  });

  function updateJudgeScore(side: "aka" | "ao", judgeIdx: number, val: string) {
    if (side === "aka") {
      akaJudgeScores[judgeIdx] = val;
    } else {
      aoJudgeScores[judgeIdx] = val;
    }
  }

  // Kiken Handlers (Replaces DQ, no "(Aka Menang)" / "(Ao Menang)" text)
  function applyKiken(side: "aka" | "ao") {
    if (side === "aka") {
      akaJudgeScores = ["0.0", "0.0", "0.0", "0.0", "0.0"];
      aoJudgeScores = aoJudgeScores.map((s) => (s && s !== "0.0" ? s : "8.0"));
      matchWinner = "2";
    } else {
      aoJudgeScores = ["0.0", "0.0", "0.0", "0.0", "0.0"];
      akaJudgeScores = akaJudgeScores.map((s) => (s && s !== "0.0" ? s : "8.0"));
      matchWinner = "1";
    }
  }

  // ==================== KUMITE SCORING ====================
  function addKumiteScore(side: "aka" | "ao", pts: number) {
    if (side === "aka") {
      akaScore = Math.max(0, akaScore + pts);
    } else {
      aoScore = Math.max(0, aoScore + pts);
    }
    autoSuggestKumiteWinner();
  }

  function toggleSenshu(side: "aka" | "ao") {
    if (side === "aka") {
      akaSenshu = !akaSenshu;
      if (akaSenshu) aoSenshu = false;
    } else {
      aoSenshu = !aoSenshu;
      if (aoSenshu) akaSenshu = false;
    }
    autoSuggestKumiteWinner();
  }

  function togglePenalty(side: "aka" | "ao", code: "c1" | "c2" | "c3" | "hc" | "h") {
    if (side === "aka") {
      akaPenalties[code] = !akaPenalties[code];
      if (code === "h" && akaPenalties[code]) {
        matchWinner = "2";
      }
    } else {
      aoPenalties[code] = !aoPenalties[code];
      if (code === "h" && aoPenalties[code]) {
        matchWinner = "1";
      }
    }
  }

  function autoSuggestKumiteWinner() {
    if (akaScore > aoScore) {
      matchWinner = "1";
    } else if (aoScore > akaScore) {
      matchWinner = "2";
    } else if (akaSenshu) {
      matchWinner = "1";
    } else if (aoSenshu) {
      matchWinner = "2";
    }
  }

  function selectWinner(winner: "1" | "2") {
    if (matchWinner === winner) {
      matchWinner = "";
    } else {
      matchWinner = winner;
    }
  }

  // ==================== SAVING & EXIT ====================
  async function handleSaveAndExit() {
    if (isSaving) return;
    isSaving = true;

    try {
      const finalScore1 = activeTab === "kata" ? String(judgeComparisons.akaPoints) : String(akaScore);
      const finalScore2 = activeTab === "kata" ? String(judgeComparisons.aoPoints) : String(aoScore);

      const input: SaveMatchInput = {
        match_id: match.id,
        bagan_id: match.bagan_id || 0,
        score1: finalScore1,
        score2: finalScore2,
        scorekecil1: null,
        scorekecil2: null,
        pemenang: matchWinner || null,
        kata1: kataAkaName || null,
        kata2: kataAoName || null,
        vr1: hasVr1,
        vr2: hasVr2,
        hantei: isHantei,
        selesai: isMatchFinished || !!matchWinner,
      };

      await onSave(input);
    } catch (err: any) {
      console.error("Gagal menyimpan hasil partai:", err);
    } finally {
      isSaving = false;
    }
  }

  // Initialize values from match prop
  onMount(() => {
    activeTab = detectedIsKata ? "kata" : "kumite";
    matchWinner = match.pemenang || "";
    isMatchFinished = match.selesai || false;
    hasVr1 = match.vr1 || false;
    hasVr2 = match.vr2 || false;
    isHantei = match.hantei || false;

    // Kumite initial scores
    akaScore = parseInt(match.score1 || "0", 10) || 0;
    aoScore = parseInt(match.score2 || "0", 10) || 0;

    // Kata initial scores
    kataAkaName = match.kata1 || "";
    kataAoName = match.kata2 || "";

    const handleKeyDown = (e: KeyboardEvent) => {
      // Spacebar for Timer Start/Stop
      if (e.code === "Space" && !(e.target instanceof HTMLInputElement)) {
        e.preventDefault();
        toggleTimer();
      }
      // Kumite Hotkeys (1/2/3 for AO, 7/8/9 for AKA)
      if (activeTab === "kumite" && !(e.target instanceof HTMLInputElement)) {
        if (e.key === "1") addKumiteScore("ao", 1);
        if (e.key === "2") addKumiteScore("ao", 2);
        if (e.key === "3") addKumiteScore("ao", 3);
        if (e.key === "7") addKumiteScore("aka", 1);
        if (e.key === "8") addKumiteScore("aka", 2);
        if (e.key === "9") addKumiteScore("aka", 3);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      if (timerInterval) clearInterval(timerInterval);
    };
  });
</script>

<svelte:window onkeydown={handleKataModalKeydown} />

<!-- Main Cockpit Container (Discord Palette: #0d0f12 deep background) -->
<div class="h-full flex flex-col bg-[#0d0f12] text-zinc-100 overflow-hidden font-sans select-none p-3">
  <!-- Top Cockpit Header Bar (Discord Card #181a20, Border #2b2d31) -->
  <div class="cockpit-page-header flex items-center justify-between flex-wrap gap-3 p-3 bg-[#181a20] border border-[#2b2d31] rounded-xl shadow-lg mb-3 shrink-0">
    <div class="flex items-center space-x-3">
      <!-- Back Button -->
      <button
        type="button"
        onclick={onBack}
        class="px-3.5 py-1.5 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-200 text-xs font-semibold rounded-lg transition flex items-center space-x-1.5 shadow-sm active:scale-95"
      >
        <svg class="w-4 h-4 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
        </svg>
        <span>Kembali ke Bagan</span>
      </button>

      <!-- Save & Exit Button -->
      <button
        type="button"
        onclick={handleSaveAndExit}
        disabled={isSaving}
        class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-bold rounded-lg transition flex items-center space-x-1.5 shadow-sm active:scale-95 disabled:opacity-50"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
        </svg>
        <span>{isSaving ? "Menyimpan..." : "Simpan & Keluar"}</span>
      </button>

      <!-- Mode Switcher Tabs -->
      <div class="flex items-center bg-[#111216] border border-[#2b2d31] rounded-lg p-0.5 text-xs font-medium ml-2">
        <button
          type="button"
          onclick={() => (activeTab = "kata")}
          class="px-3 py-1 rounded-md transition {activeTab === 'kata' ? 'bg-[#5865f2] text-white font-bold shadow' : 'text-zinc-400 hover:text-white'}"
        >
          Mode Kata
        </button>
        <button
          type="button"
          onclick={() => (activeTab = "kumite")}
          class="px-3 py-1 rounded-md transition {activeTab === 'kumite' ? 'bg-[#5865f2] text-white font-bold shadow' : 'text-zinc-400 hover:text-white'}"
        >
          Mode Kumite
        </button>
      </div>
    </div>

    <!-- Match Identity Badge -->
    <div class="flex items-center space-x-2 text-xs font-mono">
      <span class="px-2 py-0.5 rounded font-bold uppercase {activeTab === 'kata' ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30' : 'bg-blue-500/20 text-blue-400 border border-blue-500/30'}">
        {activeTab === "kata" ? "KATA" : "KUMITE"}
      </span>
      <span class="text-zinc-200 font-bold uppercase truncate max-w-[280px]">
        {baganName || "Kategori Turnamen"}
      </span>
      <span class="text-zinc-500">•</span>
      <span class="text-zinc-300 font-semibold">MATCH {match.urutan || 1}</span>
      <span class="text-zinc-500">•</span>
      <span class="text-zinc-400">
        {match.round === 4 ? "PARTAI FINAL" : match.round === 3 ? "SEMIFINAL" : `BABAK ${match.round || 1}`}
      </span>
      <span class="text-zinc-500">•</span>
      <span class="text-amber-400 font-bold">Tatami {tatamiNumber}</span>
    </div>

    <!-- Active Indicator -->
    <div class="flex items-center space-x-2">
      <div class="flex items-center space-x-1.5 text-xs font-medium text-emerald-400 bg-emerald-500/10 border border-emerald-500/20 px-2.5 py-1 rounded-lg">
        <span class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></span>
        <span>Control Panel Aktif</span>
      </div>
    </div>
  </div>

  <!-- Main View Area (Kata vs Kumite) -->
  {#if activeTab === "kata"}
    <!-- ==================== KATA COMPARATIVE SCORING DASHBOARD ==================== -->
    <div class="flex-1 flex flex-col gap-3 min-h-0 overflow-y-auto pr-1">
      <!-- Top Row: Match Timer Deck (Presetted 30s..300s, MM:SS) -->
      <div class="bg-[#181a20] border border-[#2b2d31] rounded-xl p-3 shadow-lg shrink-0">
        <div class="flex items-center justify-between flex-wrap gap-3">
          <div class="flex items-center space-x-3">
            <span class="text-xs font-mono font-bold uppercase tracking-wider text-amber-400 flex items-center gap-1.5">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              Timer Penampilan Kata
            </span>
            <!-- Digital Clock Display MM:SS -->
            <div class="bg-[#111216] border border-[#2b2d31] px-4 py-1.5 rounded-lg flex items-baseline space-x-1 shadow-inner">
              <span class="font-mono font-black text-2xl text-amber-300 tracking-wider">
                {formatTimeMMSS(timerSeconds)}
              </span>
              <span class="font-mono text-xs text-amber-500/70">.00</span>
            </div>
            <!-- Start/Pause Toggle -->
            <button
              type="button"
              onclick={toggleTimer}
              class="px-4 py-2 rounded-lg text-xs font-black uppercase tracking-wider transition shadow flex items-center space-x-1.5 {isTimerRunning ? 'bg-amber-600 hover:bg-amber-500 text-white' : 'bg-emerald-600 hover:bg-emerald-500 text-white active:scale-95'}"
            >
              {#if isTimerRunning}
                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>
                <span>Jeda (Spasi)</span>
              {:else}
                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>
                <span>Mulai (Spasi)</span>
              {/if}
            </button>
            <!-- Reset Button -->
            <button
              type="button"
              onclick={resetTimer}
              class="px-3 py-2 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 text-xs font-bold rounded-lg transition"
            >
              Reset
            </button>
            <!-- Offset Steppers -->
            <div class="flex items-center space-x-1">
              <button
                type="button"
                onclick={() => adjustTimer(-1)}
                class="px-2 py-2 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-xs font-mono font-bold text-zinc-300 rounded-md"
                title="Kurangi 1 detik"
              >
                -1s
              </button>
              <button
                type="button"
                onclick={() => adjustTimer(1)}
                class="px-2 py-2 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-xs font-mono font-bold text-zinc-300 rounded-md"
                title="Tambah 1 detik"
              >
                +1s
              </button>
            </div>
          </div>

          <!-- Presets Bar: 30, 60, 90, 120, 150, 180, 300 -->
          <div class="flex items-center space-x-1.5 bg-[#111216] border border-[#2b2d31] rounded-lg p-1 text-xs font-mono">
            <span class="text-[10px] text-zinc-500 uppercase px-1 font-bold">Preset:</span>
            {#each TIMER_PRESETS as preset}
              <button
                type="button"
                onclick={() => selectTimerPreset(preset)}
                class="px-2.5 py-1 rounded-md font-bold transition {selectedPresetSeconds === preset ? 'bg-[#5865f2] text-white shadow' : 'text-zinc-400 hover:text-white hover:bg-white/[0.04]'}"
              >
                {formatPresetLabel(preset)}
              </button>
            {/each}
          </div>
        </div>
      </div>

      <!-- KATA 5-JUDGE COMPARATIVE SCORING TABLE -->
      <div class="flex-1 bg-[#181a20] border border-[#2b2d31] rounded-xl p-4 shadow-xl flex flex-col justify-between">
        <div>
          <!-- Header Bar with Kiken Overrides -->
          <div class="flex items-center justify-between pb-3 mb-3 border-b border-[#2b2d31] flex-wrap gap-2">
            <div>
              <h2 class="text-sm font-black text-white uppercase tracking-wider flex items-center gap-2">
                <svg class="w-4 h-4 text-amber-400" fill="currentColor" viewBox="0 0 20 20">
                  <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                </svg>
                Penilaian Komparatif Juri Kata (5 Juri)
              </h2>
              <p class="text-[11px] text-zinc-400 mt-0.5">
                Nilai manual 5.0 s/d 10.0 per juri. Juri dengan skor lebih tinggi otomatis memberikan +1 poin bendera ke sudut tersebut.
              </p>
            </div>

            <!-- Kiken Buttons (Clean: Kiken AKA & Kiken AO, without DQ / Aka Menang text) -->
            <div class="flex items-center space-x-2">
              <button
                type="button"
                onclick={() => applyKiken("aka")}
                class="px-3 py-1.5 bg-red-600/20 hover:bg-red-600/30 border border-red-500/50 text-red-300 text-xs font-bold rounded-lg transition active:scale-95"
                title="Kiken Sudut Merah AKA (Semua Juri 0.0)"
              >
                Kiken AKA
              </button>
              <button
                type="button"
                onclick={() => applyKiken("ao")}
                class="px-3 py-1.5 bg-blue-600/20 hover:bg-blue-600/30 border border-blue-500/50 text-blue-300 text-xs font-bold rounded-lg transition active:scale-95"
                title="Kiken Sudut Biru AO (Semua Juri 0.0)"
              >
                Kiken AO
              </button>
            </div>
          </div>

          <!-- The Table -->
          <div class="overflow-x-auto">
            <table class="w-full text-center border-collapse">
              <thead>
                <tr class="text-[11px] font-mono font-bold uppercase tracking-wider text-zinc-400 border-b border-[#2b2d31]">
                  <th class="py-2.5 px-3 text-left w-[280px]">Sudut & Atlet</th>
                  {#each [1, 2, 3, 4, 5] as jNum, idx}
                    {@const decision = judgeComparisons.decisions[idx]}
                    <th class="py-2.5 px-2 w-[110px] transition-colors rounded-t-lg {decision?.winner === 'aka' ? 'bg-red-500/20 text-red-300 border-t-2 border-red-500' : decision?.winner === 'ao' ? 'bg-blue-500/20 text-blue-300 border-t-2 border-blue-500' : 'bg-transparent text-zinc-400'}">
                      <div>JURI {jNum}</div>
                      {#if decision?.winner === "aka"}
                        <div class="text-[9px] font-black text-red-400 mt-0.5">AKA (+1)</div>
                      {:else if decision?.winner === "ao"}
                        <div class="text-[9px] font-black text-blue-400 mt-0.5">AO (+1)</div>
                      {:else if decision?.winner === "seri"}
                        <div class="text-[9px] font-bold text-zinc-500 mt-0.5">SERI</div>
                      {:else}
                        <div class="text-[9px] font-normal text-zinc-600 mt-0.5">-</div>
                      {/if}
                    </th>
                  {/each}
                  <th class="py-2.5 px-3 text-left w-[200px]">Pilihan Kata</th>
                  <th class="py-2.5 px-3 w-[120px] text-center">Total Poin</th>
                  <th class="py-2.5 px-3 w-[140px] text-center">Status</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-[#2b2d31]">
                <!-- AKA (RED) ROW -->
                <tr class="hover:bg-red-500/5 transition">
                  <td class="py-3 px-3 text-left">
                    <div class="flex items-center space-x-2">
                      <span class="px-2 py-0.5 bg-red-600 text-white font-mono font-black text-[10px] rounded shrink-0">
                        AKA
                      </span>
                      <span class="font-bold text-sm text-white leading-tight">
                        {match.atlet1_nama || "Sudut Merah"}
                      </span>
                    </div>
                    <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
                    <div class="text-xs text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                      {match.atlet1_kontingen || "-"}
                    </div>
                  </td>

                  <!-- 5 Judge Input Columns for AKA -->
                  {#each [0, 1, 2, 3, 4] as idx}
                    {@const decision = judgeComparisons.decisions[idx]}
                    <td class="py-2.5 px-1.5 {decision?.winner === 'aka' ? 'bg-red-500/10' : ''}">
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="10"
                        placeholder="0.0"
                        value={akaJudgeScores[idx]}
                        oninput={(e) => updateJudgeScore("aka", idx, e.currentTarget.value)}
                        class="w-20 h-11 bg-[#111216] border-2 {decision?.winner === 'aka' ? 'border-red-500 text-red-200 ring-2 ring-red-500/30' : 'border-[#2b2d31] text-zinc-200'} focus:border-red-400 text-center font-mono font-bold text-lg rounded-lg outline-none transition"
                      />
                    </td>
                  {/each}

                  <!-- Kata Selection (Searchable Discord Style) -->
                  <td class="py-2.5 px-3 text-left">
                    <button
                      type="button"
                      onclick={() => openKataModal("aka")}
                      class="w-full h-11 px-3 bg-[#111216] hover:bg-[#1e2029] border border-[#2b2d31] hover:border-white/20 rounded-lg text-left flex items-center justify-between text-xs font-semibold text-zinc-200 transition"
                      title="Klik untuk memilih Kata"
                    >
                      <span class="truncate">{kataAkaName || "Pilih Kata..."}</span>
                      <svg class="w-4 h-4 text-zinc-400 shrink-0 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                  </td>

                  <!-- Total Flags / Points for AKA -->
                  <td class="py-2.5 px-3">
                    <div class="h-11 bg-[#111216] border-2 {judgeComparisons.akaPoints > judgeComparisons.aoPoints ? 'border-red-500 text-red-400 bg-red-500/10' : 'border-[#2b2d31] text-zinc-300'} font-mono font-black text-2xl flex items-center justify-center rounded-lg shadow-inner">
                      {judgeComparisons.akaPoints}
                    </div>
                  </td>

                  <!-- Winner Status Button for AKA -->
                  <td class="py-2.5 px-3">
                    <button
                      type="button"
                      onclick={() => selectWinner("1")}
                      class="w-full py-2.5 rounded-lg text-xs font-black transition flex items-center justify-center gap-1.5 shadow active:scale-95 {matchWinner === '1' ? 'bg-red-600 text-white shadow-red-600/40 border border-red-400' : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-[#2b2d31]'}"
                    >
                      {#if matchWinner === "1"}
                        <svg class="w-3.5 h-3.5 text-amber-300" fill="currentColor" viewBox="0 0 20 20">
                          <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                        </svg>
                      {/if}
                      <span>AKA Menang</span>
                    </button>
                  </td>
                </tr>

                <!-- AO (BLUE) ROW -->
                <tr class="hover:bg-blue-500/5 transition">
                  <td class="py-3 px-3 text-left">
                    <div class="flex items-center space-x-2">
                      <span class="px-2 py-0.5 bg-blue-600 text-white font-mono font-black text-[10px] rounded shrink-0">
                        AO
                      </span>
                      <span class="font-bold text-sm text-white leading-tight">
                        {match.atlet2_nama || "Sudut Biru"}
                      </span>
                    </div>
                    <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
                    <div class="text-xs text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                      {match.atlet2_kontingen || "-"}
                    </div>
                  </td>

                  <!-- 5 Judge Input Columns for AO -->
                  {#each [0, 1, 2, 3, 4] as idx}
                    {@const decision = judgeComparisons.decisions[idx]}
                    <td class="py-2.5 px-1.5 {decision?.winner === 'ao' ? 'bg-blue-500/10' : ''}">
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="10"
                        placeholder="0.0"
                        value={aoJudgeScores[idx]}
                        oninput={(e) => updateJudgeScore("ao", idx, e.currentTarget.value)}
                        class="w-20 h-11 bg-[#111216] border-2 {decision?.winner === 'ao' ? 'border-blue-500 text-blue-200 ring-2 ring-blue-500/30' : 'border-[#2b2d31] text-zinc-200'} focus:border-blue-400 text-center font-mono font-bold text-lg rounded-lg outline-none transition"
                      />
                    </td>
                  {/each}

                  <!-- Kata Selection (Searchable Discord Style) -->
                  <td class="py-2.5 px-3 text-left">
                    <button
                      type="button"
                      onclick={() => openKataModal("ao")}
                      class="w-full h-11 px-3 bg-[#111216] hover:bg-[#1e2029] border border-[#2b2d31] hover:border-white/20 rounded-lg text-left flex items-center justify-between text-xs font-semibold text-zinc-200 transition"
                      title="Klik untuk memilih Kata"
                    >
                      <span class="truncate">{kataAoName || "Pilih Kata..."}</span>
                      <svg class="w-4 h-4 text-zinc-400 shrink-0 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                  </td>

                  <!-- Total Flags / Points for AO -->
                  <td class="py-2.5 px-3">
                    <div class="h-11 bg-[#111216] border-2 {judgeComparisons.aoPoints > judgeComparisons.akaPoints ? 'border-blue-500 text-blue-400 bg-blue-500/10' : 'border-[#2b2d31] text-zinc-300'} font-mono font-black text-2xl flex items-center justify-center rounded-lg shadow-inner">
                      {judgeComparisons.aoPoints}
                    </div>
                  </td>

                  <!-- Winner Status Button for AO -->
                  <td class="py-2.5 px-3">
                    <button
                      type="button"
                      onclick={() => selectWinner("2")}
                      class="w-full py-2.5 rounded-lg text-xs font-black transition flex items-center justify-center gap-1.5 shadow active:scale-95 {matchWinner === '2' ? 'bg-blue-600 text-white shadow-blue-600/40 border border-blue-400' : 'bg-[#1e293b] text-zinc-400 hover:text-white border border-[#2b2d31]'}"
                    >
                      {#if matchWinner === "2"}
                        <svg class="w-3.5 h-3.5 text-amber-300" fill="currentColor" viewBox="0 0 20 20">
                          <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                        </svg>
                      {/if}
                      <span>AO Menang</span>
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- Comparative Summary Footer -->
        <div class="mt-4 pt-3 border-t border-[#2b2d31] flex items-center justify-between text-xs text-zinc-400">
          <div class="flex items-center space-x-2">
            <span class="font-semibold text-zinc-300">Hasil Komparasi:</span>
            <span class="px-2 py-0.5 rounded font-mono font-black {judgeComparisons.akaPoints > judgeComparisons.aoPoints ? 'bg-red-500/20 text-red-400 border border-red-500/40' : judgeComparisons.aoPoints > judgeComparisons.akaPoints ? 'bg-blue-500/20 text-blue-400 border border-blue-500/40' : 'bg-zinc-800 text-zinc-400'}">
              AKA ({judgeComparisons.akaPoints}) - AO ({judgeComparisons.aoPoints})
            </span>
          </div>
          <div class="flex items-center space-x-2">
            <label class="flex items-center space-x-2 cursor-pointer text-xs text-zinc-300">
              <input type="checkbox" bind:checked={isMatchFinished} class="rounded text-emerald-600 focus:ring-emerald-500" />
              <span>Tandai Partai Ini Selesai</span>
            </label>
          </div>
        </div>
      </div>
    </div>

  {:else}
    <!-- ==================== KUMITE 3-COLUMN COCKPIT ==================== -->
    <div class="flex-1 grid grid-cols-1 lg:grid-cols-12 gap-3 min-h-0 overflow-hidden">
      <!-- POD KIRI: AO (BLUE) -->
      <div class="lg:col-span-5 bg-[#181a20] border-t-4 border-t-blue-600 border border-[#2b2d31] rounded-xl p-4 flex flex-col justify-between shadow-xl">
        <div>
          <!-- Header AO -->
          <div class="flex items-start justify-between gap-2 pb-3 mb-3 border-b border-[#2b2d31]">
            <div class="min-w-0 flex-1">
              <span class="px-2 py-0.5 bg-blue-600 text-white font-mono font-black text-[10px] rounded tracking-wider uppercase inline-block mb-1">
                AO • BIRU
              </span>
              <div class="text-xl md:text-2xl font-black text-white uppercase leading-tight">
                {match.atlet2_nama || "Atlet Biru"}
              </div>
              <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
              <div class="text-xs font-semibold text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                {match.atlet2_kontingen || "-"}
              </div>
            </div>

            <button
              type="button"
              onclick={() => selectWinner("2")}
              class="px-3 py-1.5 rounded-lg text-xs font-black uppercase tracking-wider transition shrink-0 flex items-center gap-1.5 {matchWinner === '2' ? 'bg-gradient-to-r from-blue-600 to-blue-700 text-white border-2 border-blue-400 shadow-lg shadow-blue-600/60' : 'bg-[#1e2029] border border-[#2b2d31] text-zinc-300 hover:text-white'}"
            >
              <svg class="w-4 h-4 {matchWinner === '2' ? 'text-amber-300' : 'text-zinc-400'}" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
              </svg>
              <span>AO Menang</span>
            </button>
          </div>

          <!-- Score & Senshu Row -->
          <div class="flex items-center gap-3 mb-4">
            <div class="flex-1 h-24 bg-[#111216] border-2 border-blue-600 rounded-xl flex items-center justify-center shadow-inner">
              <span class="text-6xl md:text-7xl font-mono font-black text-blue-400 tracking-tight">
                {aoScore}
              </span>
            </div>

            <!-- Senshu Ribbon -->
            <button
              type="button"
              onclick={() => toggleSenshu("ao")}
              class="w-24 h-24 rounded-xl flex flex-col items-center justify-center gap-1 font-black transition border-2 {aoSenshu ? 'bg-gradient-to-br from-amber-400 to-amber-600 text-slate-950 border-white shadow-lg shadow-amber-500/50' : 'bg-[#1e2029] text-amber-500 border-amber-600/50 hover:bg-[#2b2d31]'}"
            >
              <svg class="w-7 h-7" fill="currentColor" viewBox="0 0 24 24">
                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
              </svg>
              <span class="text-[11px] tracking-wider font-mono">SENSHU</span>
            </button>
          </div>

          <!-- Fast Score Deck -->
          <div class="grid grid-cols-4 gap-2 mb-4">
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 1)}
              class="h-12 bg-gradient-to-br from-blue-600 to-blue-800 hover:from-blue-500 hover:to-blue-700 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +1 YUKO
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 2)}
              class="h-12 bg-gradient-to-br from-blue-600 to-blue-800 hover:from-blue-500 hover:to-blue-700 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +2 WAZA
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 3)}
              class="h-12 bg-gradient-to-br from-blue-600 to-blue-800 hover:from-blue-500 hover:to-blue-700 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +3 IPPON
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", -1)}
              class="h-12 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 font-mono font-black text-base rounded-lg active:scale-95 transition"
            >
              -1
            </button>
          </div>

          <!-- Penalties (C1..H) -->
          <div class="bg-[#111216] border border-[#2b2d31] rounded-xl p-3 mb-3">
            <div class="text-[10px] font-mono font-bold text-zinc-400 uppercase tracking-wider mb-2">Pelanggaran</div>
            <div class="grid grid-cols-5 gap-1.5 font-mono font-bold text-xs">
              {#each ["c1", "c2", "c3", "hc", "h"] as pCode}
                {@const isMarked = aoPenalties[pCode as keyof typeof aoPenalties]}
                <button
                  type="button"
                  onclick={() => togglePenalty("ao", pCode as any)}
                  class="h-9 rounded-md transition flex items-center justify-center uppercase {isMarked ? (pCode === 'h' ? 'bg-red-600 text-white shadow-lg' : 'bg-amber-500 text-black font-black') : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-white/5'}"
                >
                  {pCode.toUpperCase()}
                </button>
              {/each}
            </div>
          </div>
        </div>

        <!-- Video Review AO -->
        <div class="pt-2 border-t border-[#2b2d31] flex items-center justify-between text-xs text-zinc-400">
          <span>Video Review (VR):</span>
          <button
            type="button"
            onclick={() => (hasVr2 = !hasVr2)}
            class="px-2.5 py-1 rounded text-[11px] font-bold font-mono transition {hasVr2 ? 'bg-red-600/30 text-red-300 border border-red-500' : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-white/5'}"
          >
            {hasVr2 ? "KARTU HANGUS" : "TERSEDIA"}
          </button>
        </div>
      </div>

      <!-- HUB TENGAH: TIMER & STATUS -->
      <div class="lg:col-span-2 bg-[#181a20] border-t-4 border-t-amber-500 border border-[#2b2d31] rounded-xl p-4 flex flex-col items-center justify-between shadow-xl">
        <div class="w-full text-center">
          <span class="text-[10px] font-mono font-bold text-amber-400 uppercase tracking-wider block mb-2">
            Match Timer
          </span>
          <div class="w-full bg-[#111216] border-2 border-amber-500/70 rounded-xl py-3 px-2 shadow-inner mb-3">
            <span class="text-4xl md:text-5xl font-mono font-black text-amber-300 tracking-wider">
              {formatTimeMMSS(timerSeconds)}
            </span>
            <span class="text-xs font-mono text-amber-500/70 block mt-0.5">.00</span>
          </div>

          <button
            type="button"
            onclick={toggleTimer}
            class="w-full py-3 rounded-xl font-mono font-black text-sm uppercase tracking-wider transition shadow-lg active:scale-95 mb-2 flex items-center justify-center space-x-1.5 {isTimerRunning ? 'bg-amber-600 hover:bg-amber-500 text-white' : 'bg-emerald-600 hover:bg-emerald-500 text-white'}"
          >
            {#if isTimerRunning}
              <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>
              <span>PAUSE (SPASI)</span>
            {:else}
              <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>
              <span>START (SPASI)</span>
            {/if}
          </button>

          <div class="grid grid-cols-2 gap-1.5 mb-2 font-mono text-xs">
            <button type="button" onclick={() => adjustTimer(-1)} class="py-1.5 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 rounded font-bold text-zinc-300">-1s</button>
            <button type="button" onclick={() => adjustTimer(1)} class="py-1.5 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 rounded font-bold text-zinc-300">+1s</button>
          </div>

          <div class="grid grid-cols-2 gap-1.5 font-mono text-[11px] mb-2">
            {#each [60, 90, 120, 180] as p}
              <button
                type="button"
                onclick={() => selectTimerPreset(p)}
                class="py-1 bg-[#111216] hover:bg-[#1e2029] border border-white/10 rounded text-zinc-300 transition"
              >
                {formatPresetLabel(p)}
              </button>
            {/each}
          </div>

          <button
            type="button"
            onclick={resetTimer}
            class="w-full py-1 text-xs font-mono font-bold text-red-400 bg-red-600/10 hover:bg-red-600/20 border border-red-500/30 rounded transition"
          >
            Reset
          </button>
        </div>

        <div class="w-full pt-3 border-t border-[#2b2d31] flex flex-col items-center gap-1.5 text-xs text-zinc-400">
          <label class="flex items-center space-x-2 cursor-pointer font-semibold text-zinc-300">
            <input type="checkbox" bind:checked={isHantei} class="rounded text-amber-500 focus:ring-amber-400" />
            <span>Hantei</span>
          </label>
          <span class="text-[9px] font-mono text-zinc-500">Hotkeys: 1/2/3 & 7/8/9</span>
        </div>
      </div>

      <!-- POD KANAN: AKA (RED) -->
      <div class="lg:col-span-5 bg-[#181a20] border-t-4 border-t-red-600 border border-[#2b2d31] rounded-xl p-4 flex flex-col justify-between shadow-xl">
        <div>
          <!-- Header AKA -->
          <div class="flex items-start justify-between gap-2 pb-3 mb-3 border-b border-[#2b2d31]">
            <button
              type="button"
              onclick={() => selectWinner("1")}
              class="px-3 py-1.5 rounded-lg text-xs font-black uppercase tracking-wider transition shrink-0 flex items-center gap-1.5 {matchWinner === '1' ? 'bg-gradient-to-r from-red-600 to-red-700 text-white border-2 border-red-400 shadow-lg shadow-red-600/60' : 'bg-[#1e2029] border border-[#2b2d31] text-zinc-300 hover:text-white'}"
            >
              <svg class="w-4 h-4 {matchWinner === '1' ? 'text-amber-300' : 'text-zinc-400'}" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
              </svg>
              <span>AKA Menang</span>
            </button>

            <div class="min-w-0 flex-1 text-right">
              <span class="px-2 py-0.5 bg-red-600 text-white font-mono font-black text-[10px] rounded tracking-wider uppercase inline-block mb-1">
                AKA • MERAH
              </span>
              <div class="text-xl md:text-2xl font-black text-white uppercase leading-tight">
                {match.atlet1_nama || "Sudut Merah"}
              </div>
              <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
              <div class="text-xs font-semibold text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                {match.atlet1_kontingen || "-"}
              </div>
            </div>
          </div>

          <!-- Score & Senshu Row -->
          <div class="flex items-center gap-3 mb-4">
            <!-- Senshu Ribbon -->
            <button
              type="button"
              onclick={() => toggleSenshu("aka")}
              class="w-24 h-24 rounded-xl flex flex-col items-center justify-center gap-1 font-black transition border-2 {akaSenshu ? 'bg-gradient-to-br from-amber-400 to-amber-600 text-slate-950 border-white shadow-lg shadow-amber-500/50' : 'bg-[#1e2029] text-amber-500 border-amber-600/50 hover:bg-[#2b2d31]'}"
            >
              <svg class="w-7 h-7" fill="currentColor" viewBox="0 0 24 24">
                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
              </svg>
              <span class="text-[11px] tracking-wider font-mono">SENSHU</span>
            </button>

            <div class="flex-1 h-24 bg-[#111216] border-2 border-red-600 rounded-xl flex items-center justify-center shadow-inner">
              <span class="text-6xl md:text-7xl font-mono font-black text-red-400 tracking-tight">
                {akaScore}
              </span>
            </div>
          </div>

          <!-- Fast Score Deck -->
          <div class="grid grid-cols-4 gap-2 mb-4">
            <button
              type="button"
              onclick={() => addKumiteScore("aka", -1)}
              class="h-12 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 font-mono font-black text-base rounded-lg active:scale-95 transition"
            >
              -1
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 1)}
              class="h-12 bg-gradient-to-br from-red-600 to-red-800 hover:from-red-500 hover:to-red-700 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +1 YUKO
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 2)}
              class="h-12 bg-gradient-to-br from-red-600 to-red-800 hover:from-red-500 hover:to-red-700 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +2 WAZA
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 3)}
              class="h-12 bg-gradient-to-br from-red-600 to-red-800 hover:from-red-500 hover:to-red-700 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +3 IPPON
            </button>
          </div>

          <!-- Penalties (C1..H) -->
          <div class="bg-[#111216] border border-[#2b2d31] rounded-xl p-3 mb-3">
            <div class="text-[10px] font-mono font-bold text-zinc-400 uppercase tracking-wider mb-2 text-right">Pelanggaran</div>
            <div class="grid grid-cols-5 gap-1.5 font-mono font-bold text-xs">
              {#each ["c1", "c2", "c3", "hc", "h"] as pCode}
                {@const isMarked = akaPenalties[pCode as keyof typeof akaPenalties]}
                <button
                  type="button"
                  onclick={() => togglePenalty("aka", pCode as any)}
                  class="h-9 rounded-md transition flex items-center justify-center uppercase {isMarked ? (pCode === 'h' ? 'bg-red-600 text-white shadow-lg' : 'bg-amber-500 text-black font-black') : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-white/5'}"
                >
                  {pCode.toUpperCase()}
                </button>
              {/each}
            </div>
          </div>
        </div>

        <!-- Video Review AKA -->
        <div class="pt-2 border-t border-[#2b2d31] flex items-center justify-between text-xs text-zinc-400">
          <span>Video Review (VR):</span>
          <button
            type="button"
            onclick={() => (hasVr1 = !hasVr1)}
            class="px-2.5 py-1 rounded text-[11px] font-bold font-mono transition {hasVr1 ? 'bg-red-600/30 text-red-300 border border-red-500' : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-white/5'}"
          >
            {hasVr1 ? "KARTU HANGUS" : "TERSEDIA"}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<!-- ==================== DISCORD-STYLED KATA SELECT MODAL ==================== -->
{#if isKataModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4">
    <!-- Click backdrop to close -->
    <button
      type="button"
      class="absolute inset-0 w-full h-full cursor-default"
      onclick={() => (isKataModalOpen = false)}
      tabindex="-1"
      aria-label="Tutup modal"
    ></button>

    <div class="relative w-full max-w-[500px] bg-[#181a20] border border-[#2b2d31] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[85vh] z-10 animate-in fade-in zoom-in-95 duration-100">
      <!-- Modal Header -->
      <div class="p-4 border-b border-[#2b2d31] flex items-center justify-between bg-[#14161b]">
        <div>
          <h3 class="text-sm font-black text-white uppercase tracking-wider flex items-center gap-2">
            <span class="px-2 py-0.5 text-[10px] rounded font-mono font-bold {kataModalTarget === 'aka' ? 'bg-red-600 text-white' : 'bg-blue-600 text-white'}">
              {kataModalTarget === "aka" ? "AKA • SUDUT MERAH" : "AO • SUDUT BIRU"}
            </span>
            <span>Pilih Gerakan Kata (WKF)</span>
          </h3>
          <p class="text-[11px] text-zinc-400 mt-0.5">Ketik nomor kode (e.g. 99) atau nama kata untuk mencari.</p>
        </div>
        <button
          type="button"
          onclick={() => (isKataModalOpen = false)}
          aria-label="Tutup modal"
          class="w-7 h-7 flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/10 rounded-lg transition"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <!-- Search Input Box -->
      <div class="p-3 bg-[#111216] border-b border-[#2b2d31]">
        <div class="relative flex items-center">
          <svg class="w-4 h-4 text-zinc-400 absolute left-3 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            bind:this={searchInputEl}
            type="text"
            bind:value={kataSearchQuery}
            placeholder="Cari kode nomor (99) atau nama (Unsu)..."
            class="w-full h-10 pl-9 pr-8 bg-[#181a20] border border-[#2b2d31] focus:border-[#5865f2] focus:ring-2 focus:ring-[#5865f2]/30 text-white text-xs font-semibold rounded-xl outline-none transition placeholder-zinc-500"
          />
          {#if kataSearchQuery}
            <button
              type="button"
              onclick={() => (kataSearchQuery = "")}
              aria-label="Bersihkan pencarian"
              class="absolute right-2.5 w-5 h-5 flex items-center justify-center text-zinc-400 hover:text-white"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
            </button>
          {/if}
        </div>
      </div>

      <!-- Kata Items List -->
      <div class="flex-1 overflow-y-auto p-2 divide-y divide-[#2b2d31]/40 scrollbar-thin max-h-[380px]">
        {#if filteredKataList.length === 0}
          <div class="p-8 text-center text-zinc-500 text-xs font-medium">
            Tidak ada kata yang cocok dengan "{kataSearchQuery}"
          </div>
        {:else}
          {#each filteredKataList as item, idx}
            <button
              type="button"
              onclick={() => selectKataItem(item)}
              class="w-full px-3 py-2.5 rounded-lg flex items-center justify-between text-left transition {idx === kataModalSelectedIndex ? 'bg-[#5865f2]/20 border border-[#5865f2] text-white' : 'hover:bg-white/[0.04] text-zinc-300'}"
            >
              <div class="flex items-center space-x-2.5">
                <span class="px-2 py-0.5 bg-[#111216] border border-[#2b2d31] text-amber-400 font-mono font-bold text-xs rounded">
                  {item.code}
                </span>
                <span class="font-bold text-sm text-white">
                  {item.name}
                </span>
              </div>
              <span class="text-[11px] font-mono text-zinc-500">
                WKF #{item.code}
              </span>
            </button>
          {/each}
        {/if}
      </div>

      <!-- Modal Footer -->
      <div class="p-2.5 bg-[#14161b] border-t border-[#2b2d31] flex items-center justify-between text-[11px] text-zinc-400 font-mono">
        <span>Gunakan tombol ↑ ↓ dan Enter untuk navigasi</span>
        <button
          type="button"
          onclick={() => {
            if (kataModalTarget === "aka") kataAkaName = "";
            else kataAoName = "";
            isKataModalOpen = false;
          }}
          class="text-red-400 hover:underline font-bold"
        >
          Kosongkan Pilihan
        </button>
      </div>
    </div>
  </div>
{/if}
