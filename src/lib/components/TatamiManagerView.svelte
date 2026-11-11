<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "$lib/api_client";
  import { listen } from "@tauri-apps/api/event";
  import { initLiveSync, subscribeLiveSync } from "$lib/liveSync";
  import type { TatamiItem } from "$lib/types";

  interface Props {
    eventId: number;
    tatamis: TatamiItem[];
    hostAddress: string;
    onRefreshTatamis: () => void;
    onShowToast?: (msg: string) => void;
  }

  let {
    eventId,
    tatamis,
    hostAddress,
    onRefreshTatamis,
    onShowToast,
  }: Props = $props();

  // Filter & Search State
  let searchKeyword = $state("");
  let selectedStatusFilter = $state<"all" | "live" | "ready">("all");
  let selectedSort = $state<"tatami_asc" | "tatami_desc" | "live_first">("tatami_asc");

  // Pagination State
  let currentPage = $state(1);
  let pageSize = $state(50);

  // Modals State
  let isAddingModal = $state(false);
  let newTatamiNumber = $state<number>(1);
  let isSubmitting = $state(false);
  let addError = $state<string | null>(null);

  let editModalTatami = $state<TatamiItem | null>(null);
  let editTatamiNumber = $state<number>(1);
  let isEditing = $state(false);
  let editError = $state<string | null>(null);

  let tatamiToDelete = $state<TatamiItem | null>(null);
  let isDeleting = $state(false);

  // QR Code Modal State
  let qrModalTatami = $state<TatamiItem | null>(null);
  let qrRole = $state<"jury" | "lo-kata" | "operator" | "coach-supervisor" | "wasit-vr">("jury");
  let qrSvgContent = $state<string>("");
  let isLoadingQr = $state(false);
  let availableIps = $state<Array<{ name: string; ip: string; is_default: boolean }>>([]);
  let currentQrHost = $state<string>("");

  // Toast Notification State
  let toastMsg = $state<string | null>(null);
  let toastTimer: any = null;
  let syncInterval: any = null;

  function showToast(msg: string) {
    toastMsg = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMsg = null;
    }, 3500);
    onShowToast?.(msg);
  }

  let effectiveTatamis = $derived(tatamis ?? []);

  // KPI Metrics Calculation
  let totalTatamis = $derived(effectiveTatamis.length);
  let activeMatchesCount = $derived(effectiveTatamis.filter((t) => t.detail_bagan_id !== null).length);
  let readyCount = $derived(Math.max(0, totalTatamis - activeMatchesCount));

  // Filtered and Sorted Tatamis
  let filteredTatamis = $derived.by(() => {
    let result = [...effectiveTatamis];

    // Status Filter
    if (selectedStatusFilter === "live") {
      result = result.filter((t) => t.detail_bagan_id !== null);
    } else if (selectedStatusFilter === "ready") {
      result = result.filter((t) => t.detail_bagan_id === null);
    }

    // Search Keyword
    if (searchKeyword.trim()) {
      const q = searchKeyword.toLowerCase().trim();
      result = result.filter((t) => {
        const tNumStr = (t.tatami_number ?? "").toString();
        const baganStr = t.nama_bagan || (t.detail_bagan_id ? `partai ${t.detail_bagan_id} #${t.detail_bagan_id}` : "");
        return (
          tNumStr.includes(q) ||
          `tatami ${tNumStr}`.includes(q) ||
          baganStr.toLowerCase().includes(q)
        );
      });
    }

    // Sorting
    if (selectedSort === "tatami_asc") {
      result.sort((a, b) => (a.tatami_number ?? 0) - (b.tatami_number ?? 0));
    } else if (selectedSort === "tatami_desc") {
      result.sort((a, b) => (b.tatami_number ?? 0) - (a.tatami_number ?? 0));
    } else if (selectedSort === "live_first") {
      result.sort((a, b) => {
        const aLive = a.detail_bagan_id !== null ? 1 : 0;
        const bLive = b.detail_bagan_id !== null ? 1 : 0;
        if (bLive !== aLive) return bLive - aLive;
        return (a.tatami_number ?? 0) - (b.tatami_number ?? 0);
      });
    }

    return result;
  });

  // Pagination
  let totalPages = $derived(Math.max(1, Math.ceil(filteredTatamis.length / pageSize)));
  let paginatedTatamis = $derived(
    filteredTatamis.slice((currentPage - 1) * pageSize, currentPage * pageSize)
  );

  // Create Tatami Modal
  function openAddModal() {
    const maxNum = effectiveTatamis.length > 0 ? Math.max(...effectiveTatamis.map((t) => t.tatami_number || 0)) : 0;
    newTatamiNumber = maxNum + 1;
    addError = null;
    isAddingModal = true;
  }

  async function handleAddTatami() {
    if (!eventId || newTatamiNumber < 1) return;
    isSubmitting = true;
    addError = null;
    try {
      await invoke("create_tatami", {
        eventId,
        tatamiNumber: newTatamiNumber,
      });

      isAddingModal = false;
      onRefreshTatamis();
      showToast(`Tatami ${newTatamiNumber} berhasil ditambahkan!`);
    } catch (err: any) {
      addError = typeof err === "string" ? err : err?.message || "Gagal menambahkan tatami.";
    } finally {
      isSubmitting = false;
    }
  }

  // Edit Tatami Modal
  function openEditModal(tatami: TatamiItem) {
    editModalTatami = tatami;
    editTatamiNumber = tatami.tatami_number ?? 1;
    editError = null;
    isEditing = false;
  }

  async function handleSaveEditTatami() {
    if (!editModalTatami || editTatamiNumber < 1) return;
    isEditing = true;
    editError = null;
    try {
      await invoke("update_tatami_number", {
        tatamiId: editModalTatami.id,
        newNumber: editTatamiNumber,
      });

      const updatedNum = editTatamiNumber;
      editModalTatami = null;
      onRefreshTatamis();
      showToast(`Nomor tatami berhasil diperbarui ke Tatami ${updatedNum}!`);
    } catch (err: any) {
      editError = typeof err === "string" ? err : err?.message || "Gagal mengubah nomor tatami.";
    } finally {
      isEditing = false;
    }
  }

  // Single Delete Tatami
  function promptDeleteTatami(tatami: TatamiItem) {
    tatamiToDelete = tatami;
  }

  async function confirmDeleteTatami() {
    if (!tatamiToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_tatami", { tatamiId: tatamiToDelete.id });

      const deletedNum = tatamiToDelete.tatami_number;
      tatamiToDelete = null;
      onRefreshTatamis();
      showToast(`Tatami ${deletedNum} berhasil dihapus.`);
    } catch (err: any) {
      showToast(`Gagal menghapus tatami: ${err?.message || err}`);
    } finally {
      isDeleting = false;
    }
  }

  // QR Code Access Modal
  async function openQrModal(tatami: TatamiItem, defaultRole: "jury" | "lo-kata" | "operator" | "coach-supervisor" | "wasit-vr" = "jury") {
    qrModalTatami = tatami;
    qrRole = defaultRole;
    if (!currentQrHost) {
      currentQrHost = hostAddress;
    }
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        availableIps = await invoke("get_available_ips");
      }
    } catch (_) {}
    await updateQrCode();
  }

  async function updateQrCode() {
    if (!qrModalTatami) return;
    const tNum = qrModalTatami.tatami_number ?? 1;
    const baseHost = currentQrHost || hostAddress;
    let targetUrl = `${baseHost}/jury/${tNum}/`;
    if (qrRole === "lo-kata") {
      targetUrl = `${baseHost}/lo-kata/${tNum}/`;
    } else if (qrRole === "coach-supervisor") {
      targetUrl = `${baseHost}/coach-supervisor/${tNum}/`;
    } else if (qrRole === "wasit-vr") {
      targetUrl = `${baseHost}/wasit-vr/${tNum}/`;
    } else if (qrRole === "operator") {
      targetUrl = `${baseHost}/control-panel?tatami=${tNum}`;
    }

    isLoadingQr = true;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        qrSvgContent = await invoke<string>("generate_qr_svg", { url: targetUrl });
      } else {
        qrSvgContent = "";
      }
    } catch (_) {
      qrSvgContent = "";
    } finally {
      isLoadingQr = false;
    }
  }

  function copyTargetUrl() {
    if (!qrModalTatami) return;
    const tNum = qrModalTatami.tatami_number ?? 1;
    const baseHost = currentQrHost || hostAddress;
    let targetUrl = `${baseHost}/jury/${tNum}/`;
    let label = "Panel Juri";
    if (qrRole === "lo-kata") {
      targetUrl = `${baseHost}/lo-kata/${tNum}/`;
      label = "LO / Guide";
    } else if (qrRole === "coach-supervisor") {
      targetUrl = `${baseHost}/coach-supervisor/${tNum}/`;
      label = "Coach Supervisor (VR)";
    } else if (qrRole === "wasit-vr") {
      targetUrl = `${baseHost}/wasit-vr/${tNum}/`;
      label = "Meja Wasit VR";
    } else if (qrRole === "operator") {
      targetUrl = `${baseHost}/control-panel?tatami=${tNum}`;
      label = "Operator Meja";
    }
    navigator.clipboard.writeText(targetUrl);
    showToast(`URL ${label} Tatami ${tNum} disalin!`);
  }

  let unlistenLiveState: any = null;
  let liveBroadcastChannel: BroadcastChannel | null = null;

  onMount(() => {
    // Initial fetch
    onRefreshTatamis();

    initLiveSync(hostAddress);
    const unsubLive = subscribeLiveSync(() => {
      onRefreshTatamis();
    });

    // Instant sync on tatami state events
    listen("tatami_live_state", () => {
      onRefreshTatamis();
    }).then((u) => {
      unlistenLiveState = u;
    }).catch(() => {});

    try {
      if (typeof window !== "undefined" && window.BroadcastChannel) {
        liveBroadcastChannel = new BroadcastChannel("ambrilindo_live_sync");
        liveBroadcastChannel.onmessage = () => {
          onRefreshTatamis();
        };
      }
    } catch (_) {}

    // Real-time synchronization: Auto-refresh tatami state every 3 seconds
    syncInterval = setInterval(() => {
      onRefreshTatamis();
    }, 3000);

    return () => {
      unsubLive();
      if (unlistenLiveState) unlistenLiveState();
      if (liveBroadcastChannel) liveBroadcastChannel.close();
    };
  });

  onDestroy(() => {
    if (syncInterval) clearInterval(syncInterval);
    if (toastTimer) clearTimeout(toastTimer);
  });
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Toast Notification Overlay -->
  {#if toastMsg}
    <div class="absolute top-4 right-6 z-50 animate-in fade-in slide-in-from-top-2 duration-150">
      <div class="bg-white text-black font-semibold text-xs px-4 py-2.5 rounded-xl shadow-2xl flex items-center space-x-2 border border-zinc-200">
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
        <span>{toastMsg}</span>
      </div>
    </div>
  {/if}

  <!-- Header Bar -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4 shrink-0">
    <div>
      <div class="flex items-center space-x-2.5">
        <h2 class="text-xl font-black text-white tracking-tight uppercase">MANAJEMEN TATAMI</h2>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
          {totalTatamis} ARENA
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1 font-normal">
        Alokasi arena pertandingan dan tautan akses venue (Scoreboard TV, Operator Meja, dan Juri)
      </p>
    </div>

    <!-- Actions (No manual refresh button needed, realtime sync active) -->
    <div class="flex items-center space-x-2 shrink-0 self-start sm:self-auto">
      <button
        onclick={openAddModal}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors active:scale-[0.98] flex items-center space-x-2 cursor-pointer"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
        <span>Tambah Tatami</span>
      </button>
    </div>
  </div>

  <!-- KPI Stats Cards (4-Column Clean Grid) -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 my-4 shrink-0">
    <!-- 1. Total Tatami -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">TOTAL ARENA</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalTatamis}</span>
        <span class="text-[10px] font-mono text-zinc-500">tatami</span>
      </div>
    </div>

    <!-- 2. Tatami Live -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <div class="flex items-center justify-between">
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">TATAMI LIVE</span>
        {#if activeMatchesCount > 0}
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
        {/if}
      </div>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-amber-400 tabular-nums">{activeMatchesCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">partai aktif</span>
      </div>
    </div>

    <!-- 3. Standby / Siap -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">STANDBY / READY</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-emerald-400 tabular-nums">{readyCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">siap tanding</span>
      </div>
    </div>

    <!-- 4. Status Server Host LAN -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">HUB SERVER LAN</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-xl sm:text-2xl font-black font-mono text-white truncate">{hostAddress ? 'ONLINE' : 'LOKAL'}</span>
        <span class="text-[10px] font-mono text-zinc-500">port 8000</span>
      </div>
    </div>
  </div>

  <!-- Toolbar: Filter Pills, Sort & Search -->
  <div class="flex flex-col md:flex-row md:items-center justify-between gap-3 mb-3 shrink-0">
    <!-- Quick Status Filter Pills -->
    <div class="flex flex-wrap items-center gap-1.5 overflow-x-auto py-0.5">
      <button
        onclick={() => {
          selectedStatusFilter = 'all';
          currentPage = 1;
        }}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'all'
          ? 'bg-white text-black border-white shadow-sm'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Semua ({totalTatamis})
      </button>

      <button
        onclick={() => {
          selectedStatusFilter = 'live';
          currentPage = 1;
        }}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'live'
          ? 'bg-amber-400 text-black border-amber-400 shadow-sm font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Running ({activeMatchesCount})
      </button>

      <button
        onclick={() => {
          selectedStatusFilter = 'ready';
          currentPage = 1;
        }}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'ready'
          ? 'bg-emerald-400 text-black border-emerald-400 shadow-sm font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Standby ({readyCount})
      </button>
    </div>

    <!-- Sort & Search Bar -->
    <div class="flex items-center space-x-2.5 shrink-0">
      <!-- Sort Select -->
      <select
        bind:value={selectedSort}
        class="bg-[#181a20] text-xs text-white px-3 py-1.5 rounded-xl border border-white/[0.1] focus:border-white outline-none cursor-pointer"
      >
        <option value="tatami_asc">Nomor Tatami (1 → N)</option>
        <option value="tatami_desc">Nomor Tatami (N → 1)</option>
        <option value="live_first">Partai Live Dahulu</option>
      </select>

      <!-- Search Input -->
      <div class="relative w-56">
        <svg
          class="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2 pointer-events-none"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          stroke-width="2"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
          />
        </svg>
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari tatami, partai..."
          class="w-full bg-[#181a20] text-xs text-white pl-8 pr-7 py-1.5 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors placeholder-zinc-500"
        />
        {#if searchKeyword}
          <button
            onclick={() => (searchKeyword = "")}
            aria-label="Hapus pencarian"
            class="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-white text-xs cursor-pointer"
          >
            ✕
          </button>
        {/if}
      </div>
    </div>
  </div>

  <!-- Data Table Container (Clean 5-Column Layout) -->
  <div class="flex-1 overflow-y-auto min-h-0 rounded-2xl border border-white/[0.08] bg-[#181a20] shadow-lg flex flex-col">
    <table class="w-full text-left text-xs border-collapse">
      <thead class="bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider sticky top-0 z-10 border-b border-white/[0.08]">
        <tr>
          <th class="w-12 px-3 py-3 font-bold text-center">NO</th>
          <th class="px-4 py-3 font-bold">ARENA TATAMI</th>
          <th class="px-4 py-3 font-bold">BAGAN AKTIF</th>
          <th class="px-4 py-3 font-bold">STATUS ARENA</th>
          <th class="px-4 py-3 font-bold">AKSES VENUE</th>
          <th class="w-24 px-4 py-3 font-bold text-center pr-6">AKSI</th>
        </tr>
      </thead>

      <tbody class="divide-y divide-white/[0.06] font-medium">
        {#if paginatedTatamis.length === 0}
          <tr>
            <td colspan="6" class="py-16 text-center text-zinc-500">
              <div class="flex flex-col items-center justify-center space-y-2">
                <svg class="w-8 h-8 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                </svg>
                <span class="text-xs font-semibold text-zinc-400">Tidak ada data tatami yang sesuai</span>
                <span class="text-[11px] text-zinc-600">Gunakan tombol "Tambah Tatami" di atas untuk menambah arena.</span>
              </div>
            </td>
          </tr>
        {:else}
          {#each paginatedTatamis as tatami, idx}
            {@const tNum = tatami.tatami_number ?? 1}
            <tr class="hover:bg-white/[0.02] transition-colors">
              <!-- No -->
              <td class="px-3 py-3 font-mono text-zinc-500 text-center text-[11px] tabular-nums">
                {(currentPage - 1) * pageSize + idx + 1}
              </td>

              <!-- Tatami Number Badge (Clean, no inline edit icon) -->
              <td class="px-4 py-3">
                <div class="flex items-center space-x-3">
                  <div class="w-8 h-8 rounded-lg bg-white/[0.04] border border-white/[0.08] font-mono font-bold text-sm flex items-center justify-center text-white shrink-0">
                    {tNum}
                  </div>
                  <div>
                    <span class="font-bold text-white text-xs block">Tatami {tNum}</span>
                    <span class="text-[10px] font-mono text-zinc-500 block">ID #{tatami.id}</span>
                  </div>
                </div>
              </td>

              <!-- Bagan / Kelas Aktif -->
              <td class="px-4 py-3">
                {#if tatami.nama_bagan}
                  <div class="flex items-center space-x-2 text-white font-bold text-xs truncate max-w-xs xl:max-w-md">
                    <span class="w-2 h-2 rounded-full bg-amber-400 shrink-0 animate-pulse"></span>
                    <span class="truncate uppercase tracking-tight" title={tatami.nama_bagan}>{tatami.nama_bagan}</span>
                  </div>
                  {#if tatami.detail_bagan_id}
                    <div class="flex items-center space-x-1.5 mt-0.5">
                      <span class="text-[10px] font-mono text-zinc-400 pl-4 block tabular-nums">Partai #{tatami.detail_bagan_id}</span>
                      <span class="text-[9px] font-mono font-black px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 border border-amber-500/30 uppercase tracking-wider animate-pulse">Running</span>
                    </div>
                  {/if}
                {:else if tatami.detail_bagan_id}
                  <div class="flex items-center space-x-2 text-zinc-200 font-bold text-xs">
                    <span class="w-2 h-2 rounded-full bg-amber-400 shrink-0 animate-pulse"></span>
                    <span class="font-mono tabular-nums">Partai #{tatami.detail_bagan_id}</span>
                    <span class="text-[9px] font-mono font-black px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 border border-amber-500/30 uppercase tracking-wider animate-pulse">Running</span>
                  </div>
                {:else}
                  <span class="text-zinc-600 font-mono text-[11px]">-</span>
                {/if}
              </td>

              <!-- Status Arena -->
              <td class="px-4 py-3">
                {#if tatami.detail_bagan_id}
                  <span class="inline-flex items-center space-x-1.5 px-2.5 py-1 rounded text-[10px] font-mono font-black bg-amber-500/15 text-amber-300 border border-amber-500/30 animate-pulse shadow-xs">
                    <span class="w-2 h-2 rounded-full bg-amber-400 animate-ping"></span>
                    <span>MATCH RUNNING</span>
                  </span>
                {:else}
                  <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-white/[0.04] text-zinc-400 border border-white/[0.08]">
                    <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                    <span>READY</span>
                  </span>
                {/if}
              </td>

              <!-- Akses Venue (Dedicated Portal Shortcuts) -->
              <td class="px-4 py-3">
                <div class="flex items-center space-x-2">
                  <!-- Scoreboard TV Display Link -->
                  <a
                    href="{hostAddress}/scoreboard?tatami={tNum}"
                    target="_blank"
                    class="h-7 px-2.5 rounded-lg bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 text-[11px] font-semibold transition-colors flex items-center space-x-1.5"
                    title="Buka Scoreboard TV Arena Tatami {tNum} di browser / proyektor"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                      <line x1="8" y1="21" x2="16" y2="21"/>
                      <line x1="12" y1="17" x2="12" y2="21"/>
                    </svg>
                    <span>Scoreboard</span>
                  </a>

                  <!-- Operator Meja Link -->
                  <a
                    href="{hostAddress}/tatami-{tNum}/"
                    target="_blank"
                    class="h-7 px-2.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] text-[11px] font-semibold transition-colors flex items-center"
                    title="Buka panel operator meja tatami di browser"
                  >
                    Operator
                  </a>

                  <!-- Juri Link -->
                  <a
                    href="{hostAddress}/jury/{tNum}/"
                    target="_blank"
                    class="h-7 px-2.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] text-[11px] font-semibold transition-colors flex items-center"
                    title="Buka panel wasit / juri di browser"
                  >
                    Juri
                  </a>

                  <!-- QR Code Button -->
                  <button
                    onclick={() => openQrModal(tatami, "jury")}
                    class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/[0.08] hover:border-white/20 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                    title="Pindai QR code untuk smartphone/tablet juri & operator"
                    aria-label="Akses QR Code Tatami"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 4.875c0-.621.504-1.125 1.125-1.125h4.5c.621 0 1.125.504 1.125 1.125v4.5c0 .621-.504 1.125-1.125 1.125h-4.5A1.125 1.125 0 0 1 3.75 9.375v-4.5ZM3.75 14.625c0-.621.504-1.125 1.125-1.125h4.5c.621 0 1.125.504 1.125 1.125v4.5c0 .621-.504 1.125-1.125 1.125h-4.5a1.125 1.125 0 0 1-1.125-1.125v-4.5ZM13.5 4.875c0-.621.504-1.125 1.125-1.125h4.5c.621 0 1.125.504 1.125 1.125v4.5c0 .621-.504 1.125-1.125 1.125h-4.5A1.125 1.125 0 0 1 13.5 9.375v-4.5Z" />
                      <path stroke-linecap="round" stroke-linejoin="round" d="M6.75 6.75h.75v.75h-.75v-.75ZM6.75 16.5h.75v.75h-.75v-.75ZM16.5 6.75h.75v.75h-.75v-.75ZM13.5 13.5h3v3h-3v-3ZM13.5 19.5h3v1.5h-3v-1.5ZM19.5 13.5h1.5v3H19.5v-3ZM19.5 19.5h1.5v1.5H19.5v-1.5ZM16.5 16.5h3v3h-3v-3Z" />
                    </svg>
                  </button>
                </div>
              </td>

              <!-- Aksi (Dedicated Arena Management Actions) -->
              <td class="w-24 px-4 py-3 text-center pr-6">
                <div class="flex items-center justify-center space-x-1.5">
                  <!-- Ubah Nomor Tatami Button -->
                  <button
                    onclick={() => openEditModal(tatami)}
                    class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-sky-500/15 text-zinc-400 hover:text-sky-300 border border-white/[0.08] hover:border-sky-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                    title="Ubah nomor tatami"
                    aria-label="Ubah nomor tatami"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125" />
                    </svg>
                  </button>

                  <!-- Hapus Tatami Button -->
                  <button
                    onclick={() => promptDeleteTatami(tatami)}
                    class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-rose-500/15 text-zinc-400 hover:text-rose-400 border border-white/[0.08] hover:border-rose-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                    title={`Hapus Tatami ${tNum}`}
                    aria-label={`Hapus Tatami ${tNum}`}
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>

  <!-- Pagination Footer -->
  <div class="bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0 font-sans">
    <div class="tabular-nums">
      Menampilkan {paginatedTatamis.length > 0 ? (currentPage - 1) * pageSize + 1 : 0} - {Math.min(currentPage * pageSize, filteredTatamis.length)} dari {filteredTatamis.length} tatami
    </div>

    <div class="flex items-center space-x-4">
      <div class="flex items-center space-x-1.5">
        <span>Per baris:</span>
        <select
          bind:value={pageSize}
          onchange={() => (currentPage = 1)}
          class="bg-[#181a20] text-white border border-white/[0.1] rounded-lg px-2 py-0.5 text-xs outline-none focus:border-white cursor-pointer"
        >
          <option value={10}>10</option>
          <option value={25}>25</option>
          <option value={50}>50</option>
          <option value={100}>100</option>
        </select>
      </div>

      <div class="flex items-center space-x-1">
        <button
          onclick={() => (currentPage = Math.max(1, currentPage - 1))}
          disabled={currentPage === 1}
          class="h-7 px-2.5 rounded-lg bg-white/[0.04] border border-white/[0.08] hover:bg-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors cursor-pointer"
        >
          ‹
        </button>
        <span class="px-2 font-mono text-zinc-300 tabular-nums">
          {currentPage} / {totalPages}
        </span>
        <button
          onclick={() => (currentPage = Math.min(totalPages, currentPage + 1))}
          disabled={currentPage >= totalPages}
          class="h-7 px-2.5 rounded-lg bg-white/[0.04] border border-white/[0.08] hover:bg-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors cursor-pointer"
        >
          ›
        </button>
      </div>
    </div>
  </div>

  <!-- Modal: Tambah Tatami Baru -->
  {#if isAddingModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-3">
          <h3 class="text-sm font-bold text-white tracking-tight">Tambah Tatami Baru</h3>
          <button
            onclick={() => (isAddingModal = false)}
            class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        {#if addError}
          <div class="p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs">
            {addError}
          </div>
        {/if}

        <div class="space-y-1.5">
          <label for="tatami-num-input" class="text-xs font-semibold text-zinc-300 block">Nomor Tatami</label>
          <input
            id="tatami-num-input"
            type="number"
            min="1"
            bind:value={newTatamiNumber}
            class="w-full bg-[#111215] border border-white/[0.1] text-white font-mono text-sm px-3 py-2 rounded-xl outline-none focus:border-white transition-colors"
          />
        </div>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={() => (isAddingModal = false)}
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
          >
            Batal
          </button>
          <button
            onclick={handleAddTatami}
            disabled={isSubmitting}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors disabled:opacity-50 cursor-pointer"
          >
            {isSubmitting ? "Menyimpan..." : "Simpan Tatami"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: Ubah Nomor Tatami -->
  {#if editModalTatami}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-3">
          <h3 class="text-sm font-bold text-white tracking-tight">Ubah Nomor Tatami</h3>
          <button
            onclick={() => (editModalTatami = null)}
            class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        {#if editError}
          <div class="p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs">
            {editError}
          </div>
        {/if}

        <div class="space-y-1.5">
          <label for="edit-tatami-num-input" class="text-xs font-semibold text-zinc-300 block">Nomor Tatami Baru</label>
          <input
            id="edit-tatami-num-input"
            type="number"
            min="1"
            bind:value={editTatamiNumber}
            class="w-full bg-[#111215] border border-white/[0.1] text-white font-mono text-sm px-3 py-2 rounded-xl outline-none focus:border-white transition-colors"
          />
        </div>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={() => (editModalTatami = null)}
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
          >
            Batal
          </button>
          <button
            onclick={handleSaveEditTatami}
            disabled={isEditing}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors disabled:opacity-50 cursor-pointer"
          >
            {isEditing ? "Menyimpan..." : "Simpan Perubahan"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: Konfirmasi Hapus Single Tatami -->
  {#if tatamiToDelete}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
            </svg>
          </div>
          <div>
            <h3 class="text-sm font-bold text-white tracking-tight">Hapus Tatami {tatamiToDelete.tatami_number}</h3>
          </div>
        </div>

        <p class="text-xs text-zinc-300 leading-relaxed">
          Apakah Anda yakin ingin menghapus <strong>Tatami {tatamiToDelete.tatami_number}</strong> dari turnamen ini?
        </p>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={() => (tatamiToDelete = null)}
            disabled={isDeleting}
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
          >
            Batal
          </button>
          <button
            onclick={confirmDeleteTatami}
            disabled={isDeleting}
            class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl shadow-md transition-colors disabled:opacity-50 flex items-center space-x-1.5 cursor-pointer"
          >
            <span>{isDeleting ? "Menghapus..." : "Hapus Tatami"}</span>
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: QR Code Akses Tatami Mobile -->
  {#if qrModalTatami}
    {@const tNum = qrModalTatami.tatami_number ?? 1}
    {@const baseHost = currentQrHost || hostAddress}
    {@const targetUrl = qrRole === "jury" ? `${baseHost}/jury/${tNum}/` : (qrRole === "lo-kata" ? `${baseHost}/lo-kata/${tNum}/` : `${baseHost}/control-panel?tatami=${tNum}`)}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150 text-center">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-2.5 text-left">
          <h3 class="text-sm font-bold text-white tracking-tight">Akses Mobile Tatami {tNum}</h3>
          <button
            onclick={() => (qrModalTatami = null)}
            class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        <!-- Role Toggle Tabs -->
        <div class="grid grid-cols-3 gap-1 p-1 bg-[#111215] rounded-xl border border-white/[0.08]">
          <button
            onclick={() => {
              qrRole = "jury";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'jury'
              ? 'bg-white text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Panel Juri
          </button>
          <button
            onclick={() => {
              qrRole = "lo-kata";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'lo-kata'
              ? 'bg-white text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            LO / Guide
          </button>
          <button
            onclick={() => {
              qrRole = "operator";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'operator'
              ? 'bg-white text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Meja Tatami
          </button>
          <button
            onclick={() => {
              qrRole = "coach-supervisor";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'coach-supervisor'
              ? 'bg-red-500 text-white shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Coach VR
          </button>
          <button
            onclick={() => {
              qrRole = "wasit-vr";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors col-span-2 cursor-pointer {qrRole === 'wasit-vr'
              ? 'bg-amber-500 text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Meja Wasit VR
          </button>
        </div>

        <!-- IP Venue Network Selector if multiple interfaces available -->
        {#if availableIps.length > 1}
          <div class="flex items-center justify-between gap-2 px-3 py-1.5 bg-[#111215] rounded-xl border border-white/[0.08] text-[11px] text-left">
            <span class="text-zinc-500 font-mono font-bold uppercase text-[9px] shrink-0">IP Venue:</span>
            <select
              value={currentQrHost.replace(/^https?:\/\//, "").replace(/:\d+.*$/, "")}
              onchange={(e) => {
                const targetIp = (e.target as HTMLSelectElement).value;
                currentQrHost = `http://${targetIp}:8000`;
                updateQrCode();
              }}
              class="bg-transparent text-white font-mono font-bold outline-none cursor-pointer text-right flex-1 truncate"
            >
              {#each availableIps as item}
                <option value={item.ip} class="bg-[#181a20] text-white">
                  {item.ip} ({item.name})
                </option>
              {/each}
            </select>
          </div>
        {/if}

        <!-- QR Code Canvas Display -->
        {#if isLoadingQr}
          <div class="w-52 h-52 mx-auto flex items-center justify-center bg-white/[0.02] border border-white/[0.08] rounded-xl text-xs text-zinc-500 font-mono">
            Menghasilkan QR Code...
          </div>
        {:else if qrSvgContent}
          <div class="bg-white p-3 rounded-xl w-52 h-52 mx-auto flex items-center justify-center shadow-lg">
            {@html qrSvgContent}
          </div>
        {:else}
          <div class="w-52 h-52 mx-auto flex flex-col items-center justify-center bg-white/[0.02] border border-white/[0.08] rounded-xl p-4 space-y-2">
            <svg class="w-8 h-8 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4v1m6 11h2m-6 0h-2v4m0-11v3m0 0h.01M12 12h4.01M16 20h4M4 12h4m12 0h.01M5 8h2a1 1 0 001-1V5a1 1 0 00-1-1H5a1 1 0 00-1 1v2a1 1 0 001 1zm12 0h2a1 1 0 001-1V5a1 1 0 00-1-1h-2a1 1 0 00-1 1v2a1 1 0 001 1zM5 20h2a1 1 0 001-1v-2a1 1 0 00-1-1H5a1 1 0 00-1 1v2a1 1 0 001 1z" />
            </svg>
            <span class="text-xs text-zinc-400 font-mono break-all">{targetUrl}</span>
          </div>
        {/if}

        <!-- Target URL Box -->
        <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-2.5 text-left">
          <span class="text-[9px] font-mono font-bold text-zinc-500 uppercase tracking-wider block">ALAMAT URL VENUE</span>
          <p class="text-xs font-mono font-semibold text-white truncate mt-0.5 select-all">{targetUrl}</p>
        </div>

        <div class="flex items-center justify-between pt-2 border-t border-white/[0.08]">
          <a
            href={targetUrl}
            target="_blank"
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors flex items-center space-x-1.5"
          >
            <span>Buka di Tab</span>
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 6H5.25A2.25 2.25 0 0 0 3 8.25v10.5A2.25 2.25 0 0 0 5.25 21h10.5A2.25 2.25 0 0 0 18 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25" />
            </svg>
          </a>

          <div class="flex items-center space-x-2">
            <button
              onclick={copyTargetUrl}
              class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
            >
              Salin URL
            </button>
            <button
              onclick={() => (qrModalTatami = null)}
              class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors cursor-pointer"
            >
              Tutup
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>
