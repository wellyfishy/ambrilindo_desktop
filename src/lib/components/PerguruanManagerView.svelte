<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke, isClientMode, getConnectionConfig } from "$lib/api_client";
  import type { EnrichedPerguruanItem, ActiveChannel } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
    onNavigate?: (channel: ActiveChannel) => void;
    onShowToast?: (msg: string) => void;
  }

  let {
    eventId,
    eventName = "Event Turnamen",
    hostAddress,
    onNavigate,
    onShowToast,
  }: Props = $props();

  // Data State
  let perguruans = $state<EnrichedPerguruanItem[]>([]);
  let isLoading = $state(false);

  // Filter & Search
  let searchKeyword = $state("");
  let filterType = $state<"all" | "has_athletes" | "no_athletes">("all");
  let sortBy = $state<"name_asc" | "name_desc" | "athletes_desc" | "athletes_asc">("name_asc");

  // Modals State
  let modalOpen = $state(false);
  let isEditMode = $state(false);
  let isSaving = $state(false);
  let formError = $state<string | null>(null);

  let formInput = $state<{
    id?: number | null;
    nama_perguruan: string;
  }>({
    nama_perguruan: "",
  });

  // Delete Modal
  let deleteModalOpen = $state(false);
  let itemToDelete = $state<EnrichedPerguruanItem | null>(null);
  let isDeleting = $state(false);

  // Cloud Pull State
  let isPullingCloud = $state(false);

  // Toast
  let toastMsg = $state<string | null>(null);
  let toastTimer: any = null;

  function showToast(msg: string) {
    toastMsg = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMsg = null;
    }, 3500);
    onShowToast?.(msg);
  }

  // Derived calculations
  let filteredPerguruans = $derived.by<EnrichedPerguruanItem[]>(() => {
    let list = perguruans.filter((p) => {
      if (searchKeyword.trim()) {
        const kw = searchKeyword.toLowerCase();
        if (!p.nama_perguruan?.toLowerCase().includes(kw)) return false;
      }
      if (filterType === "has_athletes") return (p.athlete_count || 0) > 0;
      if (filterType === "no_athletes") return (p.athlete_count || 0) === 0;
      return true;
    });

    return list.sort((a, b) => {
      if (sortBy === "name_asc") return (a.nama_perguruan || "").localeCompare(b.nama_perguruan || "");
      if (sortBy === "name_desc") return (b.nama_perguruan || "").localeCompare(a.nama_perguruan || "");
      if (sortBy === "athletes_desc") return (b.athlete_count || 0) - (a.athlete_count || 0);
      if (sortBy === "athletes_asc") return (a.athlete_count || 0) - (b.athlete_count || 0);
      return 0;
    });
  });

  let totalPerguruans = $derived(perguruans.length);
  let totalAthletes = $derived(perguruans.reduce((acc, p) => acc + (p.athlete_count || 0), 0));
  let perguruansWithAthletes = $derived(perguruans.filter((p) => (p.athlete_count || 0) > 0).length);

  // Load Data
  async function loadData() {
    isLoading = true;
    try {
      const data = await invoke<EnrichedPerguruanItem[]>("get_enriched_perguruans", {
        eventId,
        event_id: eventId,
      });
      perguruans = data || [];
    } catch (err: any) {
      console.error("Gagal memuat perguruan:", err);
      showToast(`Gagal memuat data: ${err?.message || err}`);
    } finally {
      isLoading = false;
    }
  }

  // Add / Edit Modal
  function openAddModal() {
    isEditMode = false;
    formError = null;
    formInput = {
      id: null,
      nama_perguruan: "",
    };
    modalOpen = true;
  }

  function openEditModal(item: EnrichedPerguruanItem) {
    isEditMode = true;
    formError = null;
    formInput = {
      id: item.id,
      nama_perguruan: item.nama_perguruan || "",
    };
    modalOpen = true;
  }

  async function handleSave() {
    formError = null;
    const name = formInput.nama_perguruan.trim().toUpperCase();
    if (!name) {
      formError = "Nama Perguruan Karate wajib diisi.";
      return;
    }

    isSaving = true;
    try {
      if (isEditMode && formInput.id) {
        await invoke("update_perguruan", {
          id: formInput.id,
          namaPerguruan: name,
          nama_perguruan: name,
        });
        showToast(`Perguruan "${name}" berhasil diperbarui.`);
      } else {
        await invoke("create_perguruan", {
          eventId,
          event_id: eventId,
          namaPerguruan: name,
          nama_perguruan: name,
        });
        showToast(`Perguruan "${name}" berhasil ditambahkan.`);
      }

      modalOpen = false;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menyimpan perguruan:", err);
      formError = typeof err === "string" ? err : err?.message || JSON.stringify(err);
    } finally {
      isSaving = false;
    }
  }

  // Delete
  function openDeleteModal(item: EnrichedPerguruanItem) {
    itemToDelete = item;
    deleteModalOpen = true;
  }

  async function confirmDelete() {
    if (!itemToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_perguruan", { id: itemToDelete.id });
      showToast(`Perguruan "${itemToDelete.nama_perguruan}" berhasil dihapus.`);
      deleteModalOpen = false;
      itemToDelete = null;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menghapus perguruan:", err);
      showToast(`Gagal menghapus: ${err?.message || err}`);
    } finally {
      isDeleting = false;
    }
  }

  // Pull Cloud Data
  async function handlePullCloud() {
    isPullingCloud = true;
    try {
      const res = await invoke<any>("pull_athletes_from_hosted", {
        eventId,
        event_id: eventId,
      });
      if (res && res.pulled_athletes !== undefined) {
        showToast(`Sinkronisasi Cloud Berhasil: ${res.pulled_athletes} data diproses.`);
      } else {
        showToast("Sinkronisasi cloud selesai.");
      }
      await loadData();
    } catch (err: any) {
      console.error("Gagal tarik data cloud:", err);
      showToast(`Sinkronisasi gagal: ${err?.message || err}`);
    } finally {
      isPullingCloud = false;
    }
  }

  // Real-time Event Listener & WebSocket
  let unlistenTauri: (() => void) | null = null;
  let wsClient: WebSocket | null = null;

  onMount(() => {
    loadData();

    // Listen to Tauri events (Host/Solo mode)
    if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen("master_data_updated", (event: any) => {
          if (!event?.payload?.type || event.payload.type === "perguruan_updated") {
            loadData();
          }
        }).then((unlisten) => {
          unlistenTauri = unlisten;
        });
      });
    }

    // Connect to LAN WebSocket for real-time sync across multi-laptop LAN
    try {
      const config = getConnectionConfig();
      const host = config.hostUrl.replace(/^http:\/\//, "").replace(/^https:\/\//, "");
      const wsUrl = `ws://${host}/ws/master-data/0`;
      wsClient = new WebSocket(wsUrl);
      wsClient.onmessage = (ev) => {
        try {
          const msg = JSON.parse(ev.data);
          if (msg.type === "perguruan_updated") {
            loadData();
          }
        } catch {}
      };
    } catch (e) {
      // WS fallback silent
    }
  });

  onDestroy(() => {
    if (unlistenTauri) unlistenTauri();
    if (wsClient) {
      wsClient.close();
      wsClient = null;
    }
    if (toastTimer) clearTimeout(toastTimer);
  });
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Toast Notification -->
  {#if toastMsg}
    <div class="absolute top-4 right-6 z-50 bg-[#181a20] border border-white/20 text-white text-xs font-semibold px-4 py-2.5 rounded-xl shadow-2xl flex items-center space-x-2 transition-all animate-in fade-in slide-in-from-top-2">
      <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
      <span>{toastMsg}</span>
    </div>
  {/if}

  <!-- Header Section -->
  <div class="flex items-center justify-between pb-4 border-b border-white/[0.08] shrink-0">
    <div>
      <div class="flex items-center space-x-2.5">
        <div class="w-8 h-8 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center justify-center">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        </div>
        <h2 class="text-xl font-black text-white tracking-tight uppercase">
          MANAJEMEN PERGURUAN KARATE (FORKI)
        </h2>
        <span class="px-2.5 py-0.5 rounded text-[11px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/20 tabular-nums">
          {totalPerguruans} PERGURUAN
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1">
        Kelola daftar aliran / perguruan karate FORKI (INKAI, LEMKARI, KKI, INKANAS, ASKI, dll.) dan keanggotaan atlet
      </p>
    </div>

    <!-- Action Buttons -->
    <div class="flex items-center space-x-2">
      <button
        onclick={handlePullCloud}
        disabled={isPullingCloud}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.1] text-xs font-semibold rounded-xl flex items-center space-x-1.5 transition-colors disabled:opacity-50"
        title="Tarik data perguruan & atlet dari server online Ambrilindo"
      >
        <svg class="w-3.5 h-3.5 text-sky-400 {isPullingCloud ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
        </svg>
        <span>{isPullingCloud ? "Menyinkronkan..." : "Tarik Cloud"}</span>
      </button>

      <button
        onclick={loadData}
        disabled={isLoading}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.1] text-xs font-semibold rounded-xl flex items-center space-x-1.5 transition-colors"
        title="Muat ulang data perguruan dari database lokal"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400 {isLoading ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span>Segarkan</span>
      </button>

      <button
        onclick={openAddModal}
        class="h-9 px-3.5 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl flex items-center space-x-1.5 transition-colors shadow-md active:scale-95"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
        </svg>
        <span>Tambah Perguruan</span>
      </button>
    </div>
  </div>

  <!-- Metric Statistics Cards -->
  <div class="grid grid-cols-1 sm:grid-cols-3 gap-3 my-4 shrink-0">
    <!-- Card 1: Total Perguruan -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Total Perguruan FORKI</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-white tabular-nums">{totalPerguruans}</span>
          <span class="text-xs text-zinc-500 font-medium">Perguruan</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-amber-500/10 text-amber-400 border border-amber-500/20 flex items-center justify-center">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
      </div>
    </div>

    <!-- Card 2: Total Atlet Afiliasi -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Total Atlet Berperguruan</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-emerald-400 tabular-nums">{totalAthletes}</span>
          <span class="text-xs text-zinc-500 font-medium">Atlet Terdaftar</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center justify-center">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
        </svg>
      </div>
    </div>

    <!-- Card 3: Perguruan Aktif -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Perguruan Memiliki Atlet</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-sky-400 tabular-nums">{perguruansWithAthletes}</span>
          <span class="text-xs text-zinc-500 font-medium">/ {totalPerguruans} Aktif</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-sky-500/10 text-sky-400 border border-sky-500/20 flex items-center justify-center">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      </div>
    </div>
  </div>

  <!-- Filter & Search Toolbar -->
  <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3 mb-3 shrink-0">
    <div class="flex items-center space-x-2 flex-1">
      <!-- Search Input -->
      <div class="relative flex-1 max-w-md">
        <svg class="w-4 h-4 text-zinc-400 absolute left-3 top-2.5 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari nama perguruan (misal: INKAI, LEMKARI, KKI)..."
          class="w-full bg-[#181a20] border border-white/[0.08] text-xs text-white rounded-xl pl-9 pr-3 py-2 outline-none focus:border-amber-400/50 transition-colors placeholder:text-zinc-500"
        />
        {#if searchKeyword}
          <button
            onclick={() => (searchKeyword = "")}
            class="absolute right-2.5 top-2.5 text-zinc-500 hover:text-white text-xs"
          >
            ✕
          </button>
        {/if}
      </div>

      <!-- Quick Filter -->
      <select
        bind:value={filterType}
        class="bg-[#181a20] border border-white/[0.08] text-xs text-zinc-300 rounded-xl px-3 py-2 outline-none focus:border-amber-400/50 transition-colors"
      >
        <option value="all">Semua Perguruan ({perguruans.length})</option>
        <option value="has_athletes">Dengan Atlet ({perguruansWithAthletes})</option>
        <option value="no_athletes">Tanpa Atlet ({totalPerguruans - perguruansWithAthletes})</option>
      </select>
    </div>

    <!-- Sort Dropdown -->
    <div class="flex items-center space-x-2">
      <span class="text-xs text-zinc-400">Urutkan:</span>
      <select
        bind:value={sortBy}
        class="bg-[#181a20] border border-white/[0.08] text-xs text-zinc-300 rounded-xl px-3 py-2 outline-none focus:border-amber-400/50 transition-colors"
      >
        <option value="name_asc">Nama (A - Z)</option>
        <option value="name_desc">Nama (Z - A)</option>
        <option value="athletes_desc">Atlet Terbanyak</option>
        <option value="athletes_asc">Atlet Paling Sedikit</option>
      </select>
    </div>
  </div>

  <!-- Table Container -->
  <div class="flex-1 bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden flex flex-col shadow-inner">
    <div class="overflow-y-auto flex-1">
      <table class="w-full text-left border-collapse text-xs">
        <thead class="bg-[#111215] sticky top-0 z-10 border-b border-white/[0.08] text-zinc-400 uppercase tracking-wider text-[10px] font-mono">
          <tr>
            <th class="py-3 px-4 w-12 text-center">No</th>
            <th class="py-3 px-4">Nama Perguruan Karate</th>
            <th class="py-3 px-4 w-36 text-center">Afiliasi Federasi</th>
            <th class="py-3 px-4 w-36 text-center">Total Atlet</th>
            <th class="py-3 px-4 w-44 text-right">Aksi</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-white/[0.04]">
          {#if isLoading && perguruans.length === 0}
            <tr>
              <td colspan="5" class="py-12 text-center text-zinc-500">
                <div class="flex flex-col items-center justify-center space-y-2">
                  <svg class="w-6 h-6 animate-spin text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
                  <span class="text-xs font-medium">Memuat data perguruan...</span>
                </div>
              </td>
            </tr>
          {:else if filteredPerguruans.length === 0}
            <tr>
              <td colspan="5" class="py-12 text-center text-zinc-500">
                <div class="flex flex-col items-center justify-center space-y-2">
                  <svg class="w-8 h-8 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <p class="text-xs font-semibold text-zinc-400">Tidak ada perguruan ditemukan</p>
                  {#if searchKeyword}
                    <p class="text-[11px] text-zinc-600">Coba ubah kata kunci pencarian Anda</p>
                  {/if}
                </div>
              </td>
            </tr>
          {:else}
            {#each filteredPerguruans as item, idx (item.id)}
              <tr class="hover:bg-white/[0.02] transition-colors group">
                <!-- No -->
                <td class="py-3 px-4 text-center font-mono text-zinc-500 text-[11px] tabular-nums">
                  {idx + 1}
                </td>

                <!-- Nama Perguruan -->
                <td class="py-3 px-4">
                  <div class="font-bold text-white uppercase tracking-tight text-xs flex items-center space-x-2">
                    <span>{item.nama_perguruan}</span>
                  </div>
                  <div class="text-[10px] text-zinc-500 font-mono mt-0.5">
                    ID #{item.id}
                  </div>
                </td>

                <!-- Afiliasi Federasi -->
                <td class="py-3 px-4 text-center">
                  <span class="inline-flex items-center space-x-1 px-2.5 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/20">
                    <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
                    <span>FORKI</span>
                  </span>
                </td>

                <!-- Total Atlet -->
                <td class="py-3 px-4 text-center">
                  {#if (item.athlete_count || 0) > 0}
                    <button
                      onclick={() => onNavigate?.({
                        type: "roster",
                        id: "athletes",
                        name: "data-atlet",
                        subtitle: `Daftar Atlet - Perguruan ${item.nama_perguruan}`,
                      })}
                      class="inline-flex items-center space-x-1.5 px-2.5 py-1 rounded-full text-[11px] font-mono font-bold bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 hover:bg-emerald-500/20 transition-colors"
                      title="Lihat atlet pada perguruan ini"
                    >
                      <span>{item.athlete_count} Atlet</span>
                      <svg class="w-3 h-3 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                      </svg>
                    </button>
                  {:else}
                    <span class="text-[11px] font-mono text-zinc-500">0 Atlet</span>
                  {/if}
                </td>

                <!-- Aksi -->
                <td class="py-3 px-4 text-right">
                  <div class="flex items-center justify-end space-x-1.5">
                    <button
                      onclick={() => openEditModal(item)}
                      class="h-7 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] rounded-lg text-xs font-medium transition-colors flex items-center space-x-1"
                      title="Edit Nama Perguruan"
                    >
                      <svg class="w-3 h-3 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                      </svg>
                      <span>Edit</span>
                    </button>

                    <button
                      onclick={() => openDeleteModal(item)}
                      class="h-7 px-2.5 bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/20 rounded-lg text-xs font-medium transition-colors flex items-center space-x-1"
                      title="Hapus Perguruan"
                    >
                      <svg class="w-3 h-3 text-rose-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                      <span>Hapus</span>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>

    <!-- Table Footer Info -->
    <div class="px-4 py-2.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-between text-xs text-zinc-500 shrink-0">
      <span>Menampilkan {filteredPerguruans.length} dari {totalPerguruans} perguruan</span>
      <span class="font-mono text-[11px] text-zinc-600">Realtime LAN Sync Active • Event #{eventId}</span>
    </div>
  </div>
</div>

<!-- Modal: Tambah / Edit Perguruan -->
{#if modalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-md shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <!-- Header -->
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight flex items-center space-x-2">
          <span>{isEditMode ? "Edit Perguruan Karate" : "Tambah Perguruan Baru"}</span>
        </h3>
        <button onclick={() => (modalOpen = false)} class="text-zinc-500 hover:text-white transition-colors">
          ✕
        </button>
      </div>

      <!-- Body Form -->
      <div class="p-6 space-y-4">
        {#if formError}
          <div class="bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs px-3 py-2 rounded-xl">
            {formError}
          </div>
        {/if}

        <!-- Nama Perguruan -->
        <div>
          <label for="nama_perguruan_input" class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Nama Perguruan Karate <span class="text-rose-400">*</span>
          </label>
          <input
            id="nama_perguruan_input"
            type="text"
            bind:value={formInput.nama_perguruan}
            placeholder="Contoh: INKAI / LEMKARI / KKI / INKANAS"
            class="w-full bg-[#111215] text-xs font-semibold text-white uppercase px-3 py-2.5 rounded-xl border border-white/[0.1] focus:border-amber-400 outline-none transition-colors"
          />
          <p class="text-[11px] text-zinc-500 mt-1.5">
            Gunakan nama resmi perguruan karate yang terdaftar di FORKI (Federasi Olahraga Karate-Do Indonesia).
          </p>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-end space-x-2">
        <button
          onclick={() => (modalOpen = false)}
          disabled={isSaving}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleSave}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isSaving ? "Menyimpan..." : isEditMode ? "Simpan Perubahan" : "Simpan Perguruan"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Perguruan -->
{#if deleteModalOpen && itemToDelete}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-sm shadow-2xl p-6 animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">Hapus Perguruan</h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Apakah Anda yakin ingin menghapus perguruan <strong class="text-white uppercase">{itemToDelete.nama_perguruan}</strong> dari database turnamen?
      </p>

      {#if (itemToDelete.athlete_count || 0) > 0}
        <div class="mt-3 p-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-300 text-[11px] leading-snug">
          ⚠️ <strong>Perhatian:</strong> Terdapat <strong>{itemToDelete.athlete_count} atlet</strong> yang bernaung di perguruan ini. Menghapus perguruan akan melepaskan afiliasi perguruan atlet tersebut (atlet tidak akan dihapus dari roster).
        </div>
      {/if}

      <div class="mt-5 flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          onclick={() => { deleteModalOpen = false; itemToDelete = null; }}
          disabled={isDeleting}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={confirmDelete}
          disabled={isDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isDeleting ? "Menghapus..." : "Hapus Perguruan"}
        </button>
      </div>
    </div>
  </div>
{/if}
