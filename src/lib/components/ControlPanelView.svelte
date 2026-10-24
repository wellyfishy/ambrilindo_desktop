<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { DetailBaganItem, BaganItem, SaveMatchInput } from "$lib/types";

  interface Props {
    match: DetailBaganItem;
    baganInfo?: BaganItem | null;
    baganName: string;
    eventName?: string;
    tatamiNumber?: number;
    hostUrl?: string;
    logoMap?: Record<string, string>;
    onBack: () => void;
    onSave: (input: SaveMatchInput) => Promise<void>;
  }

  let {
    match,
    baganInfo = null,
    baganName,
    eventName = "",
    tatamiNumber = 1,
    hostUrl = "",
    logoMap = {},
    onBack,
    onSave,
  }: Props = $props();

  // Detect Kata vs Kumite
  let isKata = $derived(
    baganName.toLowerCase().includes("kata") ||
    (baganInfo?.tipe_tanding === "1") ||
    (baganInfo?.tipe_tanding === "kata")
  );

  // Common Match State
  let matchWinner = $state<string>(match.pemenang || "");
  let isMatchFinished = $state<boolean>(match.selesai || false);
  let isSaving = $state<boolean>(false);
  let hasVr1 = $state<boolean>(match.vr1 || false);
  let hasVr2 = $state<boolean>(match.vr2 || false);
  let isHantei = $state<boolean>(match.hantei || false);

  // Kumite Score State
  let akaScore = $state<number>(parseInt(match.score1 || "0", 10) || 0);
  let aoScore = $state<number>(parseInt(match.score2 || "0", 10) || 0);
  let akaSenshu = $state<boolean>(false);
  let aoSenshu = $state<boolean>(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Kumite Timer State
  let timerSeconds = $state<number>(120);
  let timerMs = $state<number>(0);
  let isTimerRunning = $state<boolean>(false);
  let timerInterval: any = null;

  // Kata 5-Judges State
  let kataAkaName = $state<string>(match.kata1 || "");
  let kataAoName = $state<string>(match.kata2 || "");
  let akaJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let aoJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let akaTotalScore = $state<string>(match.score1 || "");
  let aoTotalScore = $state<string>(match.score2 || "");

  // Common Karate Kata List for Autocomplete
  const commonKatas = [
    "Anan", "Anan Dai", "Ananko", "Chatanyara Kushanku", "Chinto", "Enpi",
    "Gankaku", "Gojushiho", "Gojushiho Dai", "Gojushiho Sho", "Kanku Dai",
    "Kanku Sho", "Kururunfa", "Kusanku", "Matsumura Bassai", "Nipaipo",
    "Ohan", "Ohan Dai", "Paiku", "Papuren", "Rohai", "Saifa", "Seichin",
    "Seienchin", "Seipai", "Seisan", "Shinpa", "Shisochin", "Sochin",
    "Suparinpei", "Tomari Bassai", "Unsu"
  ];

  // Helper for Round title
  function getRoundTitle(round: number | null): string {
    switch (round) {
      case 1: return "BABAK 16 BESAR";
      case 2: return "PEREMPAT FINAL";
      case 3: return "SEMI FINAL";
      case 4: return "PARTAI FINAL";
      default: return `BABAK ${round || 1}`;
    }
  }

  // Format timer
  let formattedMinutes = $derived(Math.floor(timerSeconds / 60).toString().padStart(2, "0"));
  let formattedSeconds = $derived((timerSeconds % 60).toString().padStart(2, "0"));
  let formattedMs = $derived(Math.floor(timerMs / 10).toString().padStart(2, "0"));

  function toggleTimer() {
    if (isTimerRunning) {
      clearInterval(timerInterval);
      timerInterval = null;
      isTimerRunning = false;
    } else {
      if (timerSeconds <= 0 && timerMs <= 0) {
        timerSeconds = 120;
        timerMs = 0;
      }
      isTimerRunning = true;
      const startTime = Date.now();
      const totalRemainingMs = timerSeconds * 1000 + timerMs;

      timerInterval = setInterval(() => {
        const elapsed = Date.now() - startTime;
        const currentRemaining = Math.max(0, totalRemainingMs - elapsed);
        timerSeconds = Math.floor(currentRemaining / 1000);
        timerMs = currentRemaining % 1000;

        if (timerSeconds === 15 && timerMs < 50) {
          triggerBuzzer("warning");
        }

        if (currentRemaining <= 0) {
          clearInterval(timerInterval);
          timerInterval = null;
          isTimerRunning = false;
          timerSeconds = 0;
          timerMs = 0;
          triggerBuzzer("finish");
        }
      }, 50);
    }
  }

  function adjustTimer(deltaSec: number) {
    timerSeconds = Math.max(0, timerSeconds + deltaSec);
  }

  function setTimerPreset(sec: number) {
    if (timerInterval) clearInterval(timerInterval);
    timerInterval = null;
    isTimerRunning = false;
    timerSeconds = sec;
    timerMs = 0;
  }

  async function triggerBuzzer(sound: "warning" | "finish") {
    try {
      await invoke("play_sound", { soundType: sound });
    } catch {
      // ignore
    }
  }

  // Kumite Scoring
  function addKumiteScore(side: "aka" | "ao", pts: number) {
    if (side === "aka") {
      akaScore = Math.max(0, akaScore + pts);
      if (pts > 0 && !akaSenshu && !aoSenshu) {
        akaSenshu = true;
      }
    } else {
      aoScore = Math.max(0, aoScore + pts);
      if (pts > 0 && !akaSenshu && !aoSenshu) {
        aoSenshu = true;
      }
    }
    autoSuggestWinner();
  }

  function toggleSenshu(side: "aka" | "ao") {
    if (side === "aka") {
      akaSenshu = !akaSenshu;
      if (akaSenshu) aoSenshu = false;
    } else {
      aoSenshu = !aoSenshu;
      if (aoSenshu) akaSenshu = false;
    }
  }

  function togglePenalty(side: "aka" | "ao", key: "c1" | "c2" | "c3" | "hc" | "h") {
    if (side === "aka") {
      akaPenalties[key] = !akaPenalties[key];
    } else {
      aoPenalties[key] = !aoPenalties[key];
    }
  }

  function setCriteriaLevel(side: "aka" | "ao", level: number) {
    const p = {
      c1: level >= 1,
      c2: level >= 2,
      c3: level >= 3,
      hc: level >= 4,
      h: level >= 5,
    };
    if (side === "aka") akaPenalties = p;
    else aoPenalties = p;
  }

  function autoSuggestWinner() {
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
      isMatchFinished = false;
    } else {
      matchWinner = winner;
      isMatchFinished = true;
    }
  }

  // Kata Scoring Calculation
  function calculateKataTotal(scores: string[]): number {
    const valid = scores
      .map((s) => parseFloat(s.trim()))
      .filter((n) => !isNaN(n) && n > 0);
    if (valid.length === 0) return 0;
    // Standard sum of judges
    const sum = valid.reduce((acc, curr) => acc + curr, 0);
    return Math.round(sum * 100) / 100;
  }

  function updateKataAkaScore(idx: number, val: string) {
    akaJudgeScores[idx] = val;
    const tot = calculateKataTotal(akaJudgeScores);
    akaTotalScore = tot > 0 ? tot.toFixed(2) : "";
    suggestKataWinner();
  }

  function updateKataAoScore(idx: number, val: string) {
    aoJudgeScores[idx] = val;
    const tot = calculateKataTotal(aoJudgeScores);
    aoTotalScore = tot > 0 ? tot.toFixed(2) : "";
    suggestKataWinner();
  }

  function suggestKataWinner() {
    const akaN = parseFloat(akaTotalScore) || 0;
    const aoN = parseFloat(aoTotalScore) || 0;
    if (akaN > aoN) {
      matchWinner = "1";
    } else if (aoN > akaN) {
      matchWinner = "2";
    }
  }

  function chiefOverrideDQ(dqSide: "aka" | "ao") {
    if (dqSide === "aka") {
      akaTotalScore = "0.00";
      matchWinner = "2";
      isMatchFinished = true;
    } else {
      aoTotalScore = "0.00";
      matchWinner = "1";
      isMatchFinished = true;
    }
  }

  async function handleSaveAndExit() {
    if (isSaving) return;
    isSaving = true;

    try {
      const payload: SaveMatchInput = {
        match_id: match.id,
        bagan_id: match.bagan_id || baganInfo?.id || 1,
        score1: isKata ? (akaTotalScore || "0") : akaScore.toString(),
        score2: isKata ? (aoTotalScore || "0") : aoScore.toString(),
        scorekecil1: null,
        scorekecil2: null,
        pemenang: matchWinner || null,
        kata1: isKata ? kataAkaName : null,
        kata2: isKata ? kataAoName : null,
        vr1: hasVr1,
        vr2: hasVr2,
        hantei: isHantei,
        selesai: isMatchFinished || !!matchWinner,
      };

      await onSave(payload);
    } catch (err: any) {
      alert("Gagal menyimpan hasil partai: " + err);
    } finally {
      isSaving = false;
    }
  }

  // Keyboard shortcut listener
  function handleKeyDown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) {
      return;
    }

    if (e.key === "Escape") {
      onBack();
    } else if (e.code === "Space" && !isKata) {
      e.preventDefault();
      toggleTimer();
    } else if (!isKata) {
      // Kumite fast hotkeys (Spatial layout: AO on left keys 1/2/3, AKA on right keys 7/8/9)
      if (e.key === "1") { e.preventDefault(); addKumiteScore("ao", 1); }
      else if (e.key === "2") { e.preventDefault(); addKumiteScore("ao", 2); }
      else if (e.key === "3") { e.preventDefault(); addKumiteScore("ao", 3); }
      else if (e.key === "7") { e.preventDefault(); addKumiteScore("aka", 1); }
      else if (e.key === "8") { e.preventDefault(); addKumiteScore("aka", 2); }
      else if (e.key === "9") { e.preventDefault(); addKumiteScore("aka", 3); }
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
  });

  onDestroy(() => {
    if (timerInterval) clearInterval(timerInterval);
    window.removeEventListener("keydown", handleKeyDown);
  });
