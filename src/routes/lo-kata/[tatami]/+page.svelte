<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { KATA_ENTRIES, type KataItem } from "$lib/kata_data";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { getHostBaseUrl } from "$lib/api_client";

  // Tatami number from route params; the event comes from the portal / tatami picker
  let tatamiNumber = $derived(Number(page.params.tatami || "1"));
  let eventId = $derived(Number(page.url.searchParams.get("event")) || null);
  let eventQuery = $derived(eventId ? `?event=${eventId}` : "");

  // Connection state (the shared socket reconnects by itself)
  let isWsConnected = $state(false);
  let socket: TatamiSocket | null = null;

  // Tatami matches list
  let tatamiMatches = $state<any[]>([]);
  let selectedMatchId = $state<number | null>(null);
  let isLoadingMatches = $state(false);

  // Active match data
  let activeMatch = $derived.by(() => {
    if (!selectedMatchId) return tatamiMatches[0] || null;
    return tatamiMatches.find((m) => m.id === selectedMatchId) || tatamiMatches[0] || null;
  });

  // Kata selection state
  let akaKata = $state<string>("");
  let aoKata = $state<string>("");

  // Modal Kata Picker State
  let isModalOpen = $state(false);
  let modalTarget = $state<"aka" | "ao">("aka");
  let searchQuery = $state("");

  let filteredKataList = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return KATA_ENTRIES;
    return KATA_ENTRIES.filter(
      (k) => k.name.toLowerCase().includes(q) || k.code.toLowerCase().includes(q)
    );
  });

  // Repeat Kata Detection Engine
  let previousAkaKatas = $derived.by(() => {
    if (!activeMatch || !activeMatch.atlet1_id) return [];
    return tatamiMatches
      .filter((m) => m.id !== activeMatch.id)
      .filter((m) => m.atlet1_id === activeMatch.atlet1_id || m.atlet2_id === activeMatch.atlet1_id)
      .map((m) => ({
        round: m.round ? `Babak ${m.round}` : "Babak Sebelumnya",
        kata: m.atlet1_id === activeMatch.atlet1_id ? m.kata1 : m.kata2,
      }))
      .filter((item) => !!item.kata);
  });

  let isAkaKataRepeated = $derived.by(() => {
    if (!akaKata) return false;
    return previousAkaKatas.some((item) => item.kata.toLowerCase() === akaKata.toLowerCase());
  });

  let previousAoKatas = $derived.by(() => {
    if (!activeMatch || !activeMatch.atlet2_id) return [];
    return tatamiMatches
      .filter((m) => m.id !== activeMatch.id)
      .filter((m) => m.atlet1_id === activeMatch.atlet2_id || m.atlet2_id === activeMatch.atlet2_id)
      .map((m) => ({
        round: m.round ? `Babak ${m.round}` : "Babak Sebelumnya",
        kata: m.atlet1_id === activeMatch.atlet2_id ? m.kata1 : m.kata2,
      }))
      .filter((item) => !!item.kata);
  });

  let isAoKataRepeated = $derived.by(() => {
    if (!aoKata) return false;
    return previousAoKatas.some((item) => item.kata.toLowerCase() === aoKata.toLowerCase());
  });

  // Action status feedback
  let isSaving = $state(false);
  let toastMessage = $state<string | null>(null);
  let toastTimer: any = null;

  function showToast(msg: string) {
    toastMessage = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage = null;
    }, 2500);
  }

  // Open Kata selection modal
  function openKataModal(target: "aka" | "ao") {
    modalTarget = target;
    searchQuery = "";
    isModalOpen = true;
  }

  function selectKata(kata: KataItem) {
    if (modalTarget === "aka") {
      akaKata = kata.name;
    } else {
      aoKata = kata.name;
    }
    isModalOpen = false;
  }

  // Broadcast & Save Kata
  async function broadcastAndSaveKata() {
    if (!activeMatch) return;
    if (isSaving) return;
    isSaving = true;

    try {
      const payload = {
        match_id: activeMatch.id,
        tatami: tatamiNumber,
        kata1: akaKata || null,
        kata2: aoKata || null,
      };

      // 1. Save on the host (it also tells the control panel)
      const res = await fetch(`${getHostBaseUrl()}/api/match/save-kata`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);

      // 2. Also over the live socket (control panel updates right away)
      socket?.send({
        type: "lo_kata_broadcast",
        tatami: tatamiNumber,
        match_id: activeMatch.id,
        kata_aka: akaKata,
        kata_ao: aoKata,
      });

      showToast("✓ Kata Berhasil Disiarkan ke Scoreboard & Meja!");
      if (typeof navigator !== "undefined" && navigator.vibrate) {
        navigator.vibrate([40, 50, 40]);
      }
    } catch (err) {
      console.error("Gagal menyiarkan kata:", err);
      showToast("Gagal menyimpan kata ke server");
    } finally {
      isSaving = false;
    }
  }

  // Fetch matches assigned to this tatami
  async function fetchTatamiMatches() {
    isLoadingMatches = true;
    try {
      // 1. Try active match
      const base = getHostBaseUrl();
      const currentRes = await fetch(`${base}/api/tatami/${tatamiNumber}/current-match${eventQuery}`);
      if (currentRes.ok) {
        const json = await currentRes.json();
        if (json.status === "success" && json.data) {
          const m = json.data;
          selectedMatchId = m.id;
          akaKata = m.kata1 || "";
          aoKata = m.kata2 || "";
        }
      }

      // 2. Fetch list of all matches on this tatami
      const matchesRes = await fetch(`${base}/api/tatami/${tatamiNumber}/matches${eventQuery}`);
      if (matchesRes.ok) {
        const json = await matchesRes.json();
        if (json.status === "success" && Array.isArray(json.data)) {
          // Only kata bouts: kumite on the same tatami has no kata to choose.
          tatamiMatches = json.data.filter(
            (m: any) => !m.nama_bagan || /KATA/i.test(m.nama_bagan)
          );
        }
      }
    } catch (err) {
      console.warn("Gagal memuat jadwal partai tatami:", err);
    } finally {
      isLoadingMatches = false;
    }
  }

  // Live state from the control panel: follow the match it opens, refresh after reconnect.
  function handleLiveState(d: any) {
    if (!d || Number(d.tatami) !== tatamiNumber) return;
    const mId = d.matchId ? Number(d.matchId) : null;
    if (mId && mId !== selectedMatchId) {
      selectedMatchId = mId;
      fetchTatamiMatches();
    }
  }

  onMount(() => {
    fetchTatamiMatches();
    socket = connectTatamiSocket(
      "lo-kata",
      tatamiNumber,
      (msg) => {
        isWsConnected = true;
        if (msg?.type === "tatami_live_state") handleLiveState(msg.payload || msg.data);
        else if (msg?.type === "bracket_updated") fetchTatamiMatches();
      },
      (sock) => {
        isWsConnected = true;
        // Whatever changed while offline: reload the list and ask the control panel.
        fetchTatamiMatches();
        sock.send({ type: "request_tatami_state", tatami: tatamiNumber });
      },
      () => (isWsConnected = false),
      { staleMs: 20000, onStale: (sock) => sock.send({ type: "request_tatami_state", tatami: tatamiNumber }) }
    );

    return () => {
      socket?.close();
      if (toastTimer) clearTimeout(toastTimer);
    };
  });
