<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke, isClientMode, getConnectionConfig } from "$lib/api_client";
  import type { EnrichedUtusanItem, ActiveChannel } from "$lib/types";

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
  let contingents = $state<EnrichedUtusanItem[]>([]);
  let isLoading = $state(false);
  let logoMap = $state<Record<string, string>>({});

  // Filter & Search
  let searchKeyword = $state("");
  let filterType = $state<"all" | "has_athletes" | "no_athletes" | "has_logo" | "no_logo">("all");
  let sortBy = $state<"name_asc" | "name_desc" | "athletes_desc" | "athletes_asc">("name_asc");

  // Modals State
  let modalOpen = $state(false);
  let isEditMode = $state(false);
  let isSaving = $state(false);
  let formError = $state<string | null>(null);

  let formInput = $state<{
    id?: number | null;
    nama_utusan: string;
    logo?: string | null;
    new_logo_data?: string | null;
    new_logo_name?: string | null;
  }>({
    nama_utusan: "",
  });

  // Delete Modal
  let deleteModalOpen = $state(false);
  let itemToDelete = $state<EnrichedUtusanItem | null>(null);
  let isDeleting = $state(false);

  // Cloud Pull / Sync State
  let isPullingCloud = $state(false);

  // Local Toast
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
  let filteredContingents = $derived.by<EnrichedUtusanItem[]>(() => {
    let list = contingents.filter((c) => {
      if (searchKeyword.trim()) {
        const kw = searchKeyword.toLowerCase();
        if (!c.nama_utusan?.toLowerCase().includes(kw)) return false;
      }
      if (filterType === "has_athletes") return (c.athlete_count || 0) > 0;
      if (filterType === "no_athletes") return (c.athlete_count || 0) === 0;
      if (filterType === "has_logo") return !!c.logo;
      if (filterType === "no_logo") return !c.logo;
      return true;
    });

    return list.sort((a, b) => {
      if (sortBy === "name_asc") return (a.nama_utusan || "").localeCompare(b.nama_utusan || "");
      if (sortBy === "name_desc") return (b.nama_utusan || "").localeCompare(a.nama_utusan || "");
      if (sortBy === "athletes_desc") return (b.athlete_count || 0) - (a.athlete_count || 0);
      if (sortBy === "athletes_asc") return (a.athlete_count || 0) - (b.athlete_count || 0);
      return 0;
    });
  });

  let totalContingents = $derived(contingents.length);
  let totalAthletes = $derived(contingents.reduce((acc, c) => acc + (c.athlete_count || 0), 0));
  let totalWithLogo = $derived(contingents.filter((c) => !!c.logo).length);

  // Load Data
  async function loadData() {
    isLoading = true;
    try {
      const data = await invoke<EnrichedUtusanItem[]>("get_enriched_contingents", {
        eventId,
        event_id: eventId,
      });
      contingents = data || [];
      loadMissingLogos(contingents);
    } catch (err: any) {
      console.error("Gagal memuat kontingen:", err);
      showToast(`Gagal memuat data: ${err?.message || err}`);
    } finally {
      isLoading = false;
    }
  }

  async function loadMissingLogos(list: EnrichedUtusanItem[]) {
    for (const c of list) {
      if (c.logo && !logoMap[c.logo]) {
        try {
          const url = await invoke<string | null>("get_media_data_url", {
            relativePath: c.logo,
            relative_path: c.logo,
          });
          if (url) {
            logoMap[c.logo] = url;
            logoMap = { ...logoMap };
          }
        } catch (e) {
          // ignore logo load errors
        }
      }
    }
  }

  // Add / Edit Modal
  function openAddModal() {
    isEditMode = false;
    formError = null;
    formInput = {
      id: null,
      nama_utusan: "",
      logo: null,
      new_logo_data: null,
      new_logo_name: null,
    };
    modalOpen = true;
  }

  function openEditModal(item: EnrichedUtusanItem) {
    isEditMode = true;
    formError = null;
    formInput = {
      id: item.id,
      nama_utusan: item.nama_utusan || "",
      logo: item.logo || null,
      new_logo_data: null,
      new_logo_name: null,
    };
    modalOpen = true;
  }

  function handleLogoFileChange(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      const file = input.files[0];
      const reader = new FileReader();
      reader.onload = (e) => {
        formInput.new_logo_data = e.target?.result as string;
        formInput.new_logo_name = file.name;
      };
      reader.readAsDataURL(file);
    }
  }

  function clearLogo() {
    formInput.logo = null;
    formInput.new_logo_data = null;
    formInput.new_logo_name = null;
  }

  async function handleSave() {
    formError = null;
    const name = formInput.nama_utusan.trim().toUpperCase();
    if (!name) {
      formError = "Nama Kontingen / Utusan wajib diisi.";
      return;
    }

    isSaving = true;
    try {
      let finalLogoPath = formInput.logo;

      // Upload new logo if provided
      if (formInput.new_logo_data && formInput.new_logo_name) {
        const base64Data = formInput.new_logo_data.split(",")[1] || formInput.new_logo_data;
        const uploadedPath = await invoke<string>("save_contingent_logo", {
          fileName: formInput.new_logo_name,
          file_name: formInput.new_logo_name,
          base64Data: base64Data,
          base64_data: base64Data,
        });
        finalLogoPath = uploadedPath;
      }

      if (isEditMode && formInput.id) {
        await invoke("update_contingent", {
          id: formInput.id,
          namaUtusan: name,
          nama_utusan: name,
          logo: finalLogoPath,
        });
        showToast(`Kontingen "${name}" berhasil diperbarui.`);
      } else {
        await invoke("create_contingent", {
          eventId,
          event_id: eventId,
          namaUtusan: name,
          nama_utusan: name,
          logo: finalLogoPath,
        });
        showToast(`Kontingen "${name}" berhasil ditambahkan.`);
      }

      modalOpen = false;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menyimpan kontingen:", err);
      formError = typeof err === "string" ? err : err?.message || JSON.stringify(err);
    } finally {
      isSaving = false;
    }
  }

  // Delete
  function openDeleteModal(item: EnrichedUtusanItem) {
    itemToDelete = item;
    deleteModalOpen = true;
  }

  async function confirmDelete() {
    if (!itemToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_contingent", { id: itemToDelete.id });
      showToast(`Kontingen "${itemToDelete.nama_utusan}" berhasil dihapus.`);
      deleteModalOpen = false;
      itemToDelete = null;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menghapus kontingen:", err);
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
          if (!event?.payload?.type || event.payload.type === "contingent_updated") {
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
          if (msg.type === "contingent_updated") {
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
        <div class="w-8 h-8 rounded-lg bg-purple-500/10 border border-purple-500/20 text-purple-400 flex items-center justify-center">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
          </svg>
        </div>
        <h2 class="text-xl font-black text-white tracking-tight uppercase">
          MANAJEMEN KONTINGEN & UTUSAN
        </h2>
        <span class="px-2.5 py-0.5 rounded text-[11px] font-mono font-bold bg-purple-500/10 text-purple-300 border border-purple-500/20 tabular-nums">
          {totalContingents} KONTINGEN
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1">
        Kelola dojo, kontingen pengcab/pengprov, dan logo official untuk scoring board & bagan turnamen
      </p>
    </div>

    <!-- Action Buttons -->
    <div class="flex items-center space-x-2">
      <button
        onclick={handlePullCloud}
        disabled={isPullingCloud}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.1] text-xs font-semibold rounded-xl flex items-center space-x-1.5 transition-colors disabled:opacity-50"
        title="Tarik data kontingen & atlet terbaru dari database online Ambrilindo Cloud"
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
        title="Muat ulang data kontingen dari basis data lokal"
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
        <span>Tambah Kontingen</span>
      </button>
    </div>
  </div>

  <!-- Metric Statistics Cards -->
  <div class="grid grid-cols-1 sm:grid-cols-3 gap-3 my-4 shrink-0">
    <!-- Card 1: Total Kontingen -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Total Kontingen</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-white tabular-nums">{totalContingents}</span>
          <span class="text-xs text-zinc-500 font-medium">Tim / Utusan</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-purple-500/10 text-purple-400 border border-purple-500/20 flex items-center justify-center">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
        </svg>
      </div>
    </div>

    <!-- Card 2: Total Atlet Bernaung -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Total Atlet Bernaung</span>
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

    <!-- Card 3: Logo Resmi Siap -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Logo Terverifikasi</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-sky-400 tabular-nums">{totalWithLogo}</span>
          <span class="text-xs text-zinc-500 font-medium">/ {totalContingents} Logo</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-sky-500/10 text-sky-400 border border-sky-500/20 flex items-center justify-center">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
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
          placeholder="Cari nama kontingen / utusan..."
          class="w-full bg-[#181a20] border border-white/[0.08] text-xs text-white rounded-xl pl-9 pr-3 py-2 outline-none focus:border-purple-400/50 transition-colors placeholder:text-zinc-500"
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
        class="bg-[#181a20] border border-white/[0.08] text-xs text-zinc-300 rounded-xl px-3 py-2 outline-none focus:border-purple-400/50 transition-colors"
      >
        <option value="all">Semua Kontingen ({contingents.length})</option>
        <option value="has_athletes">Dengan Atlet ({contingents.filter(c => (c.athlete_count || 0) > 0).length})</option>
        <option value="no_athletes">Tanpa Atlet ({contingents.filter(c => (c.athlete_count || 0) === 0).length})</option>
        <option value="has_logo">Dengan Logo ({totalWithLogo})</option>
        <option value="no_logo">Tanpa Logo ({totalContingents - totalWithLogo})</option>
      </select>
    </div>

    <!-- Sort Dropdown -->
    <div class="flex items-center space-x-2">
      <span class="text-xs text-zinc-400">Urutkan:</span>
      <select
        bind:value={sortBy}
        class="bg-[#181a20] border border-white/[0.08] text-xs text-zinc-300 rounded-xl px-3 py-2 outline-none focus:border-purple-400/50 transition-colors"
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
            <th class="py-3 px-4 w-20 text-center">Logo</th>
            <th class="py-3 px-4">Nama Kontingen / Utusan</th>
            <th class="py-3 px-4 w-36 text-center">Total Atlet</th>
            <th class="py-3 px-4 w-32 text-center">Status Logo</th>
            <th class="py-3 px-4 w-44 text-right">Aksi</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-white/[0.04]">
          {#if isLoading && contingents.length === 0}
            <tr>
              <td colspan="6" class="py-12 text-center text-zinc-500">
                <div class="flex flex-col items-center justify-center space-y-2">
                  <svg class="w-6 h-6 animate-spin text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
                  <span class="text-xs font-medium">Memuat data kontingen...</span>
                </div>
              </td>
            </tr>
          {:else if filteredContingents.length === 0}
            <tr>
              <td colspan="6" class="py-12 text-center text-zinc-500">
                <div class="flex flex-col items-center justify-center space-y-2">
                  <svg class="w-8 h-8 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <p class="text-xs font-semibold text-zinc-400">Tidak ada kontingen ditemukan</p>
                  {#if searchKeyword}
                    <p class="text-[11px] text-zinc-600">Coba ubah kata kunci pencarian Anda</p>
                  {/if}
                </div>
              </td>
            </tr>
          {:else}
            {#each filteredContingents as item, idx (item.id)}
              <tr class="hover:bg-white/[0.02] transition-colors group">
                <!-- No -->
                <td class="py-3 px-4 text-center font-mono text-zinc-500 text-[11px] tabular-nums">
                  {idx + 1}
                </td>

                <!-- Logo Thumbnail -->
                <td class="py-2 px-4 text-center">
                  <div class="w-10 h-10 mx-auto rounded-lg bg-[#111215] border border-white/[0.08] p-1 flex items-center justify-center overflow-hidden">
                    {#if item.logo && logoMap[item.logo]}
                      <img src={logoMap[item.logo]} alt="Logo" class="w-full h-full object-contain" />
                    {:else if item.logo}
                      <span class="text-[9px] font-mono text-zinc-600">Logo...</span>
                    {:else}
                      <span class="text-[9px] font-bold text-zinc-600 uppercase font-mono">
                        {(item.nama_utusan || "KT").substring(0, 2)}
                      </span>
                    {/if}
                  </div>
                </td>

                <!-- Nama Kontingen -->
                <td class="py-3 px-4">
                  <div class="font-bold text-white uppercase tracking-tight text-xs">
                    {item.nama_utusan}
                  </div>
                  <div class="text-[10px] text-zinc-500 font-mono mt-0.5">
                    ID #{item.id}
                  </div>
                </td>

                <!-- Total Atlet -->
                <td class="py-3 px-4 text-center">
                  {#if (item.athlete_count || 0) > 0}
                    <button
                      onclick={() => onNavigate?.({
                        type: "roster",
                        id: "athletes",
                        name: "data-atlet",
                        subtitle: `Daftar Atlet - Kontingen ${item.nama_utusan}`,
                      })}
                      class="inline-flex items-center space-x-1.5 px-2.5 py-1 rounded-full text-[11px] font-mono font-bold bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 hover:bg-emerald-500/20 transition-colors"
                      title="Lihat atlet pada kontingen ini"
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

                <!-- Status Logo -->
                <td class="py-3 px-4 text-center">
                  {#if item.logo}
                    <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[10px] font-mono font-medium bg-sky-500/10 text-sky-300 border border-sky-500/20">
                      <svg class="w-3 h-3 text-sky-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                      </svg>
                      <span>Ada Logo</span>
                    </span>
                  {:else}
                    <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[10px] font-mono font-medium bg-zinc-800 text-zinc-500 border border-white/[0.04]">
                      <span>Tanpa Logo</span>
                    </span>
                  {/if}
                </td>

                <!-- Aksi -->
                <td class="py-3 px-4 text-right">
                  <div class="flex items-center justify-end space-x-1.5">
                    <button
                      onclick={() => openEditModal(item)}
                      class="h-7 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] rounded-lg text-xs font-medium transition-colors flex items-center space-x-1"
                      title="Edit Nama dan Logo Kontingen"
                    >
                      <svg class="w-3 h-3 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                      </svg>
                      <span>Edit</span>
                    </button>

                    <button
                      onclick={() => openDeleteModal(item)}
                      class="h-7 px-2.5 bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/20 rounded-lg text-xs font-medium transition-colors flex items-center space-x-1"
                      title="Hapus Kontingen"
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
      <span>Menampilkan {filteredContingents.length} dari {totalContingents} kontingen</span>
      <span class="font-mono text-[11px] text-zinc-600">Realtime LAN Sync Active • Event #{eventId}</span>
    </div>
  </div>
</div>

<!-- Modal: Tambah / Edit Kontingen -->
{#if modalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-md shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <!-- Header -->
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight flex items-center space-x-2">
          <span>{isEditMode ? "Edit Kontingen / Utusan" : "Tambah Kontingen Baru"}</span>
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

        <!-- Nama Kontingen -->
        <div>
          <label for="nama_kontingen_input" class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Nama Kontingen / Utusan <span class="text-rose-400">*</span>
          </label>
          <input
            id="nama_kontingen_input"
            type="text"
            bind:value={formInput.nama_utusan}
            placeholder="Contoh: KONTINGEN KOTA SURABAYA / DOJO SAMURAI"
            class="w-full bg-[#111215] text-xs font-semibold text-white uppercase px-3 py-2.5 rounded-xl border border-white/[0.1] focus:border-purple-400 outline-none transition-colors"
          />
        </div>

        <!-- Logo Upload Section -->
        <div>
          <span class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Logo Kontingen (Ditampilkan pada Scoreboard & Bagan)
          </span>

          <div class="flex items-center space-x-3 p-3 bg-[#111215] rounded-xl border border-white/[0.08]">
            {#if formInput.new_logo_data}
              <div class="w-14 h-14 rounded-lg bg-white/[0.04] border border-white/[0.08] p-1 flex items-center justify-center shrink-0">
                <img src={formInput.new_logo_data} alt="Preview" class="w-full h-full object-contain" />
              </div>
            {:else if formInput.logo && logoMap[formInput.logo]}
              <div class="w-14 h-14 rounded-lg bg-white/[0.04] border border-white/[0.08] p-1 flex items-center justify-center shrink-0">
                <img src={logoMap[formInput.logo]} alt="Existing Logo" class="w-full h-full object-contain" />
              </div>
            {:else}
              <div class="w-14 h-14 rounded-lg bg-white/[0.03] border border-dashed border-white/20 flex flex-col items-center justify-center text-zinc-500 shrink-0">
                <svg class="w-5 h-5 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                </svg>
                <span class="text-[9px] mt-0.5">No Logo</span>
              </div>
            {/if}

            <div class="flex-1 space-y-1.5">
              <label for="contingent_logo_picker" class="cursor-pointer inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-white/[0.06] hover:bg-white/[0.12] text-xs font-semibold text-zinc-200 hover:text-white border border-white/[0.1] transition-colors">
                <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                </svg>
                <span>Pilih Gambar Logo</span>
                <input
                  id="contingent_logo_picker"
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/svg+xml"
                  onchange={handleLogoFileChange}
                  class="hidden"
                />
              </label>

              {#if formInput.new_logo_data || formInput.logo}
                <div>
                  <button
                    type="button"
                    onclick={clearLogo}
                    class="text-[11px] text-rose-400 hover:text-rose-300 font-medium"
                  >
                    Hapus Logo
                  </button>
                </div>
              {/if}
              <p class="text-[10px] text-zinc-500">PNG transparan direkomendasikan</p>
            </div>
          </div>
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
          {isSaving ? "Menyimpan..." : isEditMode ? "Simpan Perubahan" : "Simpan Kontingen"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Kontingen -->
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
          <h3 class="text-sm font-bold text-white tracking-tight">Hapus Kontingen</h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Apakah Anda yakin ingin menghapus kontingen <strong class="text-white uppercase">{itemToDelete.nama_utusan}</strong> dari database turnamen?
      </p>

      {#if (itemToDelete.athlete_count || 0) > 0}
        <div class="mt-3 p-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-300 text-[11px] leading-snug">
          ⚠️ <strong>Perhatian:</strong> Terdapat <strong>{itemToDelete.athlete_count} atlet</strong> yang bernaung di kontingen ini. Menghapus kontingen akan melepaskan afiliasi kontingen atlet tersebut (atlet tidak akan dihapus dari roster).
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
          {isDeleting ? "Menghapus..." : "Hapus Kontingen"}
        </button>
      </div>
    </div>
  </div>
{/if}
