<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { ActiveChannel, EnrichedNomorTanding, EnrichedAtletItem } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
    onNavigate?: (channel: ActiveChannel) => void;
    onOpenDrawingModal?: () => void;
    onShowToast?: (msg: string) => void;
  }

  let {
    eventId,
    eventName = "Event Turnamen",
    hostAddress,
    onNavigate,
    onOpenDrawingModal,
    onShowToast,
  }: Props = $props();

  // Data State
  let categories = $state<EnrichedNomorTanding[]>([]);
  let isLoading = $state(false);

  // Filter & Search State
  let searchKeyword = $state("");
  let selectedStatusFilter = $state<"all" | "kata" | "kumite" | "festival" | "drawn" | "undrawn" | "vr">("all");
  let selectedGenderFilter = $state<"all" | "putra" | "putri" | "campuran">("all");

  // Pagination State
  let currentPage = $state(1);
  let pageSize = $state(50);

  // Modals State
  let categoryModalOpen = $state(false);
  let isEditMode = $state(false);
  let isSaving = $state(false);
  let formError = $state<string | null>(null);

  let formInput = $state<{
    id?: number | null;
    nama_nomor_tanding: string;
    is_bob: boolean;
    has_vr: boolean;
  }>({
    nama_nomor_tanding: "",
    is_bob: false,
    has_vr: false,
  });

  let deleteModalOpen = $state(false);
  let categoryToDelete = $state<EnrichedNomorTanding | null>(null);
  let isDeleting = $state(false);

  // Athletes Viewer Modal State
  let athletesModalOpen = $state(false);
  let viewingCategory = $state<EnrichedNomorTanding | null>(null);
  let categoryAthletes = $state<EnrichedAtletItem[]>([]);
  let isLoadingAthletes = $state(false);

  // Toast Notification State
  let toastMsg = $state<string | null>(null);
  let toastTimer: any = null;

  function showToast(msg: string) {
    if (onShowToast) onShowToast(msg);
    toastMsg = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMsg = null;
    }, 3500);
  }

  async function loadData() {
    if (!eventId) return;
    isLoading = true;
    try {
      categories = await invoke<EnrichedNomorTanding[]>("get_enriched_categories", { eventId });

    } catch (err: any) {
      console.error("Gagal memuat nomor tanding:", err);
      showToast(`Gagal memuat nomor tanding: ${err?.message || err}`);
    } finally {
      isLoading = false;
    }
  }

  // Statistics derived
  let totalCount = $derived(categories.length);
  let totalAthletes = $derived(categories.reduce((acc, c) => acc + (c.athlete_count || 0), 0));
  let totalKata = $derived(
    categories.filter((c) => (c.nama_nomor_tanding || "").toUpperCase().includes("KATA")).length
  );
  let totalKumite = $derived(
    categories.filter((c) => (c.nama_nomor_tanding || "").toUpperCase().includes("KUMITE")).length
  );
  let festivalCount = $derived(
    categories.filter((c) => (c.nama_nomor_tanding || "").toUpperCase().includes("FESTIVAL")).length
  );
  let totalDrawn = $derived(categories.filter((c) => (c.bagan_count || 0) > 0).length);
  let totalUndrawn = $derived(categories.filter((c) => (c.bagan_count || 0) === 0).length);
  let totalVrActive = $derived(categories.filter((c) => c.has_vr).length);

  // Filtered categories
  let filteredCategories = $derived.by(() => {
    const q = searchKeyword.toLowerCase().trim();

    return categories.filter((c) => {
      const name = (c.nama_nomor_tanding || "").toUpperCase();

      // Status Pill Filter
      if (selectedStatusFilter === "kata" && !name.includes("KATA")) return false;
      if (selectedStatusFilter === "kumite" && !name.includes("KUMITE")) return false;
      if (selectedStatusFilter === "festival" && !name.includes("FESTIVAL")) return false;
      if (selectedStatusFilter === "drawn" && (c.bagan_count || 0) === 0) return false;
      if (selectedStatusFilter === "undrawn" && (c.bagan_count || 0) > 0) return false;
      if (selectedStatusFilter === "vr" && !c.has_vr) return false;

      // Gender Filter
      if (selectedGenderFilter === "putra" && (!name.includes("PUTRA") || name.includes("PUTRI"))) return false;
      if (selectedGenderFilter === "putri" && !name.includes("PUTRI")) return false;
      if (selectedGenderFilter === "campuran" && !name.includes("MIX") && !name.includes("CAMPURAN")) return false;

      // Search Query
      if (q) {
        const matchName = name.toLowerCase().includes(q);
        const matchId = String(c.id) === q;
        if (!matchName && !matchId) return false;
      }

      return true;
    });
  });

  // Pagination derived
  let totalPages = $derived(pageSize === -1 ? 1 : Math.ceil(filteredCategories.length / pageSize));
  let paginatedCategories = $derived.by(() => {
    if (pageSize === -1) return filteredCategories;
    const start = (currentPage - 1) * pageSize;
    return filteredCategories.slice(start, start + pageSize);
  });

  $effect(() => {
    if (searchKeyword || selectedStatusFilter || selectedGenderFilter) {
      currentPage = 1;
    }
  });

  // Single VR Toggle
  async function handleToggleVr(c: EnrichedNomorTanding) {
    const nextVal = !c.has_vr;
    try {
      await invoke("toggle_category_vr", { categoryId: c.id, hasVr: nextVal });

      categories = categories.map((item) => (item.id === c.id ? { ...item, has_vr: nextVal } : item));
      showToast(`VR ${c.nama_nomor_tanding} di${nextVal ? "aktifkan" : "nonaktifkan"}.`);
    } catch (err: any) {
      showToast(`Gagal mengubah VR: ${err?.message || err}`);
    }
  }

  // Create / Edit Modal
  function openCreateModal() {
    isEditMode = false;
    formInput = {
      nama_nomor_tanding: "",
      is_bob: false,
      has_vr: false,
    };
    formError = null;
    categoryModalOpen = true;
  }

  function openEditModal(c: EnrichedNomorTanding) {
    isEditMode = true;
    formInput = {
      id: c.id,
      nama_nomor_tanding: c.nama_nomor_tanding || "",
      is_bob: c.is_bob || false,
      has_vr: c.has_vr || false,
    };
    formError = null;
    categoryModalOpen = true;
  }

  async function handleSaveCategory() {
    const cleanName = formInput.nama_nomor_tanding.trim().toUpperCase();
    if (!cleanName) {
      formError = "Nama nomor tanding tidak boleh kosong.";
      return;
    }

    isSaving = true;
    formError = null;

    try {
      if (isEditMode && formInput.id) {
        const updated = await invoke<EnrichedNomorTanding>("update_category", {
          input: {
            id: formInput.id,
            event_id: eventId,
            nama_nomor_tanding: cleanName,
            is_bob: formInput.is_bob,
            has_vr: formInput.has_vr,
          },
        });
        categories = categories.map((c) => (c.id === updated.id ? updated : c));
        showToast(`Kategori "${cleanName}" berhasil diperbarui!`);
      } else {
        const created = await invoke<EnrichedNomorTanding>("create_category", {
          input: {
            id: null,
            event_id: eventId,
            nama_nomor_tanding: cleanName,
            is_bob: formInput.is_bob,
            has_vr: formInput.has_vr,
          },
        });
        categories = [created, ...categories];
        showToast(`Kategori baru "${cleanName}" berhasil ditambahkan!`);
      }


      categoryModalOpen = false;
    } catch (err: any) {
      formError = typeof err === "string" ? err : err?.message || "Gagal menyimpan nomor tanding.";
    } finally {
      isSaving = false;
    }
  }

  // Delete Single Modal
  function openDeleteSingleModal(c: EnrichedNomorTanding) {
    categoryToDelete = c;
    deleteModalOpen = true;
  }

  async function handleConfirmDeleteSingle() {
    if (!categoryToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_category", { categoryId: categoryToDelete.id });

      categories = categories.filter((c) => c.id !== categoryToDelete!.id);
      showToast(`Kategori "${categoryToDelete.nama_nomor_tanding}" berhasil dihapus.`);
      deleteModalOpen = false;
      categoryToDelete = null;
    } catch (err: any) {
      showToast(`Gagal menghapus kategori: ${err?.message || err}`);
    } finally {
      isDeleting = false;
    }
  }

  // Athletes Viewer Modal
  async function openAthletesModal(c: EnrichedNomorTanding) {
    viewingCategory = c;
    athletesModalOpen = true;
    isLoadingAthletes = true;

    try {
      categoryAthletes = await invoke<EnrichedAtletItem[]>("get_category_athletes", { categoryId: c.id });

    } catch (err) {
      categoryAthletes = [];
    } finally {
      isLoadingAthletes = false;
    }
  }

  function handleGoToBagan(c: EnrichedNomorTanding) {
    if (c.first_bagan_id && onNavigate) {
      onNavigate({
        type: "bagan",
        id: c.first_bagan_id,
        name: c.nama_nomor_tanding || "bagan",
        subtitle: `Bagan #${c.first_bagan_id}`,
      });
    }
  }

  function handleGoToAthletesFilter(c: EnrichedNomorTanding) {
    athletesModalOpen = false;
    if (onNavigate) {
      onNavigate({
        type: "roster",
        id: "roster",
        name: "data-atlet",
        subtitle: `Filter Kategori #${c.id}`,
      });
    }
  }

  $effect(() => {
    if (eventId) {
      loadData();
    }
  });

  onMount(() => {
    loadData();
  });

  onDestroy(() => {
    if (toastTimer) clearTimeout(toastTimer);
  });
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Toast Alert Notification -->
  {#if toastMsg}
    <div class="absolute top-4 right-6 z-50 animate-in fade-in slide-in-from-top-2 duration-150">
      <div class="bg-white text-black font-semibold text-xs px-4 py-2.5 rounded-xl shadow-2xl flex items-center space-x-2 border border-zinc-200">
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
        <span>{toastMsg}</span>
      </div>
    </div>
  {/if}

  <!-- Header Section -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4 shrink-0">
    <div>
      <div class="flex items-center space-x-2.5">
        <h2 class="text-xl font-black text-white tracking-tight uppercase">
          MANAJEMEN NOMOR TANDING
        </h2>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">
          {totalCount} KATEGORI
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1 font-normal">
        Klasifikasi kelas pertandingan, kuota atlet, dan status drawing bagan
      </p>
    </div>

    <!-- Action Buttons -->
    <div class="flex items-center space-x-2 shrink-0 self-start sm:self-auto">
      {#if onOpenDrawingModal}
        <button
          type="button"
          onclick={onOpenDrawingModal}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-50 cursor-pointer shadow-xs"
          title="Buka menu drawing bagan turnamen"
        >
          <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16m-7 6h7" />
          </svg>
          <span>Drawing Bagan</span>
        </button>
      {/if}

      <button
        type="button"
        onclick={openCreateModal}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center space-x-1.5 active:scale-95 shadow-md cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
        </svg>
        <span>Tambah Kategori</span>
      </button>
    </div>
  </div>

  <!-- Stat Cards Strip (5-Column Clean Grid) -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
    <!-- 1. Total Kategori -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">TOTAL KATEGORI</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">kelas</span>
      </div>
    </div>

    <!-- 2. Kata -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">NOMOR KATA</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalKata}</span>
        <span class="text-[10px] font-mono text-zinc-500">kategori</span>
      </div>
    </div>

    <!-- 3. Kumite -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">NOMOR KUMITE</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalKumite}</span>
        <span class="text-[10px] font-mono text-zinc-500">kategori</span>
      </div>
    </div>

    <!-- 4. Sudah Drawing -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">SUDAH DRAWING</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-emerald-400 tabular-nums">{totalDrawn}</span>
        <span class="text-[10px] font-mono text-zinc-500 tabular-nums">{totalUndrawn} belum</span>
      </div>
    </div>

    <!-- 5. Video Review (VR) -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">VIDEO REVIEW (VR)</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-purple-400 tabular-nums">{totalVrActive}</span>
        <span class="text-[10px] font-mono text-zinc-500">aktif</span>
      </div>
    </div>
  </div>

  <!-- Toolbar Filters & Search -->
  <div class="flex flex-col lg:flex-row lg:items-center justify-between gap-3 mb-3 shrink-0">
    <!-- Quick Status Pills -->
    <div class="flex flex-wrap items-center gap-1.5 overflow-x-auto py-0.5">
      <button
        onclick={() => (selectedStatusFilter = 'all')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'all'
          ? 'bg-white text-black border-white shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Semua ({totalCount})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'kata')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'kata'
          ? 'bg-white text-black border-white shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Kata ({totalKata})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'kumite')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'kumite'
          ? 'bg-white text-black border-white shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Kumite ({totalKumite})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'festival')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'festival'
          ? 'bg-amber-400 text-black border-amber-400 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Festival ({festivalCount})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'drawn')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'drawn'
          ? 'bg-emerald-400 text-black border-emerald-400 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Sudah Drawing ({totalDrawn})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'undrawn')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'undrawn'
          ? 'bg-zinc-700 text-white border-zinc-600 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Belum Drawing ({totalUndrawn})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'vr')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'vr'
          ? 'bg-purple-400 text-black border-purple-400 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        VR Aktif ({totalVrActive})
      </button>
    </div>

    <!-- Dropdown Filters & Search Bar -->
    <div class="flex items-center space-x-2 shrink-0">
      <!-- Filter Gender -->
      <select
        bind:value={selectedGenderFilter}
        class="bg-[#181a20] text-xs text-zinc-300 px-3 py-1.5 rounded-xl border border-white/[0.1] focus:border-white outline-none cursor-pointer"
      >
        <option value="all">Semua Gender</option>
        <option value="putra">Putra</option>
        <option value="putri">Putri</option>
        <option value="campuran">Mix / Campuran</option>
      </select>

      <!-- Search Input -->
      <div class="relative w-56">
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari kategori, umur, kelas..."
          class="w-full bg-[#181a20] text-xs text-white pl-8 pr-7 py-1.5 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors placeholder-zinc-500"
        />
        <svg class="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
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

  <!-- Table Container (Clean 6-Column Layout, No Checkboxes) -->
  <div class="flex-1 bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden flex flex-col shadow-inner min-h-0">
    {#if isLoading && categories.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
        <svg class="w-8 h-8 animate-spin text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <p class="text-xs text-zinc-400 font-mono">Memuat data nomor tanding...</p>
      </div>
    {:else if filteredCategories.length === 0}
      <!-- Empty State -->
      <div class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-zinc-500">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 6h16M4 12h16m-7 6h7" />
          </svg>
        </div>
        <p class="text-sm font-bold text-white">Tidak Ada Nomor Tanding Ditemukan</p>
        <p class="text-xs text-zinc-500 max-w-sm text-center">
          {searchKeyword ? "Coba gunakan kata kunci pencarian lain atau reset filter status." : "Belum ada kategori yang terdaftar. Buat kategori baru secara manual."}
        </p>
        {#if searchKeyword || selectedStatusFilter !== 'all' || selectedGenderFilter !== 'all'}
          <button
            onclick={() => {
              searchKeyword = "";
              selectedStatusFilter = "all";
              selectedGenderFilter = "all";
            }}
            class="h-8 px-3 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-xs font-semibold text-zinc-300 hover:text-white transition-colors cursor-pointer"
          >
            Reset Semua Filter
          </button>
        {/if}
      </div>
    {:else}
      <div class="flex-1 overflow-y-auto min-h-0">
        <table class="w-full text-left border-collapse text-xs">
          <thead class="bg-[#111215] sticky top-0 z-10 border-b border-white/[0.08] text-zinc-400 uppercase tracking-wider text-[10px] font-mono">
            <tr>
              <th class="w-12 px-3 py-3 text-center font-bold">NO</th>
              <th class="px-4 py-3 font-bold">NOMOR TANDING / KELAS PERTANDINGAN</th>
              <th class="w-36 px-4 py-3 text-center font-bold">PESERTA TERDAFTAR</th>
              <th class="w-40 px-4 py-3 text-center font-bold">STATUS BAGAN</th>
              <th class="w-32 px-3 py-3 text-center font-bold">VIDEO REVIEW</th>
              <th class="w-24 px-4 py-3 text-center pr-6 font-bold">AKSI</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.04]">
            {#each paginatedCategories as cat, idx (cat.id)}
              {@const upperName = (cat.nama_nomor_tanding || "").toUpperCase()}
              {@const isKata = upperName.includes("KATA")}
              {@const isKumite = upperName.includes("KUMITE")}
              {@const isFestival = upperName.includes("FESTIVAL")}

              <tr class="hover:bg-white/[0.02] transition-colors">
                <!-- Number -->
                <td class="px-3 py-3 text-center font-mono text-zinc-500 text-[11px] tabular-nums">
                  {(currentPage - 1) * pageSize + idx + 1}
                </td>

                <!-- Category Name & Identification -->
                <td class="px-4 py-3">
                  <div class="flex items-center space-x-2">
                    <span class="font-bold text-white text-xs uppercase tracking-tight">
                      {cat.nama_nomor_tanding || `[#${cat.id}] Tanpa Nama`}
                    </span>
                    {#if cat.is_bob}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-purple-500/10 text-purple-300 border border-purple-500/20">
                        BOB
                      </span>
                    {/if}
                  </div>

                  <div class="flex items-center space-x-1.5 mt-1">
                    {#if isKata}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.08]">
                        KATA
                      </span>
                    {:else if isKumite}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.08]">
                        KUMITE
                      </span>
                    {/if}

                    {#if isFestival}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/20">
                        FESTIVAL
                      </span>
                    {:else}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.04] text-zinc-400 border border-white/[0.06]">
                        PRESTASI
                      </span>
                    {/if}

                    <span class="font-mono text-[10px] text-zinc-500 tabular-nums">
                      ID #{cat.id}
                    </span>
                  </div>
                </td>

                <!-- Athlete Count (Clickable Viewer) -->
                <td class="px-4 py-3 text-center">
                  <button
                    type="button"
                    onclick={() => openAthletesModal(cat)}
                    class="h-6 inline-flex items-center space-x-1.5 px-2.5 rounded-lg text-[10px] font-mono font-bold transition-all border active:scale-95 cursor-pointer {cat.athlete_count > 0 ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/20 hover:bg-emerald-500/20' : 'bg-white/[0.04] text-zinc-500 border-white/[0.08] hover:text-zinc-300'}"
                    title="Klik untuk melihat daftar atlet terdaftar"
                  >
                    <svg class="w-3 h-3 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                    </svg>
                    <span class="tabular-nums">{cat.athlete_count} ATLET</span>
                  </button>
                </td>

                <!-- Bagan Status -->
                <td class="px-4 py-3 text-center">
                  {#if (cat.bagan_count || 0) > 0}
                    <div class="inline-flex items-center space-x-1.5">
                      <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded text-[10px] font-mono font-bold border bg-emerald-500/10 text-emerald-400 border-emerald-500/20">
                        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                        <span class="tabular-nums">{cat.bagan_count} BAGAN</span>
                      </span>
                      {#if cat.first_bagan_id && onNavigate}
                        <button
                          type="button"
                          onclick={() => handleGoToBagan(cat)}
                          class="p-1 text-zinc-400 hover:text-emerald-400 rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer active:scale-95"
                          title="Buka bagan pertandingan"
                        >
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                          </svg>
                        </button>
                      {/if}
                    </div>
                  {:else}
                    <div class="inline-flex items-center space-x-1.5">
                      <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold border bg-white/[0.04] text-zinc-500 border-white/[0.08]">
                        BELUM DRAWING
                      </span>
                      {#if onOpenDrawingModal}
                        <button
                          type="button"
                          onclick={onOpenDrawingModal}
                          class="p-1 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition-colors cursor-pointer active:scale-95"
                          title="Drawing bagan"
                        >
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" />
                          </svg>
                        </button>
                      {/if}
                    </div>
                  {/if}
                </td>

                <!-- Video Review (VR) Toggle -->
                <td class="px-3 py-3 text-center">
                  <button
                    type="button"
                    onclick={() => handleToggleVr(cat)}
                    class="h-6 px-2.5 rounded-lg text-[10px] font-mono font-bold transition-all border active:scale-95 cursor-pointer {cat.has_vr ? 'bg-purple-500/15 text-purple-300 border-purple-500/30 hover:bg-purple-500/25 shadow-xs' : 'bg-white/[0.04] text-zinc-500 border-white/[0.08] hover:text-zinc-300'}"
                    title="Klik untuk mengubah status Video Review (VR)"
                  >
                    {cat.has_vr ? "VR AKTIF" : "NON-VR"}
                  </button>
                </td>

                <!-- Actions (Dedicated Edit & Delete Buttons) -->
                <td class="w-24 px-4 py-3 text-center pr-6">
                  <div class="flex items-center justify-center space-x-1.5">
                    <!-- Edit Button -->
                    <button
                      onclick={() => openEditModal(cat)}
                      class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-sky-500/15 text-zinc-400 hover:text-sky-300 border border-white/[0.08] hover:border-sky-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                      title={`Ubah kategori ${cat.nama_nomor_tanding}`}
                      aria-label={`Ubah kategori ${cat.nama_nomor_tanding}`}
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125" />
                      </svg>
                    </button>

                    <!-- Delete Button -->
                    <button
                      onclick={() => openDeleteSingleModal(cat)}
                      class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-rose-500/15 text-zinc-400 hover:text-rose-400 border border-white/[0.08] hover:border-rose-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                      title={`Hapus ${cat.nama_nomor_tanding}`}
                      aria-label={`Hapus ${cat.nama_nomor_tanding}`}
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                      </svg>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <!-- Pagination / Footer Info -->
      <div class="bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0 font-sans">
        <div class="flex items-center space-x-2 tabular-nums">
          <span>Menampilkan {paginatedCategories.length > 0 ? (currentPage - 1) * pageSize + 1 : 0} - {Math.min(currentPage * pageSize, filteredCategories.length)} dari {filteredCategories.length} kategori</span>
          <span>•</span>
          <div class="flex items-center space-x-1.5">
            <span>Per baris:</span>
            <select
              bind:value={pageSize}
              onchange={() => (currentPage = 1)}
              class="bg-[#181a20] text-zinc-300 text-xs px-2 py-0.5 rounded-lg border border-white/[0.1] outline-none focus:border-white cursor-pointer"
            >
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
              <option value={-1}>Semua</option>
            </select>
          </div>
        </div>

        {#if pageSize !== -1 && totalPages > 1}
          <div class="flex items-center space-x-1 font-mono">
            <button
              disabled={currentPage <= 1}
              onclick={() => (currentPage = Math.max(1, currentPage - 1))}
              class="h-7 px-2.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:pointer-events-none transition-colors cursor-pointer"
            >
              ‹
            </button>
            <span class="px-2 text-zinc-400 tabular-nums">{currentPage} / {totalPages}</span>
            <button
              disabled={currentPage >= totalPages}
              onclick={() => (currentPage = Math.min(totalPages, currentPage + 1))}
              class="h-7 px-2.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:pointer-events-none transition-colors cursor-pointer"
            >
              ›
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- Modal: Tambah / Edit Kategori -->
{#if categoryModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl max-w-lg w-full p-6 shadow-2xl relative animate-in zoom-in-95 duration-150">
      <div class="flex items-center justify-between pb-3.5 border-b border-white/[0.08] mb-4">
        <h3 class="text-sm font-bold text-white tracking-tight">
          {isEditMode ? "Edit Nomor Tanding" : "Tambah Nomor Tanding Baru"}
        </h3>
        <button
          onclick={() => (categoryModalOpen = false)}
          aria-label="Tutup modal"
          title="Tutup"
          class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
        >
          ✕
        </button>
      </div>

      {#if formError}
        <div class="mb-4 p-3 bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs rounded-xl">
          {formError}
        </div>
      {/if}

      <div class="space-y-4">
        <div>
          <label for="nama_cat" class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Nama Nomor Tanding / Kelas <span class="text-rose-400">*</span>
          </label>
          <input
            id="nama_cat"
            type="text"
            bind:value={formInput.nama_nomor_tanding}
            placeholder="Contoh: TOURNAMENT CADET - KATA PERORANGAN PUTRA"
            class="w-full bg-[#111215] text-xs font-semibold text-white uppercase px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors"
          />
        </div>

        <!-- Quick Insert Snippets -->
        <div>
          <span class="block text-[10px] font-bold text-zinc-400 uppercase tracking-wider mb-1.5">
            Pintasan Teks Nama Kelas
          </span>
          <div class="flex flex-wrap gap-1.5">
            {#each ["TOURNAMENT", "FESTIVAL", "KATA PERORANGAN", "KUMITE", "KATA BEREGU", "PUTRA", "PUTRI", "MIX"] as tag}
              <button
                type="button"
                onclick={() => {
                  if (!formInput.nama_nomor_tanding.includes(tag)) {
                    formInput.nama_nomor_tanding = formInput.nama_nomor_tanding
                      ? `${formInput.nama_nomor_tanding} - ${tag}`
                      : tag;
                  }
                }}
                class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] text-[10px] font-mono font-semibold transition-colors cursor-pointer active:scale-95"
              >
                + {tag}
              </button>
            {/each}
          </div>
        </div>

        <!-- Options Checkboxes -->
        <div class="pt-2 border-t border-white/[0.08] grid grid-cols-2 gap-3">
          <label class="flex items-center space-x-2.5 p-3 rounded-xl bg-[#111215] border border-white/[0.08] cursor-pointer hover:border-white/20 transition-colors">
            <input
              type="checkbox"
              bind:checked={formInput.has_vr}
              class="rounded border-white/20 bg-black/40 text-white focus:ring-0 w-4 h-4 cursor-pointer"
            />
            <span class="font-bold text-white text-xs">Video Review (VR)</span>
          </label>

          <label class="flex items-center space-x-2.5 p-3 rounded-xl bg-[#111215] border border-white/[0.08] cursor-pointer hover:border-white/20 transition-colors">
            <input
              type="checkbox"
              bind:checked={formInput.is_bob}
              class="rounded border-white/20 bg-black/40 text-white focus:ring-0 w-4 h-4 cursor-pointer"
            />
            <span class="font-bold text-white text-xs">Best of the Best (BOB)</span>
          </label>
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2 mt-6 pt-3.5 border-t border-white/[0.08]">
        <button
          onclick={() => (categoryModalOpen = false)}
          disabled={isSaving}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
        >
          Batal
        </button>
        <button
          onclick={handleSaveCategory}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          <span>{isSaving ? "Menyimpan..." : isEditMode ? "Simpan Perubahan" : "Simpan Kategori"}</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Single -->
{#if deleteModalOpen && categoryToDelete}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl max-w-sm w-full p-6 shadow-2xl animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 mb-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">Hapus Nomor Tanding</h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 leading-relaxed mb-4">
        Apakah Anda yakin ingin menghapus kategori <span class="text-white font-bold uppercase">{categoryToDelete.nama_nomor_tanding}</span>?
      </p>

      {#if categoryToDelete.athlete_count > 0}
        <div class="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 text-amber-300 text-xs rounded-xl leading-relaxed">
          <strong>Catatan:</strong> {categoryToDelete.athlete_count} atlet terdaftar akan dilepaskan (unassigned) dan tidak akan terhapus dari data atlet.
        </div>
      {/if}

      <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          onclick={() => (deleteModalOpen = false)}
          disabled={isDeleting}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
        >
          Batal
        </button>
        <button
          onclick={handleConfirmDeleteSingle}
          disabled={isDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl shadow-md transition-colors flex items-center space-x-1 active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          <span>{isDeleting ? "Menghapus..." : "Hapus Kategori"}</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Daftar Peserta Terdaftar (Perguruan DULU, baru Kontingen) -->
{#if athletesModalOpen && viewingCategory}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl max-w-2xl w-full max-h-[85vh] p-6 shadow-2xl flex flex-col animate-in zoom-in-95 duration-150">
      <div class="flex items-center justify-between pb-3.5 border-b border-white/[0.08] shrink-0 mb-4">
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">
            Peserta Terdaftar
          </h3>
          <p class="text-xs text-zinc-400 mt-0.5 uppercase font-semibold">
            {viewingCategory.nama_nomor_tanding}
          </p>
        </div>
        <button
          onclick={() => (athletesModalOpen = false)}
          aria-label="Tutup modal"
          title="Tutup"
          class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
        >
          ✕
        </button>
      </div>

      <div class="flex-1 overflow-y-auto min-h-0 rounded-xl border border-white/[0.08] bg-[#111215]">
        {#if isLoadingAthletes}
          <div class="py-12 flex flex-col items-center justify-center text-zinc-500 space-y-2">
            <svg class="w-5 h-5 animate-spin text-zinc-400" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <span class="text-xs font-mono text-zinc-400">Memuat atlet...</span>
          </div>
        {:else if categoryAthletes.length === 0}
          <div class="py-12 text-center text-zinc-500 text-xs">
            Belum ada atlet yang terdaftar di kategori ini.
          </div>
        {:else}
          <table class="w-full text-left text-xs border-collapse">
            <thead class="bg-[#111215] text-zinc-400 font-mono text-[10px] uppercase tracking-wider sticky top-0 border-b border-white/[0.08]">
              <tr>
                <th class="py-2.5 px-3 w-10 text-center font-bold">NO</th>
                <th class="py-2.5 px-3 font-bold">NAMA ATLET</th>
                <th class="py-2.5 px-3 font-bold">PERGURUAN</th>
                <th class="py-2.5 px-3 font-bold">KONTINGEN</th>
                <th class="py-2.5 px-3 text-center w-24 font-bold">SEED</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-white/[0.04] font-medium">
              {#each categoryAthletes as atlet, aIdx (atlet.id)}
                <tr class="hover:bg-white/[0.02] transition-colors">
                  <td class="py-2.5 px-3 text-center font-mono text-zinc-500 text-[11px] tabular-nums">
                    {aIdx + 1}
                  </td>
                  <td class="py-2.5 px-3 font-bold text-white uppercase text-xs">
                    {atlet.nama_atlet || "Tanpa Nama"}
                  </td>
                  <!-- Perguruan First -->
                  <td class="py-2.5 px-3 text-xs">
                    {#if atlet.perguruan_nama}
                      <span class="inline-flex items-center space-x-1 text-[10px] font-mono font-semibold text-amber-300 bg-amber-500/10 px-1.5 py-0.5 rounded border border-amber-500/20">
                        <span class="w-1 h-1 rounded-full bg-amber-400"></span>
                        <span>{atlet.perguruan_nama}</span>
                      </span>
                    {:else}
                      <span class="text-zinc-500 font-mono">—</span>
                    {/if}
                  </td>
                  <!-- Kontingen Then -->
                  <td class="py-2.5 px-3 text-zinc-300 uppercase text-xs">
                    {atlet.utusan_nama || "—"}
                  </td>
                  <td class="py-2.5 px-3 text-center">
                    {#if atlet.is_priority}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/20">
                        SEED
                      </span>
                    {:else}
                      <span class="text-zinc-600 font-mono">—</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>

      <div class="flex items-center justify-between pt-3.5 border-t border-white/[0.08] shrink-0 mt-4 text-xs">
        <span class="text-zinc-500 font-mono font-semibold tabular-nums">
          Total: {categoryAthletes.length} Atlet
        </span>
        <div class="flex items-center space-x-2">
          {#if onNavigate}
            <button
              type="button"
              onclick={() => viewingCategory && handleGoToAthletesFilter(viewingCategory)}
              class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors cursor-pointer active:scale-95"
            >
              Buka di Data Atlet
            </button>
          {/if}
          <button
            type="button"
            onclick={() => (athletesModalOpen = false)}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors cursor-pointer active:scale-95"
          >
            Tutup
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
