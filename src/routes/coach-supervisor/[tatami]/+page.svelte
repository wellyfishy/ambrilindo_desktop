<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));

  // Connection & sync state
  let isWsConnected = $state(false);
  let wsClient: WebSocket | null = null;
  let reconnectInterval: any = null;
  let broadcastChannel: BroadcastChannel | null = null;

  // Active match data
  let matchCategory = $state("KUMITE PERORANGAN");
  let matchRound = $state("BABAK PENYISIHAN");
  let matchNumber = $state("1");
  let timerText = $state("02:00");
  let isTimerRunning = $state(false);

  // Combatant & Card Status
  let akaName = $state("SUDUT MERAH (AKA)");
  let akaClub = $state("—");
  let akaScore = $state(0);
  let hasVr1Burned = $state(false); // true = hangus, false = tersedia

  let aoName = $state("SUDUT BIRU (AO)");
  let aoClub = $state("—");
  let aoScore = $state(0);
  let hasVr2Burned = $state(false); // true = hangus, false = tersedia

  // Active Challenge State
  interface ActiveChallenge {
    side: "aka" | "ao";
    reason: string;
    timestamp: number;
    status: "in_progress" | "accepted" | "rejected";
  }
  let activeChallenge = $state<ActiveChallenge | null>(null);

  // Reason Selection Modal State
  let showReasonModal = $state(false);
  let selectedSide = $state<"aka" | "ao">("aka");
  let selectedReason = $state("IPPON (3 Poin)");
  let customReasonText = $state("");
  let isSubmitting = $state(false);
  let toastMessage = $state<string | null>(null);
  let toastTimer: any = null;

  const STANDARD_REASONS = [
    { label: "IPPON (3 Poin)", desc: "Tendangan jodan atau teknik bantingan + pukulan" },
    { label: "WAZA-ARI (2 Poin)", desc: "Tendangan chudan ke arah badan lawan" },
    { label: "YUKO (1 Poin)", desc: "Pukulan tsuki chudan atau jodan yang sah" },
    { label: "SENSHU", desc: "Penetapan hak poin pertama tanpa protes" },
    { label: "FOUL / PENALTY", desc: "Kontak berlebih, memegang, atau dorongan (C1/C2)" },
    { label: "JOGAI", desc: "Keluar dari batas area pertandingan (tatami)" },
  ];

  function showToast(msg: string) {
    toastMessage = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage = null;
      toastTimer = null;
    }, 3000);
  }

  function handleLivePayload(data: any) {
    if (!data) return;
    if (data.tatami && data.tatami != tatamiNumber) return;

    if (data.type === "vr_challenge_started") {
      activeChallenge = {
        side: data.side,
        reason: data.reason,
        timestamp: data.timestamp || Date.now(),
        status: "in_progress",
      };
      return;
    }

    if (data.type === "vr_challenge_resolved") {
      if (data.decision === "rejected") {
        if (data.side === "aka") hasVr1Burned = true;
        if (data.side === "ao") hasVr2Burned = true;
      }
      if (activeChallenge) {
        activeChallenge = {
          ...activeChallenge,
          status: data.decision === "accepted" ? "accepted" : "rejected",
        };
        setTimeout(() => {
          activeChallenge = null;
        }, 3500);
      }
      return;
    }

    // Apply tatami_live_state payload
    if (data.category) matchCategory = data.category;
    if (data.round) matchRound = data.round;
    if (data.matchNumber !== undefined) matchNumber = String(data.matchNumber);

    if (data.timerMs !== undefined) {
      const totalSec = Math.max(0, Math.floor(data.timerMs / 1000));
      const m = Math.floor(totalSec / 60);
      const s = totalSec % 60;
      timerText = `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
    }
    if (data.isTimerRunning !== undefined) isTimerRunning = data.isTimerRunning;

    if (data.aka) {
      if (data.aka.name) akaName = data.aka.name;
      if (data.aka.contingent) akaClub = data.aka.contingent;
      if (data.aka.score !== undefined) akaScore = data.aka.score;
      if (data.aka.vr !== undefined) hasVr1Burned = data.aka.vr;
    }

    if (data.ao) {
      if (data.ao.name) aoName = data.ao.name;
      if (data.ao.contingent) aoClub = data.ao.contingent;
      if (data.ao.score !== undefined) aoScore = data.ao.score;
      if (data.ao.vr !== undefined) hasVr2Burned = data.ao.vr;
    }
  }

  function initWebSocket() {
    if (typeof window === "undefined") return;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const host = window.location.host;
    const wsUrl = `${protocol}//${host}/ws/vr/${tatamiNumber}`;

    try {
      wsClient = new WebSocket(wsUrl);

      wsClient.onopen = () => {
        isWsConnected = true;
      };

      wsClient.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data);
          handleLivePayload(parsed);
        } catch {}
      };

      wsClient.onclose = () => {
        isWsConnected = false;
      };

      wsClient.onerror = () => {
        isWsConnected = false;
      };
    } catch {
      isWsConnected = false;
    }
  }

  function openChallengeModal(side: "aka" | "ao") {
    if (side === "aka" && hasVr1Burned) {
      showToast("Kartu VR Pelatih AKA sudah hangus!");
      return;
    }
    if (side === "ao" && hasVr2Burned) {
      showToast("Kartu VR Pelatih AO sudah hangus!");
      return;
    }
    selectedSide = side;
    selectedReason = STANDARD_REASONS[0].label;
    customReasonText = "";
    showReasonModal = true;
  }

  async function submitChallenge() {
    isSubmitting = true;
    const finalReason = customReasonText.trim() ? `${selectedReason} - ${customReasonText.trim()}` : selectedReason;

    const payload = {
      tatami: tatamiNumber,
      side: selectedSide,
      reason: finalReason,
    };

    try {
      // 1. Send via REST endpoint
      await fetch("/api/vr/challenge", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      // 2. Also send over WebSocket if open
      if (wsClient && wsClient.readyState === WebSocket.OPEN) {
        wsClient.send(
          JSON.stringify({
            type: "vr_challenge_started",
            tatami: tatamiNumber,
            side: selectedSide,
            reason: finalReason,
          })
        );
      }

      // 3. BroadcastChannel fallback
      if (broadcastChannel) {
        broadcastChannel.postMessage({
          type: "vr_challenge_started",
          tatami: tatamiNumber,
          side: selectedSide,
          reason: finalReason,
        });
      }

      activeChallenge = {
        side: selectedSide,
        reason: finalReason,
        timestamp: Date.now(),
        status: "in_progress",
      };

      showReasonModal = false;
      showToast(`✓ Challenge ${selectedSide.toUpperCase()} Berhasil Diajukan!`);

      if (typeof navigator !== "undefined" && navigator.vibrate) {
        navigator.vibrate([100, 60, 100]);
      }
    } catch (err) {
      console.error("Gagal mengajukan challenge:", err);
      showToast("Gagal mengirim challenge ke meja wasit.");
    } finally {
      isSubmitting = false;
    }
  }

  onMount(() => {
    initWebSocket();
    reconnectInterval = setInterval(() => {
      if (!wsClient || wsClient.readyState === WebSocket.CLOSED) {
        initWebSocket();
      }
    }, 3000);

    if (typeof BroadcastChannel !== "undefined") {
      broadcastChannel = new BroadcastChannel("ambrilindo_live_sync");
      broadcastChannel.onmessage = (event) => {
        handleLivePayload(event.data);
      };
    }
  });

  onDestroy(() => {
    if (reconnectInterval) clearInterval(reconnectInterval);
    if (wsClient) wsClient.close();
    if (broadcastChannel) broadcastChannel.close();
    if (toastTimer) clearTimeout(toastTimer);
  });
</script>

<div class="min-h-screen bg-[#0e1014] text-zinc-100 flex flex-col font-sans select-none pb-6">
  <!-- Top Navigation & Tatami Badge -->
  <header class="bg-[#14161c] border-b border-white/10 px-4 py-3 flex items-center justify-between sticky top-0 z-30 shadow-md">
    <div class="flex items-center space-x-2.5">
      <div class="w-8 h-8 rounded-lg bg-amber-500/20 border border-amber-500/40 text-amber-400 font-mono font-black flex items-center justify-center text-sm">
        T{tatamiNumber}
      </div>
      <div>
        <div class="text-[10px] uppercase font-bold text-zinc-400 tracking-wider">
          Pengawas Pelatih (VR)
        </div>
        <div class="text-xs font-black text-white uppercase tracking-tight">
          TATAMI {tatamiNumber}
        </div>
      </div>
    </div>

    <!-- Connection Indicator -->
    <div class="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-mono {isWsConnected ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/30' : 'bg-rose-500/15 text-rose-400 border border-rose-500/30'}">
      <span class="w-2 h-2 rounded-full {isWsConnected ? 'bg-emerald-400 animate-pulse' : 'bg-rose-500'}"></span>
      <span>{isWsConnected ? 'ONLINE' : 'OFFLINE'}</span>
    </div>
  </header>

  <!-- Main Content Container -->
  <main class="flex-1 p-4 max-w-lg mx-auto w-full space-y-4">
    <!-- Match Info Banner -->
    <div class="bg-[#14161c] border border-white/10 rounded-2xl p-4 shadow-md space-y-2 text-center">
      <div class="text-[10px] font-mono font-bold text-amber-400 uppercase tracking-wider">
        {matchCategory}
      </div>
      <div class="text-xs font-semibold text-zinc-400">
        {matchRound} • Partai #{matchNumber}
      </div>
      <div class="inline-flex items-center gap-2 px-3 py-1 rounded-lg bg-black/40 border border-white/10 text-white font-mono font-bold text-sm">
        <span class="w-2 h-2 rounded-full {isTimerRunning ? 'bg-emerald-400 animate-ping' : 'bg-amber-400'}"></span>
        <span>{timerText}</span>
      </div>
    </div>

    <!-- Active Challenge In-Progress Banner -->
    {#if activeChallenge}
      <div class="p-4 rounded-2xl border transition-all animate-pulse {activeChallenge.status === 'accepted' ? 'bg-emerald-500/20 border-emerald-500/60 text-emerald-300' : activeChallenge.status === 'rejected' ? 'bg-rose-500/20 border-rose-500/60 text-rose-300' : (activeChallenge.side === 'aka' ? 'bg-red-500/25 border-red-500/80 text-red-200' : 'bg-blue-500/25 border-blue-500/80 text-blue-200')} shadow-xl">
        <div class="flex items-center justify-between text-xs font-mono font-bold uppercase mb-1">
          <span>
            {#if activeChallenge.status === 'accepted'}
              ✓ CHALLENGE DITERIMA WASIT
            {:else if activeChallenge.status === 'rejected'}
              ✕ CHALLENGE DITOLAK WASIT
            {:else}
              ⏳ CHALLENGE DALAM PENINJAUAN
            {/if}
          </span>
          <span class="px-2 py-0.5 rounded text-[10px] {activeChallenge.side === 'aka' ? 'bg-red-600 text-white' : 'bg-blue-600 text-white'}">
            SUDUT {activeChallenge.side.toUpperCase()}
          </span>
        </div>
        <div class="text-sm font-black text-white mt-1">
          {activeChallenge.reason}
        </div>
        <div class="text-[11px] mt-1 opacity-80">
          {#if activeChallenge.status === 'accepted'}
            Pelatih behak mempertahankan kartu VR untuk sisa pertandingan.
          {:else if activeChallenge.status === 'rejected'}
            Kartu VR pelatih hangus untuk sisa pertandingan ini.
          {:else}
            Menunggu keputusan Meja Wasit Utama / Chief Referee...
          {/if}
        </div>
      </div>
    {/if}

    <!-- Coach Challenge Trigger Cards -->
    <div class="space-y-4">
      <!-- AKA COACH (RED) -->
      <div class="bg-[#14161c] border-2 {hasVr1Burned ? 'border-red-500/20 opacity-60' : 'border-red-500/40'} rounded-2xl p-4 shadow-lg space-y-3">
        <div class="flex items-start justify-between">
          <div>
            <div class="flex items-center gap-2">
              <span class="w-3 h-3 rounded-full bg-red-500"></span>
              <span class="text-xs font-black text-red-400 uppercase tracking-wider">
                PELATIH AKA (MERAH)
              </span>
            </div>
            <h3 class="text-sm font-bold text-white mt-1 truncate">
              {akaName}
            </h3>
            <p class="text-[11px] text-zinc-400 truncate">
              {akaClub}
            </p>
          </div>

          <!-- Score & VR Status -->
          <div class="text-right">
            <div class="text-2xl font-black font-mono text-red-400">
              {akaScore}
            </div>
            <span class="inline-block px-2 py-0.5 rounded text-[9px] font-mono font-bold uppercase {hasVr1Burned ? 'bg-zinc-800 text-zinc-500 border border-white/5' : 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40'}">
              {hasVr1Burned ? 'KARTU HANGUS' : 'KARTU AKTIF'}
            </span>
          </div>
        </div>

        <button
          type="button"
          disabled={hasVr1Burned || isSubmitting || (activeChallenge && activeChallenge.status === 'in_progress')}
          onclick={() => openChallengeModal("aka")}
          class="w-full py-3.5 px-4 rounded-xl font-black text-sm uppercase tracking-wider transition shadow-md active:scale-98 flex items-center justify-center gap-2 {hasVr1Burned ? 'bg-zinc-800/80 text-zinc-600 border border-white/5 cursor-not-allowed' : 'bg-red-600 hover:bg-red-500 text-white shadow-red-950/40'}"
        >
          <svg class="w-5 h-5 fill-current" viewBox="0 0 24 24">
            <path d="M4 4h16v12H4zM4 18h16v2H4z" />
          </svg>
          <span>{hasVr1Burned ? 'KARTU VR HANGUS' : 'CHALLENGE PELATIH AKA'}</span>
        </button>
      </div>

      <!-- AO COACH (BLUE) -->
      <div class="bg-[#14161c] border-2 {hasVr2Burned ? 'border-blue-500/20 opacity-60' : 'border-blue-500/40'} rounded-2xl p-4 shadow-lg space-y-3">
        <div class="flex items-start justify-between">
          <div>
            <div class="flex items-center gap-2">
              <span class="w-3 h-3 rounded-full bg-blue-500"></span>
              <span class="text-xs font-black text-blue-400 uppercase tracking-wider">
                PELATIH AO (BIRU)
              </span>
            </div>
            <h3 class="text-sm font-bold text-white mt-1 truncate">
              {aoName}
            </h3>
            <p class="text-[11px] text-zinc-400 truncate">
              {aoClub}
            </p>
          </div>

          <!-- Score & VR Status -->
          <div class="text-right">
            <div class="text-2xl font-black font-mono text-blue-400">
              {aoScore}
            </div>
            <span class="inline-block px-2 py-0.5 rounded text-[9px] font-mono font-bold uppercase {hasVr2Burned ? 'bg-zinc-800 text-zinc-500 border border-white/5' : 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40'}">
              {hasVr2Burned ? 'KARTU HANGUS' : 'KARTU AKTIF'}
            </span>
          </div>
        </div>

        <button
          type="button"
          disabled={hasVr2Burned || isSubmitting || (activeChallenge && activeChallenge.status === 'in_progress')}
          onclick={() => openChallengeModal("ao")}
          class="w-full py-3.5 px-4 rounded-xl font-black text-sm uppercase tracking-wider transition shadow-md active:scale-98 flex items-center justify-center gap-2 {hasVr2Burned ? 'bg-zinc-800/80 text-zinc-600 border border-white/5 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-500 text-white shadow-blue-950/40'}"
        >
          <svg class="w-5 h-5 fill-current" viewBox="0 0 24 24">
            <path d="M4 4h16v12H4zM4 18h16v2H4z" />
          </svg>
          <span>{hasVr2Burned ? 'KARTU VR HANGUS' : 'CHALLENGE PELATIH AO'}</span>
        </button>
      </div>
    </div>
  </main>

  <!-- Reason Selection Modal -->
  {#if showReasonModal}
    <div class="fixed inset-0 z-50 flex items-end sm:items-center justify-center p-3 sm:p-4 bg-black/80 backdrop-blur-sm animate-in fade-in">
      <div class="bg-[#16181e] border border-white/10 rounded-2xl w-full max-w-md p-5 shadow-2xl space-y-4 text-zinc-200">
        <div class="flex items-center justify-between pb-2 border-b border-white/10">
          <div class="flex items-center gap-2">
            <span class="w-3 h-3 rounded-full {selectedSide === 'aka' ? 'bg-red-500' : 'bg-blue-500'}"></span>
            <h3 class="text-sm font-black uppercase text-white tracking-wider">
              ALASAN CHALLENGE {selectedSide.toUpperCase()}
            </h3>
          </div>
          <button
            type="button"
            onclick={() => (showReasonModal = false)}
            class="text-zinc-400 hover:text-white p-1 rounded-lg"
          >
            ✕
          </button>
        </div>

        <p class="text-xs text-zinc-400">
          Pilih alasan permohonan peninjauan video review yang diajukan oleh pelatih:
        </p>

        <!-- Standard Reason List -->
        <div class="space-y-1.5 max-h-60 overflow-y-auto pr-1">
          {#each STANDARD_REASONS as r}
            <button
              type="button"
              onclick={() => (selectedReason = r.label)}
              class="w-full text-left p-3 rounded-xl border transition flex flex-col {selectedReason === r.label ? (selectedSide === 'aka' ? 'bg-red-500/20 border-red-500 text-white font-bold' : 'bg-blue-500/20 border-blue-500 text-white font-bold') : 'bg-white/[0.02] border-white/5 hover:border-white/20 text-zinc-300'}"
            >
              <span class="text-xs font-mono">{r.label}</span>
              <span class="text-[10px] text-zinc-400 font-normal">{r.desc}</span>
            </button>
          {/each}
        </div>

        <!-- Custom Reason / Additional Note -->
        <div>
          <label for="vr-custom-reason" class="block text-[11px] font-bold text-zinc-400 uppercase tracking-wider mb-1">
            Keterangan Tambahan (Opsional)
          </label>
          <input
            id="vr-custom-reason"
            type="text"
            bind:value={customReasonText}
            placeholder="Contoh: Mawashi Geri Jodan detik 00:45..."
            class="w-full bg-[#101216] border border-white/10 rounded-xl px-3 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-amber-400 transition"
          />
        </div>

        <!-- Action Buttons -->
        <div class="flex items-center gap-2 pt-2">
          <button
            type="button"
            onclick={() => (showReasonModal = false)}
            class="flex-1 py-2.5 rounded-xl border border-white/10 bg-white/[0.04] text-xs font-bold text-zinc-300 hover:bg-white/[0.08] transition"
          >
            Batal
          </button>
          <button
            type="button"
            disabled={isSubmitting}
            onclick={submitChallenge}
            class="flex-1 py-2.5 rounded-xl text-xs font-black uppercase tracking-wider text-white transition flex items-center justify-center gap-1.5 {selectedSide === 'aka' ? 'bg-red-600 hover:bg-red-500' : 'bg-blue-600 hover:bg-blue-500'}"
          >
            {#if isSubmitting}
              <span>Mengirim...</span>
            {:else}
              <span>Kirim Challenge</span>
            {/if}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Toast Notification -->
  {#if toastMessage}
    <div class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 bg-[#1e2029] border border-white/20 text-white text-xs font-mono font-bold px-4 py-2.5 rounded-xl shadow-2xl animate-in fade-in">
      {toastMessage}
    </div>
  {/if}
</div>
