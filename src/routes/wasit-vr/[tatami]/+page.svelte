<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";

  let tatamiNumber = $derived(Number(page.params.tatami || "1"));

  // Connection state
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
  let hasVr1Burned = $state(false);

  let aoName = $state("SUDUT BIRU (AO)");
  let aoClub = $state("—");
  let aoScore = $state(0);
  let hasVr2Burned = $state(false);

  // Active Challenge State
  interface ActiveChallenge {
    side: "aka" | "ao";
    reason: string;
    timestamp: number;
    status: "in_progress" | "accepted" | "rejected";
  }
  let activeChallenge = $state<ActiveChallenge | null>(null);
  let isSubmittingDecision = $state(false);
  let elapsedReviewSec = $state(0);
  let reviewTimerInterval: any = null;

  // Manual Trigger Modal State
  let showManualModal = $state(false);
  let manualSide = $state<"aka" | "ao">("aka");
  let manualReason = $state("IPPON (3 Poin)");

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
      elapsedReviewSec = 0;
      if (reviewTimerInterval) clearInterval(reviewTimerInterval);
      reviewTimerInterval = setInterval(() => {
        elapsedReviewSec += 1;
      }, 1000);
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
        if (reviewTimerInterval) {
          clearInterval(reviewTimerInterval);
          reviewTimerInterval = null;
        }
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

  async function resolveChallenge(decision: "accepted" | "rejected" | "cancelled") {
    if (!activeChallenge) return;
    isSubmittingDecision = true;

    const payload = {
      tatami: tatamiNumber,
      side: activeChallenge.side,
      decision,
    };

    try {
      // 1. Send via REST endpoint
      await fetch("/api/vr/resolve", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      // 2. Also send over WebSocket if open
      if (wsClient && wsClient.readyState === WebSocket.OPEN) {
        wsClient.send(
          JSON.stringify({
            type: "vr_challenge_resolved",
            tatami: tatamiNumber,
            side: activeChallenge.side,
            decision,
          })
        );
      }

      // 3. BroadcastChannel fallback
      if (broadcastChannel) {
        broadcastChannel.postMessage({
          type: "vr_challenge_resolved",
          tatami: tatamiNumber,
          side: activeChallenge.side,
          decision,
        });
      }

      if (decision === "rejected") {
        if (activeChallenge.side === "aka") hasVr1Burned = true;
        if (activeChallenge.side === "ao") hasVr2Burned = true;
      }

      activeChallenge.status = decision === "accepted" ? "accepted" : "rejected";
      if (reviewTimerInterval) {
        clearInterval(reviewTimerInterval);
        reviewTimerInterval = null;
      }

      setTimeout(() => {
        activeChallenge = null;
      }, 3500);

      if (typeof navigator !== "undefined" && navigator.vibrate) {
        navigator.vibrate(decision === "accepted" ? [100, 50, 100] : [200]);
      }
    } catch (err) {
      console.error("Gagal mengirim keputusan VR:", err);
    } finally {
      isSubmittingDecision = false;
    }
  }

  async function startManualChallenge() {
    const payload = {
      tatami: tatamiNumber,
      side: manualSide,
      reason: manualReason,
    };

    try {
      await fetch("/api/vr/challenge", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (wsClient && wsClient.readyState === WebSocket.OPEN) {
        wsClient.send(
          JSON.stringify({
            type: "vr_challenge_started",
            tatami: tatamiNumber,
            side: manualSide,
            reason: manualReason,
          })
        );
      }
      activeChallenge = {
        side: manualSide,
        reason: manualReason,
        timestamp: Date.now(),
        status: "in_progress",
      };
      showManualModal = false;
    } catch (err) {
      console.error(err);
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
    if (reviewTimerInterval) clearInterval(reviewTimerInterval);
    if (wsClient) wsClient.close();
    if (broadcastChannel) broadcastChannel.close();
  });
</script>

<div class="min-h-screen bg-[#0a0c10] text-zinc-100 flex flex-col font-sans select-none overflow-x-hidden">
  <!-- Top Cockpit Header -->
  <header class="bg-[#12141a] border-b border-white/10 px-6 py-4 flex items-center justify-between shadow-xl">
    <div class="flex items-center space-x-3">
      <div class="w-10 h-10 rounded-xl bg-amber-500/20 border border-amber-500/40 text-amber-400 font-mono font-black flex items-center justify-center text-lg shadow-inner">
        T{tatamiNumber}
      </div>
      <div>
        <div class="text-[10px] uppercase font-mono font-bold text-amber-500 tracking-wider">
          Chief Referee • Video Review Table
        </div>
        <h1 class="text-sm font-black text-white uppercase tracking-tight">
          TATAMI {tatamiNumber} VIDEO REVIEW CONSOLE
        </h1>
      </div>
    </div>

    <!-- Right Controls -->
    <div class="flex items-center gap-3">
      <!-- Status Badge -->
      <div class="flex items-center gap-2 px-3 py-1 rounded-full text-xs font-mono {isWsConnected ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/30' : 'bg-rose-500/15 text-rose-400 border border-rose-500/30'}">
        <span class="w-2 h-2 rounded-full {isWsConnected ? 'bg-emerald-400 animate-pulse' : 'bg-rose-500'}"></span>
        <span>{isWsConnected ? 'LAN ONLINE' : 'OFFLINE'}</span>
      </div>

      <!-- Manual Trigger Button -->
      {#if !activeChallenge}
        <button
          type="button"
          onclick={() => (showManualModal = true)}
          class="px-3 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-zinc-300 border border-white/10 text-xs font-bold transition flex items-center gap-1.5 active:scale-95"
        >
          <span>+ Uji Manual VR</span>
        </button>
      {/if}
    </div>
  </header>

  <!-- Main Display Screen -->
  <main class="flex-1 p-6 max-w-4xl mx-auto w-full flex flex-col justify-center space-y-6">
    {#if activeChallenge}
      <!-- ACTIVE CHALLENGE IN PROGRESS SCREEN -->
      <div class="rounded-3xl border-2 p-8 shadow-2xl space-y-6 transition-all {activeChallenge.status === 'accepted' ? 'bg-emerald-950/40 border-emerald-500/80 shadow-emerald-950/50' : activeChallenge.status === 'rejected' ? 'bg-rose-950/40 border-rose-500/80 shadow-rose-950/50' : (activeChallenge.side === 'aka' ? 'bg-red-950/40 border-red-500 shadow-red-950/50' : 'bg-blue-950/40 border-blue-500 shadow-blue-950/50')}">
        
        <!-- Challenge Header Banner -->
        <div class="flex items-center justify-between pb-4 border-b border-white/10">
          <div class="flex items-center gap-3">
            <span class="w-4 h-4 rounded-full {activeChallenge.side === 'aka' ? 'bg-red-500 animate-ping' : 'bg-blue-500 animate-ping'}"></span>
            <span class="text-sm font-mono font-black uppercase tracking-wider text-white">
              PERMOHONAN VIDEO REVIEW AKTIF
            </span>
          </div>

          <!-- Review Stopwatch -->
          <div class="px-3 py-1 rounded-xl bg-black/60 border border-white/10 font-mono text-xs font-bold text-amber-400 flex items-center gap-1.5">
            <span>Durasi Tinjauan:</span>
            <span class="text-sm font-black text-white">{String(Math.floor(elapsedReviewSec / 60)).padStart(2, '0')}:{String(elapsedReviewSec % 60).padStart(2, '0')}</span>
          </div>
        </div>

        <!-- Big Challenged Party Banner -->
        <div class="text-center py-4 space-y-2">
          <div class="inline-block px-4 py-1.5 rounded-full font-mono font-black text-xs uppercase tracking-widest {activeChallenge.side === 'aka' ? 'bg-red-500 text-white shadow-lg shadow-red-500/30' : 'bg-blue-500 text-white shadow-lg shadow-blue-500/30'}">
            CHALLENGE DARI PELATIH {activeChallenge.side.toUpperCase()} ({activeChallenge.side === 'aka' ? 'SUDUT MERAH' : 'SUDUT BIRU'})
          </div>

          <h2 class="text-3xl font-black text-white tracking-tight uppercase">
            {activeChallenge.side === 'aka' ? akaName : aoName}
          </h2>
          <p class="text-xs text-zinc-400">
            {activeChallenge.side === 'aka' ? akaClub : aoClub}
          </p>
        </div>

        <!-- Reason Card -->
        <div class="bg-black/50 border border-white/15 rounded-2xl p-5 text-center space-y-1">
          <span class="text-[11px] font-mono font-bold text-amber-400 uppercase tracking-widest block">
            ALASAN / POIN YANG DIMINTA:
          </span>
          <div class="text-xl font-black text-white tracking-wide">
            {activeChallenge.reason}
          </div>
        </div>

        <!-- Decision Status or Decision Buttons -->
        {#if activeChallenge.status === 'in_progress'}
          <div class="space-y-4 pt-2">
            <div class="text-center text-xs font-mono text-zinc-400 uppercase tracking-wider">
              Silakan periksa tayangan ulang pada monitor video dan tentukan keputusan:
            </div>

            <!-- Big Action Buttons -->
            <div class="grid grid-cols-2 gap-4">
              <!-- ACCEPT (TERIMA) -->
              <button
                type="button"
                disabled={isSubmittingDecision}
                onclick={() => resolveChallenge("accepted")}
                class="py-5 px-6 rounded-2xl bg-emerald-600 hover:bg-emerald-500 text-white font-black text-base uppercase tracking-wider transition shadow-xl shadow-emerald-950/60 active:scale-98 flex flex-col items-center justify-center gap-1 group"
              >
                <div class="flex items-center gap-2">
                  <svg class="w-6 h-6 fill-current" viewBox="0 0 24 24">
                    <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
                  </svg>
                  <span>TERIMA CHALLENGE</span>
                </div>
                <span class="text-[10px] font-normal text-emerald-200 opacity-90 group-hover:opacity-100">
                  Poin/pelanggaran disahkan • Pelatih tetap simpan kartu
                </span>
              </button>

              <!-- REJECT (TOLAK) -->
              <button
                type="button"
                disabled={isSubmittingDecision}
                onclick={() => resolveChallenge("rejected")}
                class="py-5 px-6 rounded-2xl bg-rose-600 hover:bg-rose-500 text-white font-black text-base uppercase tracking-wider transition shadow-xl shadow-rose-950/60 active:scale-98 flex flex-col items-center justify-center gap-1 group"
              >
                <div class="flex items-center gap-2">
                  <svg class="w-6 h-6 fill-current" viewBox="0 0 24 24">
                    <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" />
                  </svg>
                  <span>TOLAK CHALLENGE</span>
                </div>
                <span class="text-[10px] font-normal text-rose-200 opacity-90 group-hover:opacity-100">
                  Keputusan wasit tatami tetap • Kartu pelatih HANGUS
                </span>
              </button>
            </div>

            <!-- Cancel Button -->
            <div class="text-center pt-2">
              <button
                type="button"
                disabled={isSubmittingDecision}
                onclick={() => resolveChallenge("cancelled")}
                class="text-xs text-zinc-500 hover:text-zinc-300 font-mono underline"
              >
                Batalkan Permohonan (Salah Tekan / Dismiss)
              </button>
            </div>
          </div>
        {:else}
          <!-- Resolution Banner -->
          <div class="text-center py-6 space-y-2">
            {#if activeChallenge.status === 'accepted'}
              <div class="text-3xl font-black text-emerald-400 uppercase tracking-tight">
                ✓ CHALLENGE DITERIMA
              </div>
              <p class="text-xs text-emerald-200">
                Poin / koreksi berhasil dikonfirmasi. Pelatih mempertahankan kartu review.
              </p>
            {:else}
              <div class="text-3xl font-black text-rose-400 uppercase tracking-tight">
                ✕ CHALLENGE DITOLAK
              </div>
              <p class="text-xs text-rose-200">
                Keputusan wasit tatami dipertahankan. Kartu review pelatih hangus.
              </p>
            {/if}
          </div>
        {/if}
      </div>
    {:else}
      <!-- STANDBY DASHBOARD SCREEN -->
      <div class="rounded-3xl border border-white/10 bg-[#12141a] p-8 shadow-2xl text-center space-y-8">
        <div class="space-y-2">
          <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs font-mono font-bold tracking-wider uppercase">
            <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
            <span>SISTEM VR SIAP &amp; TERHUBUNG</span>
          </div>
          <h2 class="text-2xl font-black text-white uppercase tracking-tight">
            STANDBY • MENUNGGU PERMOHONAN PELATIH
          </h2>
          <p class="text-xs text-zinc-400">
            Layar ini akan otomatis menampilkan permohonan challenge begitu pelatih menekan tombol pada Pengawas.
          </p>
        </div>

        <!-- Current Match Snapshot -->
        <div class="bg-black/40 border border-white/10 rounded-2xl p-6 grid grid-cols-3 items-center gap-4">
          <!-- Aka -->
          <div class="text-left space-y-1">
            <div class="flex items-center gap-2">
              <span class="w-3 h-3 rounded-full bg-red-500"></span>
              <span class="text-xs font-black text-red-400 uppercase tracking-wider">AKA (MERAH)</span>
            </div>
            <div class="text-sm font-bold text-white truncate">{akaName}</div>
            <div class="text-[10px] text-zinc-400 truncate">{akaClub}</div>
            <div class="text-xs font-mono font-bold {hasVr1Burned ? 'text-zinc-500' : 'text-emerald-400'}">
              VR: {hasVr1Burned ? 'HANGUS' : 'TERSEDIA'}
            </div>
          </div>

          <!-- Center Timer & Score -->
          <div class="text-center space-y-2">
            <div class="text-[10px] font-mono text-zinc-400 uppercase">{matchRound}</div>
            <div class="text-3xl font-black font-mono text-white tracking-wider">
              <span class="text-red-400">{akaScore}</span>
              <span class="text-zinc-600 mx-2">-</span>
              <span class="text-blue-400">{aoScore}</span>
            </div>
            <div class="text-xs font-mono text-amber-400 font-bold">{timerText}</div>
          </div>

          <!-- Ao -->
          <div class="text-right space-y-1">
            <div class="flex items-center justify-end gap-2">
              <span class="text-xs font-black text-blue-400 uppercase tracking-wider">AO (BIRU)</span>
              <span class="w-3 h-3 rounded-full bg-blue-500"></span>
            </div>
            <div class="text-sm font-bold text-white truncate">{aoName}</div>
            <div class="text-[10px] text-zinc-400 truncate">{aoClub}</div>
            <div class="text-xs font-mono font-bold {hasVr2Burned ? 'text-zinc-500' : 'text-emerald-400'}">
              VR: {hasVr2Burned ? 'HANGUS' : 'TERSEDIA'}
            </div>
          </div>
        </div>
      </div>
    {/if}
  </main>

  <!-- Manual Challenge Trigger Modal -->
  {#if showManualModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
      <div class="bg-[#16181e] border border-white/10 rounded-2xl w-full max-w-md p-6 shadow-2xl space-y-4 text-zinc-200">
        <h3 class="text-sm font-black text-white uppercase tracking-wider font-mono">
          UJI MANUAL SIMULASI CHALLENGE VR
        </h3>
        <p class="text-xs text-zinc-400">
          Gunakan untuk uji coba simulasi aliran tantangan dari meja wasit:
        </p>

        <div class="space-y-3">
          <div>
            <span class="block text-[11px] font-mono font-bold text-zinc-400 uppercase mb-1">Sudut Pelatih:</span>
            <div class="grid grid-cols-2 gap-2">
              <button
                type="button"
                onclick={() => (manualSide = "aka")}
                class="py-2.5 rounded-xl border text-xs font-bold transition {manualSide === 'aka' ? 'bg-red-500/20 border-red-500 text-white' : 'bg-white/[0.02] border-white/10 text-zinc-400'}"
              >
                AKA (MERAH)
              </button>
              <button
                type="button"
                onclick={() => (manualSide = "ao")}
                class="py-2.5 rounded-xl border text-xs font-bold transition {manualSide === 'ao' ? 'bg-blue-500/20 border-blue-500 text-white' : 'bg-white/[0.02] border-white/10 text-zinc-400'}"
              >
                AO (BIRU)
              </button>
            </div>
          </div>

          <div>
            <label for="manual-vr-reason" class="block text-[11px] font-mono font-bold text-zinc-400 uppercase mb-1">Alasan Tinjauan:</label>
            <input
              id="manual-vr-reason"
              type="text"
              bind:value={manualReason}
              class="w-full bg-[#101216] border border-white/10 rounded-xl px-3.5 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-amber-400"
            />
          </div>
        </div>

        <div class="flex items-center gap-2 pt-2">
          <button
            type="button"
            onclick={() => (showManualModal = false)}
            class="flex-1 py-2.5 rounded-xl border border-white/10 bg-white/[0.04] text-xs font-bold text-zinc-300"
          >
            Batal
          </button>
          <button
            type="button"
            onclick={startManualChallenge}
            class="flex-1 py-2.5 rounded-xl bg-amber-500 hover:bg-amber-400 text-black font-black text-xs uppercase"
          >
            Mulai Challenge
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