</script>

<svelte:head>
  <title>LO Kata Matras • Tatami {tatamiNumber}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<!-- Mobile/Tablet Screen Container -->
<div class="fixed inset-0 bg-[#0c0e12] text-zinc-100 flex flex-col justify-between select-none font-sans overflow-hidden touch-manipulation">
  
  <!-- Header Bar -->
  <header class="bg-[#14161b] border-b border-white/10 px-4 py-2.5 flex items-center justify-between shrink-0 shadow-lg relative z-20">
    <div class="flex items-center gap-2">
      <span class="px-2.5 py-1 rounded-lg bg-purple-500/20 text-purple-300 font-mono font-black text-xs tracking-wider border border-purple-500/30">
        LO KATA • TATAMI {tatamiNumber}
      </span>
      <div class="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono {isWsConnected ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-red-500/10 text-red-400 border border-red-500/20'}">
        <span class="w-1.5 h-1.5 rounded-full {isWsConnected ? 'bg-emerald-400 animate-pulse' : 'bg-red-400'}"></span>
        <span>{isWsConnected ? 'ONLINE' : 'OFFLINE'}</span>
      </div>
    </div>

    <!-- Refresh Matches Button -->
    <button
      onclick={fetchTatamiMatches}
      class="p-1.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/10 transition"
      title="Segarkan data tatami"
    >
      <svg class="w-4 h-4 {isLoadingMatches ? 'animate-spin text-purple-400' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
    </button>
  </header>

  <!-- Match Selector Dropdown -->
  <div class="bg-[#111317] border-b border-white/[0.06] px-4 py-2 flex items-center gap-2 shrink-0">
    <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400 shrink-0">Partai:</span>
    <select
      bind:value={selectedMatchId}
      onchange={() => {
        if (activeMatch) {
          akaKata = activeMatch.kata1 || "";
          aoKata = activeMatch.kata2 || "";
        }
      }}
      class="flex-1 bg-[#181a20] text-xs font-semibold text-white px-2.5 py-1.5 rounded-xl border border-white/10 focus:outline-none focus:border-purple-400"
    >
      {#if tatamiMatches.length === 0}
        <option value={null}>Tidak ada partai terdaftar di tatami ini</option>
      {:else}
        {#each tatamiMatches as m}
          <option value={m.id}>
            Partai #{m.urutan || 1} • {m.round ? `Babak ${m.round}` : 'Penyisihan'} — {m.atlet1_nama || 'AKA'} vs {m.atlet2_nama || 'AO'}
          </option>
        {/each}
      {/if}
    </select>
  </div>

  <!-- Toast Notification Overlay -->
  {#if toastMessage}
    <div class="fixed top-14 left-1/2 -translate-x-1/2 z-50 bg-purple-500 text-white px-4 py-2 rounded-xl text-xs font-bold shadow-2xl animate-in slide-in-from-top-2 duration-150 flex items-center gap-2">
      <svg class="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
      </svg>
      <span>{toastMessage}</span>
    </div>
  {/if}

  <!-- Main Body: Dual Cards for AKA & AO Kata -->
  <main class="flex-1 min-h-0 p-3 flex flex-col gap-3 overflow-y-auto">
    
    <!-- ==================== AKA KATA CARD ==================== -->
    <div class="bg-[#16181f] border-2 border-red-600/70 rounded-2xl p-4 shadow-lg flex flex-col justify-between gap-3">
      <div>
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 rounded bg-red-600 text-white font-mono font-black text-[10px] tracking-wider uppercase">AKA</span>
            <h2 class="text-base font-black text-white truncate">{activeMatch?.atlet1_nama || "Sudut Merah"}</h2>
          </div>
        </div>
        <p class="text-xs text-zinc-400 mt-0.5 font-medium">{activeMatch?.atlet1_kontingen || "-"}</p>
      </div>

      <!-- Selected Kata Badge & Change Button -->
      <div class="flex items-center justify-between gap-2 bg-[#101216] border border-white/10 rounded-xl p-2.5">
        <span class="text-sm font-black font-mono {akaKata ? 'text-red-400' : 'text-zinc-500'} truncate">
          {akaKata || "Belum dipilih"}
        </span>
        <button
          onclick={() => openKataModal("aka")}
          class="h-9 px-3.5 bg-red-600 hover:bg-red-500 active:scale-95 text-white font-bold text-xs rounded-xl shadow transition shrink-0"
        >
          Pilih Kata
        </button>
      </div>

      <!-- Repeat Kata Alert Warning -->
      {#if isAkaKataRepeated}
        <div class="px-3 py-2 rounded-xl bg-amber-500/15 border border-amber-500/40 text-amber-300 text-xs font-semibold flex items-center gap-2">
          <span>⚠️</span>
          <span>Kata "{akaKata}" sudah pernah dimainkan di babak sebelumnya</span>
        </div>
      {/if}
    </div>

    <!-- ==================== AO KATA CARD ==================== -->
    <div class="bg-[#16181f] border-2 border-blue-600/70 rounded-2xl p-4 shadow-lg flex flex-col justify-between gap-3">
      <div>
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 rounded bg-blue-600 text-white font-mono font-black text-[10px] tracking-wider uppercase">AO</span>
            <h2 class="text-base font-black text-white truncate">{activeMatch?.atlet2_nama || "Sudut Biru"}</h2>
          </div>
        </div>
        <p class="text-xs text-zinc-400 mt-0.5 font-medium">{activeMatch?.atlet2_kontingen || "-"}</p>
      </div>

      <!-- Selected Kata Badge & Change Button -->
      <div class="flex items-center justify-between gap-2 bg-[#101216] border border-white/10 rounded-xl p-2.5">
        <span class="text-sm font-black font-mono {aoKata ? 'text-blue-400' : 'text-zinc-500'} truncate">
          {aoKata || "Belum dipilih"}
        </span>
        <button
          onclick={() => openKataModal("ao")}
          class="h-9 px-3.5 bg-blue-600 hover:bg-blue-500 active:scale-95 text-white font-bold text-xs rounded-xl shadow transition shrink-0"
        >
          Pilih Kata
        </button>
      </div>

      <!-- Repeat Kata Alert Warning -->
      {#if isAoKataRepeated}
        <div class="px-3 py-2 rounded-xl bg-amber-500/15 border border-amber-500/40 text-amber-300 text-xs font-semibold flex items-center gap-2">
          <span>⚠️</span>
          <span>Kata "{aoKata}" sudah pernah dimainkan di babak sebelumnya</span>
        </div>
      {/if}
    </div>

  </main>

  <!-- Sticky Bottom Broadcast Button -->
  <footer class="bg-[#14161b] border-t border-white/10 p-3 shrink-0 shadow-2xl">
    <button
      onclick={broadcastAndSaveKata}
      disabled={isSaving || !activeMatch}
      class="w-full h-12 bg-purple-600 hover:bg-purple-500 active:scale-[0.98] disabled:opacity-50 text-white font-black text-sm rounded-xl shadow-xl flex items-center justify-center gap-2 transition"
    >
      {#if isSaving}
        <svg class="animate-spin h-5 w-5 text-white" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        <span>MENYIARKAN KATA...</span>
      {:else}
        <svg class="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M11 5.882V19.24a1.76 1.76 0 01-3.417.592l-2.147-6.15M18 13a3 3 0 100-6M5.436 13.683A4.001 4.001 0 017 6h1.832c4.1 0 7.625-1.234 9.168-3v14c-1.543-1.766-5.067-3-9.168-3H7a3.988 3.988 0 01-1.564-.317z" />
        </svg>
        <span>SIARKAN KATA KE PAPAN SKOR & SIMPAN</span>
      {/if}
    </button>
  </footer>

  <!-- Modal: Official WKF Kata Search & Picker -->
  {#if isModalOpen}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-5 shadow-2xl max-w-md w-full flex flex-col max-h-[85vh] animate-in zoom-in-95 duration-150">
        
        <!-- Modal Header -->
        <div class="flex items-center justify-between pb-3 border-b border-white/10">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black uppercase {modalTarget === 'aka' ? 'bg-red-600 text-white' : 'bg-blue-600 text-white'}">
              {modalTarget === 'aka' ? 'SUDUT MERAH (AKA)' : 'SUDUT BIRU (AO)'}
            </span>
            <h3 class="text-sm font-black text-white">Daftar Kata WKF/FORKI</h3>
          </div>
          <button
            onclick={() => isModalOpen = false}
            aria-label="Tutup modal"
            class="p-1 rounded-lg hover:bg-white/10 text-zinc-400 hover:text-white transition"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <!-- Search Bar -->
        <div class="py-3">
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Ketik nama kata (misal: Anan, Chatanyara, Suparinpei...)"
            class="w-full bg-[#111317] border border-white/10 rounded-xl px-3 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-purple-400"
          />
        </div>

        <!-- Kata Scrollable List -->
        <div class="flex-1 min-h-0 overflow-y-auto space-y-1 pr-1">
          {#if filteredKataList.length === 0}
            <div class="p-6 text-center text-zinc-500 text-xs">
              Tidak ada kata yang sesuai dengan "{searchQuery}"
            </div>
          {:else}
            {#each filteredKataList as item}
              <button
                onclick={() => selectKata(item)}
                class="w-full px-3 py-2.5 rounded-xl text-left transition flex items-center justify-between hover:bg-white/[0.06] text-zinc-200"
              >
                <div>
                  <span class="font-bold text-xs text-white block">{item.name}</span>
                  <span class="text-[10px] font-mono text-zinc-400 block">{item.code}</span>
                </div>
                <svg class="w-4 h-4 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                </svg>
              </button>
            {/each}
          {/if}
        </div>

      </div>
    </div>
  {/if}

</div>
