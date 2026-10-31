<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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

  // Selection & Bulk State
  let selectedIds = $state<Set<number>>(new Set());

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

  let batchDeleteModalOpen = $state(false);
  let isBatchDeleting = $state(false);

  // Athletes Viewer Modal State
  let athletesModalOpen = $state(false);
  let viewingCategory = $state<EnrichedNomorTanding | null>(null);
  let categoryAthletes = $state<EnrichedAtletItem[]>([]);
  let isLoadingAthletes = $state(false);

  // Toast Notification State
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

  async function loadData() {
    isLoading = true;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        categories = await invoke<EnrichedNomorTanding[]>("get_enriched_categories", { eventId });
      } else {
        categories = getMockCategories();
      }
    } catch (err: any) {
      console.warn("get_enriched_categories failed, loading fallback mock:", err);
      categories = getMockCategories();
    } finally {
      isLoading = false;
    }
  }

  function getMockCategories(): EnrichedNomorTanding[] {
    return [
      {
        id: 1,
        event_id: eventId,
        nama_nomor_tanding: "TOURNAMENT PRA USIA DINI - KATA PERORANGAN PUTRI",
        is_bob: false,
        has_vr: false,
        athlete_count: 8,
        bagan_count: 1,
        first_bagan_id: 101,
      },
      {
        id: 2,
        event_id: eventId,
        nama_nomor_tanding: "TOURNAMENT PRA USIA DINI - KUMITE -25 KG PUTRA",
        is_bob: false,
        has_vr: true,
        athlete_count: 14,
        bagan_count: 1,
        first_bagan_id: 102,
      },
      {
        id: 3,
        event_id: eventId,
        nama_nomor_tanding: "FESTIVAL USIA DINI - KATA PERORANGAN PUTRA",
        is_bob: false,
        has_vr: false,
        athlete_count: 18,
        bagan_count: 2,
        first_bagan_id: 103,
      },
      {
        id: 4,
        event_id: eventId,
        nama_nomor_tanding: "TOURNAMENT PEMULA - KATA PERORANGAN PUTRI",
        is_bob: false,
        has_vr: false,
        athlete_count: 22,
        bagan_count: 3,
        first_bagan_id: 104,
      },
      {
        id: 5,
        event_id: eventId,
        nama_nomor_tanding: "TOURNAMENT KADET - KUMITE -52 KG PUTRA",
        is_bob: false,
        has_vr: true,
        athlete_count: 16,
        bagan_count: 1,
        first_bagan_id: 105,
      },
      {
        id: 6,
        event_id: eventId,
        nama_nomor_tanding: "TOURNAMENT SENIOR - KATA BEREGU PUTRA",
        is_bob: false,
        has_vr: false,
        athlete_count: 6,
        bagan_count: 0,
        first_bagan_id: null,
      },
      {
        id: 7,
        event_id: eventId,
        nama_nomor_tanding: "BEST OF THE BEST - KUMITE SENIOR PUTRA",
        is_bob: true,
        has_vr: true,
        athlete_count: 8,
        bagan_count: 1,
        first_bagan_id: 106,
      },
      {
        id: 8,
        event_id: eventId,
        nama_nomor_tanding: "TOURNAMENT JUNIOR - KUMITE +68 KG PUTRI",
        is_bob: false,
        has_vr: false,
        athlete_count: 0,
        bagan_count: 0,
        first_bagan_id: null,
      },
    ];
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

  // Checkbox selection
  function toggleSelectAll() {
    if (paginatedCategories.length === 0) return;
    const allSelected = paginatedCategories.every((c) => selectedIds.has(c.id));
    if (allSelected) {
      for (const c of paginatedCategories) {
        selectedIds.delete(c.id);
      }
    } else {
      for (const c of paginatedCategories) {
        selectedIds.add(c.id);
      }
    }
    selectedIds = new Set(selectedIds);
  }

  function toggleSelectCategory(id: number) {
    if (selectedIds.has(id)) {
      selectedIds.delete(id);
    } else {
      selectedIds.add(id);
    }
    selectedIds = new Set(selectedIds);
  }

  function clearSelection() {
    selectedIds.clear();
    selectedIds = new Set(selectedIds);
  }

  // Single VR Toggle
  async function handleToggleVr(c: EnrichedNomorTanding) {
    const nextVal = !c.has_vr;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        await invoke("toggle_category_vr", { category_id: c.id, has_vr: nextVal });
      }
      categories = categories.map((item) => (item.id === c.id ? { ...item, has_vr: nextVal } : item));
      showToast(`VR ${c.nama_nomor_tanding} di${nextVal ? "aktifkan" : "nonaktifkan"}.`);
    } catch (err: any) {
      showToast(`Gagal mengubah VR: ${err?.message || err}`);
    }
  }

  // Batch VR Toggle
  async function handleBatchSetVr(enable: boolean) {
    if (selectedIds.size === 0) return;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        for (const id of selectedIds) {
          await invoke("toggle_category_vr", { category_id: id, has_vr: enable }).catch(() => {});
        }
      }
      categories = categories.map((c) => (selectedIds.has(c.id) ? { ...c, has_vr: enable } : c));
      showToast(`Berhasil mengubah status VR untuk ${selectedIds.size} kategori!`);
      clearSelection();
    } catch (err: any) {
      showToast(`Gagal mengubah batch VR: ${err?.message || err}`);
    }
  }

  // Batch Delete
  async function handleBatchDelete() {
    if (selectedIds.size === 0) return;
    isBatchDeleting = true;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        for (const id of selectedIds) {
          await invoke("delete_category", { category_id: id }).catch(() => {});
        }
      }
      categories = categories.filter((c) => !selectedIds.has(c.id));
      showToast(`Berhasil menghapus ${selectedIds.size} kategori terpilih.`);
      clearSelection();
      batchDeleteModalOpen = false;
    } catch (err: any) {
      showToast(`Gagal menghapus kategori batch: ${err?.message || err}`);
    } finally {
      isBatchDeleting = false;
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
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
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
      } else {
        // Fallback for dev preview
        if (isEditMode && formInput.id) {
          categories = categories.map((c) =>
            c.id === formInput.id
              ? { ...c, nama_nomor_tanding: cleanName, is_bob: formInput.is_bob, has_vr: formInput.has_vr }
              : c
          );
        } else {
          const newId = Math.max(0, ...categories.map((c) => c.id)) + 1;
          categories = [
            {
              id: newId,
              event_id: eventId,
              nama_nomor_tanding: cleanName,
              is_bob: formInput.is_bob,
              has_vr: formInput.has_vr,
              athlete_count: 0,
              bagan_count: 0,
              first_bagan_id: null,
            },
            ...categories,
          ];
        }
        showToast(`Kategori "${cleanName}" berhasil disimpan!`);
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
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        await invoke("delete_category", { category_id: categoryToDelete.id });
      }
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
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        categoryAthletes = await invoke<EnrichedAtletItem[]>("get_category_athletes", { category_id: c.id });
      } else {
        categoryAthletes = [
          {
            id: 101,
            event_id: eventId,
            nama_atlet: "MUHAMMAD ADRIAN PUTRA",
            nik: "6472010101010001",
            kode_atlet: "ATL-001",
            additional_code: null,
            is_priority: true,
            perguruan_id: 1,
            perguruan_nama: "INKANAS",
            utusan_id: 1,
            utusan_nama: "DOJO ZERO 5",
            utusan_logo: null,
            nomor_tanding_id: c.id,
            nomor_tanding_nama: c.nama_nomor_tanding,
            nomor_tanding_tipe: "Tournament",
            is_bob: c.is_bob,
          },
          {
            id: 102,
            event_id: eventId,
            nama_atlet: "KYLIAN MALEAKHI JOSE",
            nik: "6472010101010002",
            kode_atlet: "ATL-002",
            additional_code: null,
            is_priority: false,
            perguruan_id: 2,
            perguruan_nama: "SHOKAIDO",
            utusan_id: 2,
            utusan_nama: "DOJO COMBAT",
            utusan_logo: null,
            nomor_tanding_id: c.id,
            nomor_tanding_nama: c.nama_nomor_tanding,
            nomor_tanding_tipe: "Tournament",
            is_bob: c.is_bob,
          },
        ];
      }
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
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Toast Alert Notification -->
  {#if toastMsg}
    <div class="absolute top-4 right-6 z-50 bg-[#181a20] border border-white/20 text-white text-xs font-semibold px-4 py-2.5 rounded-xl shadow-2xl flex items-center space-x-2 transition-all">
      <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
      <span>{toastMsg}</span>
    </div>
  {/if}

  <!-- Header Section -->
  <div class="flex items-center justify-between pb-4 border-b border-white/[0.08] shrink-0">
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
    <div class="flex items-center space-x-2">
      {#if onOpenDrawingModal}
        <button
          onclick={onOpenDrawingModal}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-50"
          title="Buka menu drawing bagan turnamen"
        >
          <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16m-7 6h7" />
          </svg>
          <span>Drawing Bagan</span>
        </button>
      {/if}

      <button
        onclick={loadData}
        disabled={isLoading}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-50"
        title="Segarkan data kategori"
      >
        <svg
          class="w-3.5 h-3.5 text-zinc-400 {isLoading ? 'animate-spin' : ''}"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
      </button>

      <button
        onclick={openCreateModal}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center space-x-1.5 active:scale-95 shadow-md"
      >
        <svg class="w-3.5 h-3.5 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
        </svg>
        <span>+ Tambah Kategori</span>
      </button>
    </div>
  </div>

  <!-- Stat Cards Strip -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
    <!-- Total Kategori -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Total Kategori</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">kelas</span>
      </div>
    </div>

    <!-- Kata -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Kata</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalKata}</span>
        <span class="text-[10px] font-mono text-zinc-500">kategori</span>
      </div>
    </div>

    <!-- Kumite -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Kumite</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalKumite}</span>
        <span class="text-[10px] font-mono text-zinc-500">kategori</span>
      </div>
    </div>

    <!-- Sudah Drawing -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <div class="flex items-center justify-between">
        <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Sudah Drawing</span>
        {#if totalDrawn > 0}
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
        {/if}
      </div>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalDrawn}</span>
        <span class="text-[10px] font-mono text-zinc-500 tabular-nums">{totalUndrawn} belum</span>
      </div>
    </div>

    <!-- Video Review (VR) -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <div class="flex items-center justify-between">
        <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Video Review (VR)</span>
        {#if totalVrActive > 0}
          <span class="w-1.5 h-1.5 rounded-full bg-purple-400"></span>
        {/if}
      </div>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalVrActive}</span>
        <span class="text-[10px] font-mono text-zinc-500">aktif</span>
      </div>
    </div>
  </div>

  <!-- Toolbar Filters & Search -->
  <div class="flex flex-col lg:flex-row lg:items-center justify-between gap-3 mb-3 shrink-0">
    <!-- Quick Status Pills -->
    <div class="flex items-center space-x-1.5 overflow-x-auto pb-1 lg:pb-0 scrollbar-none">
      <button
        onclick={() => (selectedStatusFilter = 'all')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'all' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Semua ({totalCount})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'kata')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'kata' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Kata ({totalKata})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'kumite')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'kumite' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Kumite ({totalKumite})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'festival')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'festival' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Festival ({festivalCount})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'drawn')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'drawn' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Sudah Drawing ({totalDrawn})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'undrawn')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'undrawn' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Belum Drawing ({totalUndrawn})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'vr')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'vr' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
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
            class="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-white text-xs"
          >
            ✕
          </button>
        {/if}
      </div>
    </div>
  </div>

  <!-- Bulk Actions Bar (When Items Selected) -->
  {#if selectedIds.size > 0}
    <div class="bg-[#181a20] border border-white/20 rounded-xl px-4 py-2 mb-3 flex items-center justify-between shadow-lg shrink-0">
      <div class="flex items-center space-x-2">
        <span class="w-1.5 h-1.5 rounded-full bg-white"></span>
        <span class="text-xs font-bold text-white font-mono tabular-nums">{selectedIds.size} kategori terpilih</span>
      </div>

      <div class="flex items-center space-x-2">
        <button
          onclick={() => handleBatchSetVr(true)}
          class="h-7 px-3 bg-white/[0.06] hover:bg-white/[0.12] text-zinc-200 text-xs font-semibold rounded-lg border border-white/[0.1] transition-colors active:scale-95"
        >
          Set VR Aktif
        </button>
        <button
          onclick={() => handleBatchSetVr(false)}
          class="h-7 px-3 bg-white/[0.06] hover:bg-white/[0.12] text-zinc-200 text-xs font-semibold rounded-lg border border-white/[0.1] transition-colors active:scale-95"
        >
          Set Non-VR
        </button>
        <button
          onclick={() => (batchDeleteModalOpen = true)}
          class="h-7 px-3 bg-rose-600 hover:bg-rose-500 text-white text-xs font-bold rounded-lg transition-colors active:scale-95 shadow-sm"
        >
          <span>Hapus Terpilih ({selectedIds.size})</span>
        </button>
        <button
          onclick={clearSelection}
          class="h-7 px-2.5 text-zinc-400 hover:text-white text-xs font-semibold transition-colors"
        >
          Batal
        </button>
      </div>
    </div>
  {/if}

  <!-- Main Content Table -->
  <div class="flex-1 overflow-y-auto min-h-0 rounded-2xl border border-white/[0.08] bg-[#181a20] shadow-lg flex flex-col">
    {#if isLoading}
      <div class="flex-1 flex flex-col items-center justify-center text-zinc-500 text-sm space-y-2">
        <svg class="w-6 h-6 animate-spin text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span class="text-xs font-mono text-zinc-400">Memuat data nomor tanding dari database...</span>
      </div>
    {:else if filteredCategories.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center text-center p-8">
        <p class="text-sm font-bold text-zinc-300">Tidak ada nomor tanding yang sesuai</p>
        <p class="text-xs text-zinc-500 mt-1 max-w-sm font-normal">
          {searchKeyword ? "Coba gunakan kata kunci pencarian lain atau reset filter status." : "Belum ada kategori yang terdaftar. Buat kategori baru secara manual."}
        </p>
        {#if searchKeyword || selectedStatusFilter !== 'all' || selectedGenderFilter !== 'all'}
          <button
            onclick={() => {
              searchKeyword = "";
              selectedStatusFilter = "all";
              selectedGenderFilter = "all";
            }}
            class="mt-3 h-8 px-3.5 bg-white/[0.06] hover:bg-white/[0.1] text-xs font-semibold rounded-xl border border-white/[0.1] text-zinc-200 transition-colors"
          >
            Reset Semua Filter
          </button>
        {/if}
      </div>
    {:else}
      <div class="overflow-x-auto flex-1">
        <table class="w-full text-left text-xs border-collapse">
          <thead class="bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider sticky top-0 z-10 border-b border-white/[0.08]">
            <tr>
              <th class="px-4 py-3 w-10 text-center">
                <input
                  type="checkbox"
                  aria-label="Pilih semua kategori di halaman ini"
                  checked={paginatedCategories.length > 0 && paginatedCategories.every((c) => selectedIds.has(c.id))}
                  onchange={toggleSelectAll}
                  class="rounded border-white/20 bg-black/40 text-white focus:ring-0 cursor-pointer"
                />
              </th>
              <th class="px-3 py-3 w-12 text-center font-bold">NO</th>
              <th class="px-4 py-3 font-bold">NOMOR TANDING / KELAS PERTANDINGAN</th>
              <th class="px-4 py-3 text-center w-36 font-bold">PESERTA TERDAFTAR</th>
              <th class="px-4 py-3 text-center w-40 font-bold">STATUS BAGAN</th>
              <th class="px-3 py-3 text-center w-32 font-bold">VIDEO REVIEW</th>
              <th class="px-4 py-3 text-right w-28 font-bold pr-5">AKSI</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.04] font-medium">
            {#each paginatedCategories as cat, idx (cat.id)}
              {@const upperName = (cat.nama_nomor_tanding || "").toUpperCase()}
              {@const isKata = upperName.includes("KATA")}
              {@const isKumite = upperName.includes("KUMITE")}
              {@const isFestival = upperName.includes("FESTIVAL")}

              <tr class="hover:bg-white/[0.02] transition-colors {selectedIds.has(cat.id) ? 'bg-white/[0.04]' : ''}">
                <!-- Checkbox -->
                <td class="px-4 py-2.5 text-center">
                  <input
                    type="checkbox"
                    aria-label={`Pilih kategori ${cat.nama_nomor_tanding}`}
                    checked={selectedIds.has(cat.id)}
                    onchange={() => toggleSelectCategory(cat.id)}
                    class="rounded border-white/20 bg-black/40 text-white focus:ring-0 cursor-pointer"
                  />
                </td>

                <!-- Number -->
                <td class="px-3 py-2.5 text-center text-zinc-500 font-mono text-[11px] tabular-nums">
                  {(currentPage - 1) * pageSize + idx + 1}
                </td>

                <!-- Category Name & Identification -->
                <td class="px-4 py-2.5">
                  <div class="flex items-center space-x-2">
                    <span class="font-bold text-white text-[12.5px] uppercase tracking-tight">
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
                <td class="px-4 py-2.5 text-center">
                  <button
                    type="button"
                    onclick={() => openAthletesModal(cat)}
                    class="h-6 inline-flex items-center space-x-1.5 px-2.5 rounded text-[10px] font-mono font-bold transition-colors border {cat.athlete_count > 0 ? 'bg-white/[0.06] hover:bg-white/[0.1] text-zinc-200 border-white/[0.1]' : 'bg-white/[0.04] text-zinc-500 border-white/[0.08] hover:text-zinc-300'}"
                    title="Klik untuk melihat daftar atlet terdaftar"
                  >
                    <svg class="w-3 h-3 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                    </svg>
                    <span class="tabular-nums">{cat.athlete_count} ATLET</span>
                  </button>
                </td>

                <!-- Bagan Status -->
                <td class="px-4 py-2.5 text-center">
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
                          class="p-1 text-zinc-400 hover:text-emerald-400 rounded-lg hover:bg-white/[0.08] transition-colors"
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
                          class="p-1 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition-colors"
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
                <td class="px-3 py-2.5 text-center">
                  <button
                    type="button"
                    onclick={() => handleToggleVr(cat)}
                    class="h-6 px-2.5 rounded text-[10px] font-mono font-bold transition-colors border {cat.has_vr ? 'bg-purple-500/10 text-purple-300 border-purple-500/25 hover:bg-purple-500/20' : 'bg-white/[0.04] text-zinc-500 border-white/[0.08] hover:text-zinc-300'}"
                    title="Klik untuk mengubah status Video Review (VR)"
                  >
                    {cat.has_vr ? "VR AKTIF" : "NON-VR"}
                  </button>
                </td>

                <!-- Actions -->
                <td class="px-4 py-2.5 text-right pr-5">
                  <div class="flex items-center justify-end space-x-1">
                    <button
                      onclick={() => openAthletesModal(cat)}
                      class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition-colors"
                      title="Lihat atlet terdaftar"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                      </svg>
                    </button>
                    <button
                      onclick={() => openEditModal(cat)}
                      class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition-colors"
                      title="Edit kategori"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                      </svg>
                    </button>
                    <button
                      onclick={() => openDeleteSingleModal(cat)}
                      class="p-1.5 text-zinc-400 hover:text-rose-400 rounded-lg hover:bg-white/[0.08] transition-colors"
                      title="Hapus kategori"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <!-- Pagination Bar -->
      <div class="bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0 font-sans">
        <div class="flex items-center space-x-2 tabular-nums">
          <span>Menampilkan {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, filteredCategories.length)} dari {filteredCategories.length} kategori</span>
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
              class="h-7 px-2.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:pointer-events-none transition-colors"
            >
              ‹
            </button>
            <span class="px-2 text-zinc-400 tabular-nums">{currentPage} / {totalPages}</span>
            <button
              disabled={currentPage >= totalPages}
              onclick={() => (currentPage = Math.min(totalPages, currentPage + 1))}
              class="h-7 px-2.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:pointer-events-none transition-colors"
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
          class="text-zinc-500 hover:text-white transition-colors"
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
                class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] text-[10px] font-mono font-semibold transition-colors"
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
              class="rounded border-white/20 bg-black/40 text-white focus:ring-0 w-4 h-4"
            />
            <span class="font-bold text-white text-xs">Video Review (VR)</span>
          </label>

          <label class="flex items-center space-x-2.5 p-3 rounded-xl bg-[#111215] border border-white/[0.08] cursor-pointer hover:border-white/20 transition-colors">
            <input
              type="checkbox"
              bind:checked={formInput.is_bob}
              class="rounded border-white/20 bg-black/40 text-white focus:ring-0 w-4 h-4"
            />
            <span class="font-bold text-white text-xs">Best of the Best (BOB)</span>
          </label>
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2 mt-6 pt-3.5 border-t border-white/[0.08]">
        <button
          onclick={() => (categoryModalOpen = false)}
          disabled={isSaving}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleSaveCategory}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors flex items-center space-x-1.5 disabled:opacity-50"
        >
          {#if isSaving}
            <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
          {/if}
          <span>{isEditMode ? "Simpan Perubahan" : "Simpan Kategori"}</span>
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
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleConfirmDeleteSingle}
          disabled={isDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl shadow-md transition-colors flex items-center space-x-1 disabled:opacity-50"
        >
          <span>{isDeleting ? "Menghapus..." : "Hapus Kategori"}</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Batch -->
{#if batchDeleteModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl max-w-sm w-full p-6 shadow-2xl animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 mb-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">Hapus {selectedIds.size} Kategori Terpilih</h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 leading-relaxed mb-4">
        Apakah Anda yakin ingin menghapus <strong>{selectedIds.size}</strong> kategori pertandingan sekaligus? Tindakan ini tidak dapat dibatalkan.
      </p>

      <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          onclick={() => (batchDeleteModalOpen = false)}
          disabled={isBatchDeleting}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleBatchDelete}
          disabled={isBatchDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl shadow-md transition-colors flex items-center space-x-1 disabled:opacity-50"
        >
          <span>{isBatchDeleting ? "Menghapus..." : `Hapus ${selectedIds.size} Kategori`}</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Daftar Peserta Terdaftar -->
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
          class="text-zinc-500 hover:text-white transition-colors"
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
                <th class="py-2.5 px-3 font-bold">KONTINGEN</th>
                <th class="py-2.5 px-3 font-bold">PERGURUAN</th>
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
                  <td class="py-2.5 px-3 text-zinc-300 uppercase text-xs">
                    {atlet.utusan_nama || "-"}
                  </td>
                  <td class="py-2.5 px-3 text-zinc-400 text-xs">
                    {atlet.perguruan_nama || "—"}
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
              class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors"
            >
              Buka di Data Atlet
            </button>
          {/if}
          <button
            type="button"
            onclick={() => (athletesModalOpen = false)}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors"
          >
            Tutup
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