</script>

<div class="h-full flex flex-col p-4 bg-[#030712] text-white select-none overflow-y-auto font-sans">
  <!-- Top Cockpit Header Bar -->
  <div class="cockpit-page-header flex items-center justify-between flex-wrap gap-3 p-3 bg-[#0f172a] border border-[#334155] rounded-xl shadow-lg mb-4 shrink-0">
    <div class="flex items-center gap-2">
      <!-- Back Button -->
      <button
        type="button"
        onclick={onBack}
        class="px-3.5 py-1.5 bg-[#1e293b] hover:bg-[#334155] border border-[#475569] text-white text-xs font-bold rounded-lg flex items-center gap-1.5 transition active:scale-95"
      >
        <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
          <path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/>
        </svg>
        <span>Kembali ke Bagan</span>
      </button>

      <!-- Save & Exit Button -->
      <button
        type="button"
        onclick={handleSaveAndExit}
        disabled={isSaving}
        class="px-4 py-1.5 bg-gradient-to-r from-emerald-600 to-emerald-700 hover:from-emerald-500 hover:to-emerald-600 border border-emerald-400 text-white text-xs font-black rounded-lg flex items-center gap-1.5 shadow-md shadow-emerald-900/40 transition active:scale-95 disabled:opacity-50"
      >
        {#if isSaving}
          <svg class="w-3.5 h-3.5 animate-spin" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
          <span>Menyimpan...</span>
        {:else}
          <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
            <path d="M17 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V7l-4-4zm-5 16c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3zm3-10H5V5h10v4z"/>
          </svg>
          <span>Simpan & Keluar</span>
        {/if}
      </button>
    </div>

    <!-- Match Identity Pill -->
    <div class="match-identity-pill flex items-center gap-2.5 px-3 py-1 bg-[#020617] border border-[#475569] rounded-lg text-xs font-bold shadow-inner">
      <span class="px-2 py-0.5 rounded text-[10px] font-black tracking-wider uppercase {isKata ? 'bg-amber-500/20 text-amber-400 border border-amber-500/60' : 'bg-blue-500/20 text-blue-400 border border-blue-500/60'}">
        {isKata ? "KATA" : "KUMITE"}
      </span>
      <span class="text-white font-extrabold">{baganName}</span>
      <span class="text-zinc-500">•</span>
      <span class="text-zinc-300 font-mono">MATCH {match.urutan || 1}</span>
      <span class="text-zinc-500">•</span>
      <span class="text-zinc-400">{getRoundTitle(match.round)}</span>
      <span class="text-zinc-500">•</span>
      <span class="text-amber-400 font-mono">Tatami {tatamiNumber}</span>
    </div>

    <!-- Status & Sync Badge -->
    <div class="flex items-center gap-2">
      <span class="px-2.5 py-1 bg-emerald-500/15 border border-emerald-500/40 text-emerald-400 text-[11px] font-bold rounded-md flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></span>
        <span>Control Panel Aktif</span>
      </span>
      {#if match.selesai}
        <span class="px-2.5 py-1 bg-amber-500/15 border border-amber-500/40 text-amber-300 text-[11px] font-bold rounded-md">
          Status: Selesai
        </span>
      {/if}
    </div>
  </div>

  {#if isKata}
    <!-- ==================== KATA MATCH OPERATOR PANEL (5 JUDGES) ==================== -->
    <div class="flex-1 flex flex-col space-y-4">
      <div class="bg-[#0f172a] border border-[#334155] rounded-xl p-4 shadow-xl">
        <div class="flex items-center justify-between pb-3 mb-3 border-b border-[#334155]">
          <div class="flex items-center gap-3">
            <h2 class="text-sm font-black text-amber-400 uppercase tracking-wide flex items-center gap-2">
              <svg class="w-4 h-4 fill-current" viewBox="0 0 24 24">
                <path d="M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z"/>
              </svg>
              <span>Penilaian Juri Kata (5 Juri)</span>
            </h2>
            <span class="text-xs text-zinc-400 hidden md:inline">Rentang nilai 5.0 - 10.0 • 0.0 (DQ / Kiken)</span>
          </div>

          <!-- Chief Referee Override Dropdown -->
          <div class="flex items-center gap-2">
            <button
              type="button"
              onclick={() => chiefOverrideDQ("aka")}
              class="px-2.5 py-1 bg-red-950/60 hover:bg-red-900 border border-red-700 text-red-300 rounded text-xs font-bold transition"
              title="Disqualify AKA (AO Menang)"
            >
              DQ AKA (AO Menang)
            </button>
            <button
              type="button"
              onclick={() => chiefOverrideDQ("ao")}
              class="px-2.5 py-1 bg-blue-950/60 hover:bg-blue-900 border border-blue-700 text-blue-300 rounded text-xs font-bold transition"
              title="Disqualify AO (AKA Menang)"
            >
              DQ AO (AKA Menang)
            </button>
          </div>
        </div>

        <!-- 5-Judges Table -->
        <div class="overflow-x-auto">
          <table class="w-full text-center border-collapse">
            <thead>
              <tr class="text-xs font-black uppercase text-zinc-400 border-b border-[#334155]">
                <th class="py-2 px-4 text-left w-64">Sudut & Atlet</th>
                <th class="py-2 px-2">Juri 1</th>
                <th class="py-2 px-2">Juri 2</th>
                <th class="py-2 px-2">Juri 3</th>
                <th class="py-2 px-2">Juri 4</th>
                <th class="py-2 px-2">Juri 5</th>
                <th class="py-2 px-3">Nama Kata</th>
                <th class="py-2 px-4 text-amber-400 font-extrabold">Total Poin</th>
                <th class="py-2 px-4">Pemenang</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-[#334155]/60">
              <!-- AKA (Red) Row -->
              <tr class="hover:bg-red-500/5 transition">
                <td class="py-3 px-4 text-left">
                  <div class="flex items-center gap-2">
                    <span class="px-2 py-0.5 bg-red-600 text-white font-mono font-black text-[10px] rounded">AKA</span>
                    <span class="font-bold text-sm text-white truncate max-w-[180px]">{match.atlet1_nama || "Atlet Merah"}</span>
                  </div>
                  <div class="text-xs text-zinc-400 mt-0.5 truncate max-w-[220px]">{match.atlet1_kontingen || "-"}</div>
                </td>
                {#each [0, 1, 2, 3, 4] as idx}
                  <td class="py-2 px-1.5">
                    <input
                      type="number"
                      step="0.1"
                      min="0"
                      max="10"
                      placeholder="0.0"
                      value={akaJudgeScores[idx]}
                      oninput={(e) => updateKataAkaScore(idx, e.currentTarget.value)}
                      class="w-16 h-10 bg-[#1a0808] border-2 border-red-500/70 focus:border-red-400 focus:ring-2 focus:ring-red-500/40 text-center font-mono font-bold text-base text-white rounded-lg outline-none"
                    />
                  </td>
                {/each}
                <td class="py-2 px-2">
                  <input
                    type="text"
                    list="kata-list"
                    placeholder="Pilih Kata..."
                    bind:value={kataAkaName}
                    class="w-36 h-9 px-2 bg-[#020617] border border-[#475569] text-xs font-semibold text-white rounded-md outline-none focus:border-amber-400"
                  />
                </td>
                <td class="py-2 px-4">
                  <input
                    type="text"
                    bind:value={akaTotalScore}
                    class="w-20 h-10 bg-[#020617] border-2 border-amber-500 text-amber-300 font-mono font-black text-lg text-center rounded-lg"
                  />
                </td>
                <td class="py-2 px-4">
                  <button
                    type="button"
                    onclick={() => selectWinner("1")}
                    class="px-3 py-1.5 rounded-lg text-xs font-black transition flex items-center justify-center gap-1.5 mx-auto {matchWinner === '1' ? 'bg-red-600 text-white shadow-lg shadow-red-600/50' : 'bg-[#1e293b] text-zinc-400 hover:text-white border border-[#475569]'}"
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

              <!-- AO (Blue) Row -->
              <tr class="hover:bg-blue-500/5 transition">
                <td class="py-3 px-4 text-left">
                  <div class="flex items-center gap-2">
                    <span class="px-2 py-0.5 bg-blue-600 text-white font-mono font-black text-[10px] rounded">AO</span>
                    <span class="font-bold text-sm text-white truncate max-w-[180px]">{match.atlet2_nama || "Atlet Biru"}</span>
                  </div>
                  <div class="text-xs text-zinc-400 mt-0.5 truncate max-w-[220px]">{match.atlet2_kontingen || "-"}</div>
                </td>
                {#each [0, 1, 2, 3, 4] as idx}
                  <td class="py-2 px-1.5">
                    <input
                      type="number"
                      step="0.1"
                      min="0"
                      max="10"
                      placeholder="0.0"
                      value={aoJudgeScores[idx]}
                      oninput={(e) => updateKataAoScore(idx, e.currentTarget.value)}
                      class="w-16 h-10 bg-[#091326] border-2 border-blue-500/70 focus:border-blue-400 focus:ring-2 focus:ring-blue-500/40 text-center font-mono font-bold text-base text-white rounded-lg outline-none"
                    />
                  </td>
                {/each}
                <td class="py-2 px-2">
                  <input
                    type="text"
                    list="kata-list"
                    placeholder="Pilih Kata..."
                    bind:value={kataAoName}
                    class="w-36 h-9 px-2 bg-[#020617] border border-[#475569] text-xs font-semibold text-white rounded-md outline-none focus:border-amber-400"
                  />
                </td>
                <td class="py-2 px-4">
                  <input
                    type="text"
                    bind:value={aoTotalScore}
                    class="w-20 h-10 bg-[#020617] border-2 border-amber-500 text-amber-300 font-mono font-black text-lg text-center rounded-lg"
                  />
                </td>
                <td class="py-2 px-4">
                  <button
                    type="button"
                    onclick={() => selectWinner("2")}
                    class="px-3 py-1.5 rounded-lg text-xs font-black transition flex items-center justify-center gap-1.5 mx-auto {matchWinner === '2' ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/50' : 'bg-[#1e293b] text-zinc-400 hover:text-white border border-[#475569]'}"
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

        <datalist id="kata-list">
          {#each commonKatas as k}
            <option value={k}>{k}</option>
          {/each}
        </datalist>
      </div>
    </div>
  {:else}
    <!-- ==================== KUMITE COMMAND COCKPIT (3-COLUMN DASHBOARD) ==================== -->
    <div class="flex-1 grid grid-cols-1 lg:grid-cols-[1fr_310px_1fr] gap-4 items-stretch min-h-0">
      <!-- 1. LEFT: AO (BIRU) POD -->
      <div class="competitor-pod bg-[#0f172a] border-t-4 border-t-blue-500 border border-[#334155] rounded-xl p-4 flex flex-col justify-between shadow-xl">
        <div>
          <!-- Pod Header -->
          <div class="flex items-start justify-between pb-3 mb-3 border-b border-[#334155]">
            <div class="min-w-0 pr-2">
              <span class="inline-block px-2 py-0.5 bg-blue-600 text-white font-mono font-black text-[10px] rounded tracking-wider uppercase mb-1">
                AO • BIRU
              </span>
              <div class="text-xl md:text-2xl font-black text-white uppercase leading-tight truncate">
                {match.atlet2_nama || "Atlet Biru"}
              </div>
              <div class="text-xs font-semibold text-zinc-300 mt-1 truncate">
                {match.atlet2_kontingen || "-"}
              </div>
            </div>

            <button
              type="button"
              onclick={() => selectWinner("2")}
              class="px-3 py-1.5 rounded-lg text-xs font-black uppercase tracking-wider transition shrink-0 flex items-center gap-1.5 {matchWinner === '2' ? 'bg-gradient-to-r from-blue-600 to-blue-700 text-white border-2 border-blue-400 shadow-lg shadow-blue-600/60' : 'bg-[#1e293b] border border-[#475569] text-zinc-300 hover:text-white'}"
            >
              <svg class="w-4 h-4 {matchWinner === '2' ? 'text-amber-300' : 'text-zinc-400'}" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
              </svg>
              <span>AO Menang</span>
            </button>
          </div>

          <!-- Score & Senshu Row -->
          <div class="flex items-center gap-3 mb-4">
            <div class="flex-1 h-24 bg-[#020617] border-2 border-blue-600 rounded-xl flex items-center justify-center shadow-inner">
              <span class="text-6xl md:text-7xl font-mono font-black text-blue-400 tracking-tight">
                {aoScore}
              </span>
            </div>

            <!-- Senshu Ribbon -->
            <button
              type="button"
              onclick={() => toggleSenshu("ao")}
              class="w-24 h-24 rounded-xl flex flex-col items-center justify-center gap-1 font-black transition border-2 {aoSenshu ? 'bg-gradient-to-br from-amber-400 to-amber-600 text-slate-950 border-white shadow-lg shadow-amber-500/50' : 'bg-[#1e293b] text-amber-500 border-amber-600/50 hover:bg-[#334155]'}"
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
              class="h-12 bg-gradient-to-br from-blue-700 to-blue-900 hover:from-blue-600 hover:to-blue-800 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +2 WAZA
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 3)}
              class="h-12 bg-gradient-to-br from-blue-800 to-blue-950 hover:from-blue-700 hover:to-blue-900 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +3 IPPON
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", -1)}
              class="h-12 bg-[#1e293b] hover:bg-[#334155] border border-[#64748b] text-white font-black text-lg rounded-lg active:scale-95 transition flex items-center justify-center"
              title="Kurangi 1 Poin"
            >
              -1
            </button>
          </div>

          <!-- Penalties Strip -->
          <div class="bg-[#020617] border border-[#334155] rounded-xl p-2.5 flex items-center justify-between gap-1.5">
            <span class="text-[11px] font-black tracking-wider text-zinc-300 uppercase">Pelanggaran</span>
            <div class="flex items-center gap-1.5">
              <button
                type="button"
                onclick={() => togglePenalty("ao", "c1")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {aoPenalties.c1 ? 'bg-blue-600 text-white border-blue-400 shadow-md shadow-blue-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                C1
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("ao", "c2")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {aoPenalties.c2 ? 'bg-blue-600 text-white border-blue-400 shadow-md shadow-blue-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                C2
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("ao", "c3")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {aoPenalties.c3 ? 'bg-blue-600 text-white border-blue-400 shadow-md shadow-blue-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                C3
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("ao", "hc")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {aoPenalties.hc ? 'bg-blue-600 text-white border-blue-400 shadow-md shadow-blue-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                HC
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("ao", "h")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {aoPenalties.h ? 'bg-red-600 text-white border-red-400 shadow-md shadow-red-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                H
              </button>
            </div>
          </div>
        </div>

        <!-- Video Review Indicator -->
        <div class="mt-4 pt-3 border-t border-[#334155] flex items-center justify-between text-xs">
          <span class="font-bold text-zinc-400">Video Review (VR):</span>
          <button
            type="button"
            onclick={() => (hasVr2 = !hasVr2)}
            class="px-2.5 py-1 rounded font-mono font-bold text-xs border {hasVr2 ? 'bg-blue-600 text-white border-blue-400' : 'bg-[#1e293b] text-zinc-500 border-[#475569]'}"
          >
            {hasVr2 ? "VR AKTIF" : "KARTU HANGUS"}
          </button>
        </div>
      </div>

      <!-- 2. CENTER: MASTER MATCH CONTROL HUB -->
      <div class="cockpit-hub bg-[#0f172a] border-t-4 border-t-amber-500 border border-[#334155] rounded-xl p-4 flex flex-col items-center justify-between shadow-xl">
        <div class="w-full flex flex-col items-center">
          <span class="text-xs font-black uppercase text-amber-400 tracking-widest mb-2">Match Timer</span>

          <!-- Digital Timer Box -->
          <div class="w-full h-24 bg-[#020617] border-2 border-amber-500/80 rounded-xl flex items-center justify-center shadow-inner mb-4">
            <span class="font-mono text-5xl md:text-6xl font-black text-amber-300 tracking-tight">
              {formattedMinutes}:{formattedSeconds}
              <span class="text-2xl text-amber-500/70">.{formattedMs}</span>
            </span>
          </div>

          <!-- Master Start / Stop Button -->
          <button
            type="button"
            onclick={toggleTimer}
            class="w-full h-14 rounded-xl font-mono font-black text-lg uppercase tracking-wider flex items-center justify-center gap-2 transition active:scale-95 shadow-lg mb-4 {isTimerRunning ? 'bg-gradient-to-r from-amber-500 to-amber-600 text-slate-950 shadow-amber-500/30' : 'bg-gradient-to-r from-emerald-600 to-emerald-700 text-white shadow-emerald-600/30'}"
          >
            {#if isTimerRunning}
              <svg class="w-5 h-5 fill-current" viewBox="0 0 24 24">
                <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>
              </svg>
              <span>PAUSE (SPASI)</span>
            {:else}
              <svg class="w-5 h-5 fill-current" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z"/>
              </svg>
              <span>START (SPASI)</span>
            {/if}
          </button>

          <!-- Micro Adjust Row -->
          <div class="w-full flex items-center gap-2 mb-3">
            <button
              type="button"
              onclick={() => adjustTimer(-1)}
              class="flex-1 py-1.5 bg-[#1e293b] hover:bg-[#334155] border border-[#475569] text-xs font-bold rounded-lg transition"
            >
              -1s
            </button>
            <button
              type="button"
              onclick={() => adjustTimer(1)}
              class="flex-1 py-1.5 bg-[#1e293b] hover:bg-[#334155] border border-[#475569] text-xs font-bold rounded-lg transition"
            >
              +1s
            </button>
          </div>

          <!-- Presets Row -->
          <div class="w-full grid grid-cols-4 gap-1.5 mb-4">
            <button
              type="button"
              onclick={() => setTimerPreset(90)}
              class="py-1.5 bg-[#1e293b] hover:bg-[#334155] border border-[#475569] text-xs font-bold rounded-lg font-mono text-zinc-300"
            >
              1:30
            </button>
            <button
              type="button"
              onclick={() => setTimerPreset(120)}
              class="py-1.5 bg-[#1e293b] hover:bg-[#334155] border border-[#475569] text-xs font-bold rounded-lg font-mono text-zinc-300"
            >
              2:00
            </button>
            <button
              type="button"
              onclick={() => setTimerPreset(180)}
              class="py-1.5 bg-[#1e293b] hover:bg-[#334155] border border-[#475569] text-xs font-bold rounded-lg font-mono text-zinc-300"
            >
              3:00
            </button>
            <button
              type="button"
              onclick={() => setTimerPreset(0)}
              class="py-1.5 bg-red-950/60 hover:bg-red-900 border border-red-700 text-xs font-bold rounded-lg font-mono text-red-300"
            >
              Reset
            </button>
          </div>
        </div>

        <!-- Tactical Switches -->
        <div class="w-full pt-3 border-t border-[#334155] flex items-center justify-between text-xs">
          <label class="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" bind:checked={isHantei} class="rounded bg-[#1e293b] border-[#475569] text-amber-500 focus:ring-0" />
            <span class="font-bold text-amber-400">Hantei</span>
          </label>
          <span class="text-[11px] text-zinc-500 font-mono">Hotkeys: 1/2/3 & 7/8/9</span>
        </div>
      </div>

      <!-- 3. RIGHT: AKA (MERAH) POD -->
      <div class="competitor-pod bg-[#0f172a] border-t-4 border-t-red-500 border border-[#334155] rounded-xl p-4 flex flex-col justify-between shadow-xl">
        <div>
          <!-- Pod Header -->
          <div class="flex items-start justify-between pb-3 mb-3 border-b border-[#334155]">
            <div class="min-w-0 pr-2">
              <span class="inline-block px-2 py-0.5 bg-red-600 text-white font-mono font-black text-[10px] rounded tracking-wider uppercase mb-1">
                AKA • MERAH
              </span>
              <div class="text-xl md:text-2xl font-black text-white uppercase leading-tight truncate">
                {match.atlet1_nama || "Atlet Merah"}
              </div>
              <div class="text-xs font-semibold text-zinc-300 mt-1 truncate">
                {match.atlet1_kontingen || "-"}
              </div>
            </div>

            <button
              type="button"
              onclick={() => selectWinner("1")}
              class="px-3 py-1.5 rounded-lg text-xs font-black uppercase tracking-wider transition shrink-0 flex items-center gap-1.5 {matchWinner === '1' ? 'bg-gradient-to-r from-red-600 to-red-700 text-white border-2 border-red-400 shadow-lg shadow-red-600/60' : 'bg-[#1e293b] border border-[#475569] text-zinc-300 hover:text-white'}"
            >
              <svg class="w-4 h-4 {matchWinner === '1' ? 'text-amber-300' : 'text-zinc-400'}" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
              </svg>
              <span>AKA Menang</span>
            </button>
          </div>

          <!-- Score & Senshu Row -->
          <div class="flex items-center gap-3 mb-4">
            <!-- Senshu Ribbon -->
            <button
              type="button"
              onclick={() => toggleSenshu("aka")}
              class="w-24 h-24 rounded-xl flex flex-col items-center justify-center gap-1 font-black transition border-2 {akaSenshu ? 'bg-gradient-to-br from-amber-400 to-amber-600 text-slate-950 border-white shadow-lg shadow-amber-500/50' : 'bg-[#1e293b] text-amber-500 border-amber-600/50 hover:bg-[#334155]'}"
            >
              <svg class="w-7 h-7" fill="currentColor" viewBox="0 0 24 24">
                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
              </svg>
              <span class="text-[11px] tracking-wider font-mono">SENSHU</span>
            </button>

            <div class="flex-1 h-24 bg-[#020617] border-2 border-red-600 rounded-xl flex items-center justify-center shadow-inner">
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
              class="h-12 bg-[#1e293b] hover:bg-[#334155] border border-[#64748b] text-white font-black text-lg rounded-lg active:scale-95 transition flex items-center justify-center"
              title="Kurangi 1 Poin"
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
              class="h-12 bg-gradient-to-br from-red-700 to-red-900 hover:from-red-600 hover:to-red-800 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +2 WAZA
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 3)}
              class="h-12 bg-gradient-to-br from-red-800 to-red-950 hover:from-red-700 hover:to-red-900 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +3 IPPON
            </button>
          </div>

          <!-- Penalties Strip -->
          <div class="bg-[#020617] border border-[#334155] rounded-xl p-2.5 flex items-center justify-between gap-1.5">
            <div class="flex items-center gap-1.5">
              <button
                type="button"
                onclick={() => togglePenalty("aka", "c1")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {akaPenalties.c1 ? 'bg-red-600 text-white border-red-400 shadow-md shadow-red-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                C1
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("aka", "c2")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {akaPenalties.c2 ? 'bg-red-600 text-white border-red-400 shadow-md shadow-red-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                C2
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("aka", "c3")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {akaPenalties.c3 ? 'bg-red-600 text-white border-red-400 shadow-md shadow-red-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                C3
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("aka", "hc")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {akaPenalties.hc ? 'bg-red-600 text-white border-red-400 shadow-md shadow-red-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                HC
              </button>
              <button
                type="button"
                onclick={() => togglePenalty("aka", "h")}
                class="px-2 py-1 rounded text-xs font-mono font-black border {akaPenalties.h ? 'bg-red-600 text-white border-red-400 shadow-md shadow-red-500/50' : 'bg-[#1e293b] text-zinc-400 border-[#475569]'}"
              >
                H
              </button>
            </div>
            <span class="text-[11px] font-black tracking-wider text-zinc-300 uppercase">Pelanggaran</span>
          </div>
        </div>

        <!-- Video Review Indicator -->
        <div class="mt-4 pt-3 border-t border-[#334155] flex items-center justify-between text-xs">
          <span class="font-bold text-zinc-400">Video Review (VR):</span>
          <button
            type="button"
            onclick={() => (hasVr1 = !hasVr1)}
            class="px-2.5 py-1 rounded font-mono font-bold text-xs border {hasVr1 ? 'bg-red-600 text-white border-red-400' : 'bg-[#1e293b] text-zinc-500 border-[#475569]'}"
          >
            {hasVr1 ? "VR AKTIF" : "KARTU HANGUS"}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
