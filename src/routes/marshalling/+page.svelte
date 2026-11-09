<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/stores";
  import { apiCall } from "$lib/api_client";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import type { MarshallingOverview, TatamiQueue, UrgentCall, SubmitUrgentCallPayload } from "$lib/types";

  let overview = $state<MarshallingOverview | null>(null);
  let isLoading = $state(true);
  let currentTime = $state(new Date().toLocaleTimeString("id-ID", { hour: "2-digit", minute: "2-digit", second: "2-digit" }));
  let timerInterval: any = null;
  let refreshInterval: any = null;
  let socket: TatamiSocket | null = null;
  let isLive = $state(false);

  // Active announcement state
  let activeUrgentCall = $state<UrgentCall | null>(null);
  let announcementDismissTimer: any = null;

  // Marshall Call Modal
  let isCallModalOpen = $state(false);
  let modalTatamiNumber = $state(1);
  let modalAthleteName = $state("");
  let modalContingent = $state("");
  let modalCallType = $state("PANGGILAN 1 (MASUK TATAMI)");

  const CALL_TYPE_PRESETS = [
    "PANGGILAN 1 (MASUK KE AREA TATAMI)",
    "PANGGILAN 2 (PERSIAPAN WARM-UP)",
    "PANGGILAN TERAKHIR (BATAS WAKTU 1 MENIT)",
    "PEMERIKSAAN PERLENGKAPAN & TIMBANG BADAN",
    "SEGERA MELAPOR KE MEJA MARSHALL / LO",
  ];

  // Event from the portal link (?event= / ?event_id=); without it, the newest event.
  let eventId = $state<number>(
    parseInt($page.url.searchParams.get("event") || $page.url.searchParams.get("event_id") || "0") || 0
  );

  async function resolveEvent() {
    if (eventId) return;
    try {
      const events = await apiCall<{ id: number }[]>("get_events", {});
      if (events?.length) eventId = Math.max(...events.map((e) => e.id));
    } catch (_) {}
  }

  // Live state arrives several times a second while timers run on every tatami. The queue
  // only changes when a tatami opens another match or a bracket changes, so refresh then
  // (coalesced), not on every message.
  const lastMatchByTatami: Record<number, number | null> = {};
  let refreshTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleRefresh(delay = 800) {
    if (refreshTimer) return;
    refreshTimer = setTimeout(() => {
      refreshTimer = null;
      loadData(false);
    }, delay);
  }

  onMount(async () => {
    // Clock
    timerInterval = setInterval(() => {
      currentTime = new Date().toLocaleTimeString("id-ID", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    }, 1000);

    // Initial Load
    await resolveEvent();
    await loadData();

    // Fallback polling (also covers a link that dropped without notice)
    refreshInterval = setInterval(() => {
      loadData(false);
    }, 15000);

    // WebSocket Connection
    setupWebSocket();
  });

  onDestroy(() => {
    if (timerInterval) clearInterval(timerInterval);
    if (refreshInterval) clearInterval(refreshInterval);
    if (announcementDismissTimer) clearTimeout(announcementDismissTimer);
    if (refreshTimer) clearTimeout(refreshTimer);
    socket?.close();
    socket = null;
  });

  async function loadData(showSpinner = true) {
    if (showSpinner) isLoading = true;
    try {
      overview = await apiCall<MarshallingOverview>("get_marshalling_overview", {
        eventId,
        event_id: eventId,
      });
    } catch (e) {
      console.warn("Failed to load marshalling overview:", e);
    } finally {
      if (showSpinner) isLoading = false;
    }
  }

  function setupWebSocket() {
    socket = connectTatamiSocket(
      "marshalling",
      0,
      (msg) => {
        isLive = true;
        if (!msg || typeof msg !== "object") return;
        const type = msg.type || msg.payload?.type;
        if (type === "urgent_call") {
          triggerUrgentCallAnnouncement(msg.type ? msg : msg.payload);
        } else if (type === "bracket_updated") {
          scheduleRefresh();
        } else if (type === "tatami_live_state") {
          const d = msg.payload || msg.data || {};
          const t = Number(d.tatami);
          const m = d.matchId ? Number(d.matchId) : null;
          if (t && lastMatchByTatami[t] !== m) {
            lastMatchByTatami[t] = m;
            scheduleRefresh();
          }
        }
      },
      () => {
        isLive = true;
        loadData(false); // catch up on what changed while offline
      },
      () => (isLive = false)
    );
  }

  // Web Audio Synth Stadium Chime (C5 -> E5 -> G5 ramp)
  function playStadiumChime() {
    try {
      const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
      if (!AudioCtx) return;
      const ctx = new AudioCtx();

      const notes = [523.25, 659.25, 783.99, 1046.50]; // C5, E5, G5, C6
      notes.forEach((freq, idx) => {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();

        osc.type = "sine";
        osc.frequency.setValueAtTime(freq, ctx.currentTime + idx * 0.22);

        gain.gain.setValueAtTime(0.001, ctx.currentTime + idx * 0.22);
        gain.gain.exponentialRampToValueAtTime(0.28, ctx.currentTime + idx * 0.22 + 0.05);
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + idx * 0.22 + 0.55);

        osc.connect(gain);
        gain.connect(ctx.destination);

        osc.start(ctx.currentTime + idx * 0.22);
        osc.stop(ctx.currentTime + idx * 0.22 + 0.6);
      });
    } catch (e) {
      console.warn("Audio chime error:", e);
    }
  }

  function triggerUrgentCallAnnouncement(call: any) {
    activeUrgentCall = {
      id: String(Date.now()),
      tatami_number: call.tatami_number,
      athlete_name: call.athlete_name,
      contingent: call.contingent,
      message: call.message,
      timestamp: call.timestamp || new Date().toISOString(),
    };

    playStadiumChime();

    if (announcementDismissTimer) clearTimeout(announcementDismissTimer);
    announcementDismissTimer = setTimeout(() => {
      activeUrgentCall = null;
    }, 16000);
  }

  async function handleSendUrgentCall() {
    if (!modalAthleteName.trim()) {
      alert("Nama atlet wajib diisi!");
      return;
    }

    const payload: SubmitUrgentCallPayload = {
      tatami_number: modalTatamiNumber,
      athlete_name: modalAthleteName.trim(),
      contingent: modalContingent.trim() || "-",
      message: modalCallType,
    };

    try {
      await apiCall("broadcast_urgent_call", { input: payload });
      isCallModalOpen = false;
      modalAthleteName = "";
      modalContingent = "";
    } catch (e: any) {
      alert("Gagal memanggil atlet: " + (e?.message || e));
    }
  }

  function toggleFullscreen() {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
  }
</script>

<div class="h-screen w-screen bg-[#0d0e12] text-zinc-100 flex flex-col overflow-hidden font-sans select-none">
  <!-- Top Stadium Header -->
  <header class="px-4 sm:px-6 py-3 sm:py-3.5 bg-[#14161b] border-b border-white/[0.08] flex flex-wrap items-center justify-between gap-3 shrink-0 shadow-lg">
    <div class="flex items-center gap-3 sm:gap-4 min-w-0">
      <div class="hidden sm:flex shrink-0 w-10 h-10 rounded-xl bg-gradient-to-tr from-amber-500 to-red-500 items-center justify-center text-white text-xl shadow-md">
        🥋
      </div>
      <div class="min-w-0">
        <div class="flex items-center gap-2.5 min-w-0">
          <h1 class="text-base sm:text-lg font-black tracking-tight text-white uppercase truncate">
            {overview?.event_name || "KEJUARAAN KARATE RESMI"}
          </h1>
          <span class="hidden md:inline shrink-0 px-2 py-0.5 rounded-full bg-emerald-500/20 border border-emerald-500/40 text-[10px] font-mono font-bold text-emerald-400 animate-pulse">
            LIVE CALL ROOM
          </span>
        </div>
        <p class="hidden sm:block text-xs text-zinc-400 font-mono tracking-wide truncate">
          MONITOR PANGGILAN ATLET & RUANG PEMANASAN (MARSHALLING STATION)
        </p>
      </div>
    </div>

    <!-- Clock & Quick Action Controls -->
    <div class="flex items-center gap-2 sm:gap-3 shrink-0">
      <span
        class="flex items-center gap-1.5 px-2 py-1 rounded-full text-[10px] font-bold font-mono border {isLive ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/25' : 'bg-amber-500/10 text-amber-300 border-amber-500/25'}"
      >
        <span class="w-1.5 h-1.5 rounded-full {isLive ? 'bg-emerald-400' : 'bg-amber-300 animate-pulse'}"></span>
        {isLive ? "LIVE" : "MENYAMBUNG"}
      </span>
      <!-- Digital Clock -->
      <div class="hidden sm:block px-4 py-1.5 rounded-xl bg-[#0d0e12] border border-white/[0.1] font-mono text-base font-black text-amber-400 tracking-widest shadow-inner">
        {currentTime}
      </div>

      <!-- Summon Athlete Button -->
      <button
        type="button"
        onclick={() => (isCallModalOpen = true)}
        class="px-3.5 py-2 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-bold transition flex items-center space-x-1.5 shadow-md active:scale-95"
      >
        <span>📢</span>
        <span>Panggil Atlet</span>
      </button>

      <!-- Fullscreen Toggle -->
      <button
        type="button"
        onclick={toggleFullscreen}
        class="p-2 rounded-xl bg-white/[0.05] hover:bg-white/[0.1] text-zinc-300 text-xs font-bold transition border border-white/[0.08]"
        title="Fullscreen"
      >
        ⛶
      </button>
    </div>
  </header>

  <!-- Urgent Announcement Callout Banner -->
  {#if activeUrgentCall}
    <div class="bg-gradient-to-r from-red-600 via-amber-600 to-red-600 p-3.5 shadow-2xl flex items-center justify-between text-white animate-in slide-in-from-top duration-200 shrink-0">
      <div class="flex items-center space-x-3 max-w-5xl mx-auto">
        <span class="text-2xl animate-bounce">📢</span>
        <div>
          <div class="text-[11px] font-black font-mono tracking-widest uppercase text-amber-200">
            PANGGILAN KHUSUS • TATAMI {activeUrgentCall.tatami_number}
          </div>
          <div class="text-base sm:text-lg font-black tracking-tight leading-tight">
            <span>{activeUrgentCall.athlete_name}</span>
            <span class="text-amber-200 font-normal text-sm">({activeUrgentCall.contingent})</span>
            <span class="mx-2">—</span>
            <span class="underline decoration-white underline-offset-4">{activeUrgentCall.message}</span>
          </div>
        </div>
      </div>
      <button
        type="button"
        onclick={() => (activeUrgentCall = null)}
        class="px-3 py-1 rounded-lg bg-black/30 hover:bg-black/50 text-white font-mono text-xs font-bold"
      >
        TUTUP ✕
      </button>
    </div>
  {/if}

  <!-- Multi-Tatami Live Board Columns -->
  <main class="flex-1 overflow-x-auto overflow-y-auto p-4 md:p-6">
    {#if isLoading && !overview}
      <div class="h-full flex items-center justify-center text-zinc-500 font-mono text-sm">
        Memuat status antrean Tatami...
      </div>
    {:else if !overview || overview.tatamis.length === 0}
      <div class="h-full flex items-center justify-center text-zinc-500 font-medium text-sm">
        Belum ada Tatami aktif yang terdaftar pada turnamen ini.
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 md:gap-5">
        {#each overview.tatamis as tQueue}
          <div class="rounded-2xl bg-[#14161b] border border-white/[0.08] flex flex-col justify-between overflow-hidden shadow-2xl">
            <!-- Tatami Court Header -->
            <div class="px-5 py-3.5 bg-gradient-to-r from-[#1c1f26] to-[#14161b] border-b border-white/[0.08] flex items-center justify-between">
              <div class="flex items-center space-x-2.5">
                <span class="text-lg">🥋</span>
                <span class="text-base font-black text-white uppercase tracking-wider">
                  TATAMI {tQueue.tatami_number}
                </span>
              </div>

              {#if tQueue.current_match}
                <span class="px-2.5 py-0.5 rounded-full bg-emerald-500/20 border border-emerald-500/40 text-[10px] font-mono font-bold text-emerald-400 flex items-center space-x-1">
                  <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                  <span>LIVE</span>
                </span>
              {:else}
                <span class="px-2 py-0.5 rounded text-[10px] font-mono bg-white/[0.05] text-zinc-500">
                  STANDBY
                </span>
              {/if}
            </div>

            <!-- Body Stack: Current Match -> On-Deck -> In-Hole -->
            <div class="p-4 space-y-3.5 flex-1 overflow-y-auto">
              <!-- 1. LIVE / CURRENT MATCH -->
              <div class="rounded-xl border p-3.5 transition {tQueue.current_match ? 'bg-emerald-950/20 border-emerald-500/40 shadow-md' : 'bg-white/[0.02] border-white/[0.06]'}">
                <div class="flex items-center justify-between text-[10px] font-mono font-bold uppercase tracking-wider mb-2">
                  <span class="text-emerald-400 flex items-center space-x-1">
                    <span>● SEDANG BERTANDING (ON MAT)</span>
                  </span>
                  {#if tQueue.current_match?.kode}
                    <span class="text-zinc-400">{tQueue.current_match.kode}</span>
                  {/if}
                </div>

                {#if tQueue.current_match}
                  <div class="text-xs font-bold text-white mb-2 truncate">
                    {tQueue.current_match.category_name}
                  </div>

                  <!-- Combatants Capsule -->
                  <div class="space-y-1.5 text-xs">
                    <!-- AKA -->
                    <div class="p-2 rounded-lg bg-red-950/40 border border-red-500/30 flex items-center justify-between">
                      <div class="min-w-0 flex-1">
                        <div class="font-bold text-red-200 truncate">{tQueue.current_match.atlet1_nama}</div>
                        <div class="text-[10px] text-red-300/80 font-mono truncate">{tQueue.current_match.atlet1_kontingen}</div>
                      </div>
                      <span class="px-1.5 py-0.5 rounded bg-red-600 text-white font-mono text-[9px] font-black">AKA</span>
                    </div>

                    <!-- AO -->
                    <div class="p-2 rounded-lg bg-blue-950/40 border border-blue-500/30 flex items-center justify-between">
                      <div class="min-w-0 flex-1">
                        <div class="font-bold text-blue-200 truncate">{tQueue.current_match.atlet2_nama}</div>
                        <div class="text-[10px] text-blue-300/80 font-mono truncate">{tQueue.current_match.atlet2_kontingen}</div>
                      </div>
                      <span class="px-1.5 py-0.5 rounded bg-blue-600 text-white font-mono text-[9px] font-black">AO</span>
                    </div>
                  </div>
                {:else}
                  <div class="py-3 text-center text-xs text-zinc-500">
                    Tidak ada partai yang sedang berlangsung
                  </div>
                {/if}
              </div>

              <!-- 2. ON-DECK / PANGGILAN 1 -->
              <div class="rounded-xl border p-3.5 transition {tQueue.next_match ? 'bg-amber-950/20 border-amber-500/40 shadow-md' : 'bg-white/[0.02] border-white/[0.06]'}">
                <div class="flex items-center justify-between text-[10px] font-mono font-bold uppercase tracking-wider mb-2">
                  <span class="text-amber-400 flex items-center space-x-1">
                    <span>⚡ PANGGILAN 1 • MASUK AREA (ON DECK)</span>
                  </span>
                  {#if tQueue.next_match?.kode}
                    <span class="text-zinc-400">{tQueue.next_match.kode}</span>
                  {/if}
                </div>

                {#if tQueue.next_match}
                  <div class="text-xs font-bold text-white mb-2 truncate">
                    {tQueue.next_match.category_name}
                  </div>

                  <!-- Combatants Capsule -->
                  <div class="space-y-1.5 text-xs">
                    <div class="p-1.5 rounded-lg bg-[#181a20] border border-white/[0.06] flex items-center justify-between">
                      <div class="min-w-0 flex-1">
                        <div class="font-bold text-zinc-200 truncate">{tQueue.next_match.atlet1_nama}</div>
                        <div class="text-[10px] text-zinc-400 font-mono truncate">{tQueue.next_match.atlet1_kontingen}</div>
                      </div>
                      <span class="px-1 py-0.2 rounded bg-red-600/30 text-red-300 border border-red-500/40 font-mono text-[9px] font-bold">AKA</span>
                    </div>

                    <div class="p-1.5 rounded-lg bg-[#181a20] border border-white/[0.06] flex items-center justify-between">
                      <div class="min-w-0 flex-1">
                        <div class="font-bold text-zinc-200 truncate">{tQueue.next_match.atlet2_nama}</div>
                        <div class="text-[10px] text-zinc-400 font-mono truncate">{tQueue.next_match.atlet2_kontingen}</div>
                      </div>
                      <span class="px-1 py-0.2 rounded bg-blue-600/30 text-blue-300 border border-blue-500/40 font-mono text-[9px] font-bold">AO</span>
                    </div>
                  </div>
                {:else}
                  <div class="py-2 text-center text-xs text-zinc-500">
                    Belum ada partai berikutnya
                  </div>
                {/if}
              </div>

              <!-- 3. IN-HOLE / PANGGILAN 2 & PREPARATION QUEUE -->
              <div>
                <div class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400 mb-2 flex items-center justify-between">
                  <span>ANTREAN PARTAI BERIKUTNYA</span>
                  <span>({tQueue.upcoming_matches.length})</span>
                </div>

                {#if tQueue.upcoming_matches.length === 0}
                  <div class="py-2 text-center text-[11px] text-zinc-600 italic">
                    Antrean kosong
                  </div>
                {:else}
                  <div class="space-y-1.5 text-[11px]">
                    {#each tQueue.upcoming_matches as bout, bIdx}
                      <div class="p-2 rounded-lg bg-[#181a20] border border-white/[0.05] flex items-center justify-between">
                        <div class="min-w-0 flex-1 pr-2">
                          <div class="font-semibold text-zinc-300 truncate">
                            {bout.atlet1_nama} <span class="text-zinc-500 font-normal">vs</span> {bout.atlet2_nama}
                          </div>
                          <div class="text-[10px] text-zinc-500 truncate mt-0.5">
                            {bout.category_name}
                          </div>
                        </div>

                        <span class="px-1.5 py-0.5 rounded font-mono text-[9px] font-bold {bIdx === 0 ? 'bg-blue-500/20 text-blue-300 border border-blue-500/30' : 'bg-white/[0.05] text-zinc-400'}">
                          {bIdx === 0 ? 'CALL 2' : `#${bIdx + 2}`}
                        </span>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            </div>

            <!-- Footer / Quick Tatami Summon Trigger -->
            <div class="p-3 bg-[#111214] border-t border-white/[0.06] text-center">
              <button
                type="button"
                onclick={() => {
                  modalTatamiNumber = tQueue.tatami_number;
                  if (tQueue.next_match) {
                    modalAthleteName = `${tQueue.next_match.atlet1_nama} & ${tQueue.next_match.atlet2_nama}`;
                    modalContingent = `${tQueue.next_match.atlet1_kontingen} / ${tQueue.next_match.atlet2_kontingen}`;
                  }
                  isCallModalOpen = true;
                }}
                class="w-full py-1.5 px-3 rounded-xl bg-white/[0.05] hover:bg-white/[0.1] text-zinc-300 hover:text-white text-xs font-bold transition flex items-center justify-center space-x-1.5"
              >
                <span>📢 Panggil Atlet Tatami {tQueue.tatami_number}</span>
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </main>

  <!-- MARSHALL CALL MODAL -->
  {#if isCallModalOpen}
    <div
      role="dialog"
      aria-modal="true"
      class="fixed inset-0 z-50 bg-black/80 backdrop-blur-xs flex items-center justify-center p-4 animate-in fade-in duration-100"
    >
      <div class="bg-[#181a20] border border-[#2b2d31] rounded-2xl w-full max-w-md shadow-2xl overflow-hidden flex flex-col">
        <!-- Header -->
        <div class="px-5 py-4 border-b border-[#2b2d31] flex items-center justify-between">
          <div class="flex items-center space-x-2">
            <span class="text-xl">📢</span>
            <h3 class="text-sm font-black text-white uppercase tracking-tight">Panggil Atlet / Pengumuman</h3>
          </div>
          <button
            type="button"
            onclick={() => (isCallModalOpen = false)}
            class="w-7 h-7 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-400 hover:text-white flex items-center justify-center text-sm font-bold"
          >
            ✕
          </button>
        </div>

        <!-- Form -->
        <div class="p-5 space-y-4 text-xs">
          <div>
            <label for="call-tatami" class="block font-bold text-zinc-300 mb-1">Target Tatami</label>
            <select
              id="call-tatami"
              bind:value={modalTatamiNumber}
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 focus:outline-none focus:border-red-500"
            >
              {#each (overview?.tatamis || [{ tatami_number: 1 }, { tatami_number: 2 }, { tatami_number: 3 }, { tatami_number: 4 }]) as t}
                <option value={t.tatami_number}>Tatami {t.tatami_number}</option>
              {/each}
            </select>
          </div>

          <div>
            <label for="call-athlete" class="block font-bold text-zinc-300 mb-1">Nama Atlet / Partai *</label>
            <input
              id="call-athlete"
              type="text"
              bind:value={modalAthleteName}
              placeholder="Contoh: Ahmad Fauzi / Partai #12"
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-red-500"
            />
          </div>

          <div>
            <label for="call-contingent" class="block font-bold text-zinc-300 mb-1">Kontingen / Daerah (Opsional)</label>
            <input
              id="call-contingent"
              type="text"
              bind:value={modalContingent}
              placeholder="Contoh: Kontingen Jawa Timur / INKANAS"
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-red-500"
            />
          </div>

          <div>
            <label for="call-preset" class="block font-bold text-zinc-300 mb-1">Jenis Panggilan / Pesan</label>
            <select
              id="call-preset"
              bind:value={modalCallType}
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 focus:outline-none focus:border-red-500"
            >
              {#each CALL_TYPE_PRESETS as preset}
                <option value={preset}>{preset}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Footer -->
        <div class="px-5 py-4 border-t border-[#2b2d31] flex items-center justify-end space-x-2 bg-[#14161b]">
          <button
            type="button"
            onclick={() => (isCallModalOpen = false)}
            class="px-4 py-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-xs font-bold transition"
          >
            Batal
          </button>
          <button
            type="button"
            onclick={handleSendUrgentCall}
            class="px-5 py-2 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-bold transition shadow-md active:scale-95 flex items-center space-x-1"
          >
            <span>📢 Siarkan Panggilan</span>
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
