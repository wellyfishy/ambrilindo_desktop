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

  // Stat cards double as the filter (replaces the separate metric cards + pills).
  type StatusKey = typeof selectedStatusFilter;
  let statusTabs = $derived<{ key: StatusKey; label: string; count: number }[]>([
    { key: "all", label: "Semua", count: totalCount },
    { key: "kata", label: "Kata", count: totalKata },
    { key: "kumite", label: "Kumite", count: totalKumite },
    { key: "festival", label: "Festival", count: festivalCount },
    { key: "undrawn", label: "Belum drawing", count: totalUndrawn },
    { key: "drawn", label: "Sudah drawing", count: totalDrawn },
    { key: "vr", label: "Video review", count: totalVrActive },
  ]);

  let hasActiveFilters = $derived(!!searchKeyword.trim() || selectedStatusFilter !== "all" || selectedGenderFilter !== "all");

  function resetFilters() {
    searchKeyword = "";
    selectedStatusFilter = "all";
    selectedGenderFilter = "all";
  }
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

  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-4 pb-4 border-b border-white/[0.08] shrink-0">
    <div class="flex items-center gap-2.5">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">Nomor tanding</h2>
      <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">{totalCount}</span>
    </div>

    <div class="flex items-center gap-2 shrink-0">
      {#if onOpenDrawingModal}
        <button
          type="button"
          onclick={onOpenDrawingModal}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          title="Drawing bagan prestasi"
        >
          <svg class="w-4 h-4 text-amber-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M8 21h8M12 17v4M7 4h10v5a5 5 0 01-10 0V4z" /></svg>
          <span>Drawing bagan</span>
        </button>
      {/if}
      <button
        type="button"
        onclick={openCreateModal}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center gap-1.5 shadow-md cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:ring-offset-2 focus-visible:ring-offset-[#14161b]"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14" /></svg>
        <span>Tambah kategori</span>
      </button>
    </div>
  </div>

  <!-- Status: stat cards that are also the filter -->
  <div class="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-7 gap-2 my-4 shrink-0" role="radiogroup" aria-label="Filter nomor tanding">
    {#each statusTabs as tab (tab.key)}
      {@const selected = selectedStatusFilter === tab.key}
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        onclick={() => (selectedStatusFilter = tab.key)}
        class="text-left rounded-xl px-3 py-2.5 border transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {selected
          ? 'bg-white/[0.08] border-white/50'
          : 'bg-[#181a20] border-white/[0.08] hover:border-white/20'}"
      >
        <span class="block text-xs font-semibold text-zinc-400 truncate">{tab.label}</span>
        <span class="block mt-1 text-2xl font-black font-mono tabular-nums text-white">{tab.count}</span>
      </button>
    {/each}
  </div>

  <!-- Toolbar: search first, then gender -->
  <div class="flex flex-wrap items-center gap-2 mb-3 shrink-0">
    <div class="relative flex-1 min-w-[220px]">
      <svg class="w-4 h-4 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
      <input
        type="text"
        bind:value={searchKeyword}
        placeholder="Cari kategori, umur, atau kelas"
        aria-label="Cari nomor tanding"
        class="w-full h-9 bg-[#181a20] text-sm text-white pl-9 pr-9 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors placeholder-zinc-500 select-text"
      />
      {#if searchKeyword}
        <button
          type="button"
          onclick={() => (searchKeyword = "")}
          aria-label="Hapus pencarian"
          class="absolute right-2 top-1/2 -translate-y-1/2 w-6 h-6 rounded-md text-zinc-500 hover:text-white hover:bg-white/[0.08] flex items-center justify-center cursor-pointer"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
        </button>
      {/if}
    </div>

    <label class="flex items-center gap-2 text-xs text-zinc-400">
      <span>Gender</span>
      <select
        bind:value={selectedGenderFilter}
        class="h-9 bg-[#181a20] border border-white/[0.1] text-sm text-zinc-200 rounded-xl px-2.5 outline-none focus:border-white transition-colors cursor-pointer"
      >
        <option value="all">Semua</option>
        <option value="putra">Putra</option>
        <option value="putri">Putri</option>
        <option value="campuran">Campuran</option>
      </select>
    </label>

    {#if hasActiveFilters}
      <button
        type="button"
        onclick={resetFilters}
        class="h-9 px-3 rounded-xl text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition cursor-pointer"
      >
        Reset filter
      </button>
    {/if}
  </div>

  <!-- Categories table -->
  <div class="flex-1 bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden flex flex-col shadow-lg min-h-0">
    {#if isLoading && categories.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 gap-3" role="status">
        <svg class="w-8 h-8 animate-spin text-zinc-500" fill="none" viewBox="0 0 24 24" aria-hidden="true"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" /><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" /></svg>
        <p class="text-sm text-zinc-400">Memuat nomor tanding…</p>
      </div>
    {:else if filteredCategories.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 gap-3 text-center">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-zinc-500" aria-hidden="true">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M10 6h11M10 12h11M10 18h11M4 6h1M4 12h1M4 18h1" /></svg>
        </div>
        {#if categories.length === 0}
          <p class="text-sm font-bold text-white">Belum ada nomor tanding</p>
          <button
            type="button"
            onclick={openCreateModal}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md cursor-pointer"
          >
            Tambah kategori
          </button>
        {:else}
          <p class="text-sm font-bold text-white">Tidak ada nomor tanding yang cocok</p>
          <button
            type="button"
            onclick={resetFilters}
            class="h-9 px-3.5 rounded-xl bg-white/[0.05] hover:bg-white/[0.1] text-xs font-semibold text-zinc-200 hover:text-white border border-white/[0.1] transition-colors cursor-pointer"
          >
            Reset filter
          </button>
        {/if}
      </div>
    {:else}
      <div class="flex-1 overflow-y-auto min-h-0">
        <table class="w-full text-left border-collapse text-[13px] select-text">
          <thead class="bg-[#111215] sticky top-0 z-10 border-b border-white/[0.08] text-zinc-400 text-xs select-none">
            <tr>
              <th scope="col" class="px-4 py-2.5 font-semibold">Nomor tanding</th>
              <th scope="col" class="w-36 px-4 py-2.5 font-semibold">Peserta</th>
              <th scope="col" class="w-48 px-4 py-2.5 font-semibold">Bagan</th>
              <th scope="col" class="w-24 px-2 py-2.5 font-semibold text-center">VR</th>
              <th scope="col" class="w-24 px-4 py-2.5"><span class="sr-only">Aksi</span></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.05]">
            {#each paginatedCategories as cat (cat.id)}
              {@const upperName = (cat.nama_nomor_tanding || "").toUpperCase()}
              {@const isKata = upperName.includes("KATA")}
              {@const isKumite = upperName.includes("KUMITE")}
              {@const isFestival = upperName.includes("FESTIVAL")}
              <tr class="hover:bg-white/[0.025] transition-colors">
                <!-- Nama + jenis -->
                <td class="px-4 py-2.5 align-top">
                  <p class="font-semibold text-white leading-snug">{cat.nama_nomor_tanding || `Kategori #${cat.id}`}</p>
                  <p class="mt-0.5 text-xs text-zinc-500 flex items-center gap-1.5">
                    {#if isKata}<span>Kata</span>{:else if isKumite}<span>Kumite</span>{/if}
                    {#if isKata || isKumite}<span aria-hidden="true">·</span>{/if}
                    {#if isFestival}
                      <span class="text-amber-300/90">Festival</span>
                    {:else}
                      <span>Prestasi</span>
                    {/if}
                    {#if cat.is_bob}
                      <span class="px-1.5 rounded bg-purple-500/15 text-purple-300 border border-purple-500/30 font-semibold">BOB</span>
                    {/if}
                  </p>
                </td>

                <!-- Peserta (opens list) -->
                <td class="px-4 py-2.5 align-top">
                  <button
                    type="button"
                    onclick={() => openAthletesModal(cat)}
                    class="inline-flex items-center gap-1 h-8 px-2 -ml-2 rounded-lg transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {cat.athlete_count > 0 ? 'text-zinc-200 hover:text-white hover:bg-white/[0.06]' : 'text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04]'}"
                    title="Lihat peserta"
                  >
                    <span class="tabular-nums font-semibold">{cat.athlete_count}</span>
                    <span class="text-zinc-500">atlet</span>
                    <svg class="w-3.5 h-3.5 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                  </button>
                </td>

                <!-- Bagan status (icon + text) -->
                <td class="px-4 py-2.5 align-top">
                  {#if (cat.bagan_count || 0) > 0}
                    <div class="flex items-center gap-1">
                      <span class="inline-flex items-center gap-1.5 text-zinc-200">
                        <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                        <span class="tabular-nums">{cat.bagan_count} bagan</span>
                      </span>
                      {#if cat.first_bagan_id && onNavigate}
                        <button
                          type="button"
                          onclick={() => handleGoToBagan(cat)}
                          class="h-8 px-2 rounded-lg text-xs font-semibold text-zinc-400 hover:text-white hover:bg-white/[0.06] transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                          title="Buka bagan"
                        >
                          Buka
                        </button>
                      {/if}
                    </div>
                  {:else}
                    <div class="flex items-center gap-1">
                      <span class="text-zinc-500">Belum drawing</span>
                      {#if onOpenDrawingModal && cat.athlete_count > 0}
                        <button
                          type="button"
                          onclick={onOpenDrawingModal}
                          class="h-8 px-2 rounded-lg text-xs font-semibold text-amber-300 hover:text-amber-200 hover:bg-amber-500/10 transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-300/50"
                          title="Drawing bagan"
                        >
                          Drawing
                        </button>
                      {/if}
                    </div>
                  {/if}
                </td>

                <!-- VR toggle -->
                <td class="px-2 py-2.5 text-center align-top">
                  <button
                    type="button"
                    onclick={() => handleToggleVr(cat)}
                    aria-pressed={!!cat.has_vr}
                    aria-label={cat.has_vr ? `Matikan video review ${cat.nama_nomor_tanding}` : `Aktifkan video review ${cat.nama_nomor_tanding}`}
                    title={cat.has_vr ? "Video review aktif (klik untuk matikan)" : "Aktifkan video review"}
                    class="h-8 px-2.5 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-300/50 {cat.has_vr
                      ? 'bg-purple-500/15 text-purple-200 border border-purple-500/30 hover:bg-purple-500/25'
                      : 'text-zinc-500 border border-white/[0.08] hover:text-zinc-200 hover:bg-white/[0.05]'}"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15 10l4.6-2.3A1 1 0 0121 8.6v6.8a1 1 0 01-1.4.9L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
                    {cat.has_vr ? "Aktif" : "Mati"}
                  </button>
                </td>

                <!-- Actions -->
                <td class="px-4 py-2.5 align-top">
                  <div class="flex items-center justify-end gap-1">
                    <button
                      type="button"
                      onclick={() => openEditModal(cat)}
                      class="w-9 h-9 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                      title={`Ubah ${cat.nama_nomor_tanding}`}
                      aria-label={`Ubah ${cat.nama_nomor_tanding}`}
                    >
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M16.9 4.5l1.6-1.7a1.9 1.9 0 112.7 2.7L10.6 16.1a4.5 4.5 0 01-1.9 1.1L6 18l.8-2.7a4.5 4.5 0 011.1-1.9l9-8.9z" /></svg>
                    </button>
                    <button
                      type="button"
                      onclick={() => openDeleteSingleModal(cat)}
                      class="w-9 h-9 rounded-lg text-zinc-400 hover:text-rose-300 hover:bg-rose-500/10 flex items-center justify-center transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-400/50"
                      title={`Hapus ${cat.nama_nomor_tanding}`}
                      aria-label={`Hapus ${cat.nama_nomor_tanding}`}
                    >
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 7h16M10 11v6M14 11v6M5 7l1 12a2 2 0 002 2h8a2 2 0 002-2l1-12M9 7V4a1 1 0 011-1h4a1 1 0 011 1v3" /></svg>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <!-- Pagination -->
      <div class="bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0">
        <div class="flex items-center gap-3 tabular-nums">
          <span>{paginatedCategories.length > 0 ? (currentPage - 1) * (pageSize === -1 ? 0 : pageSize) + 1 : 0}–{pageSize === -1 ? filteredCategories.length : Math.min(currentPage * pageSize, filteredCategories.length)} dari {filteredCategories.length}</span>
          <label class="flex items-center gap-1.5">
            <span>Per halaman</span>
            <select
              bind:value={pageSize}
              onchange={() => (currentPage = 1)}
              class="bg-[#181a20] text-zinc-200 text-xs px-2 py-1 rounded-lg border border-white/[0.1] outline-none focus:border-white cursor-pointer"
            >
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
              <option value={-1}>Semua</option>
            </select>
          </label>
        </div>

        {#if pageSize !== -1 && totalPages > 1}
          <div class="flex items-center gap-1">
            <button
              type="button"
              disabled={currentPage <= 1}
              onclick={() => (currentPage = Math.max(1, currentPage - 1))}
              aria-label="Halaman sebelumnya"
              class="w-8 h-8 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:pointer-events-none flex items-center justify-center transition-colors cursor-pointer"
            >
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" /></svg>
            </button>
            <span class="px-2 font-mono text-zinc-300 tabular-nums">{currentPage} / {totalPages}</span>
            <button
              type="button"
              disabled={currentPage >= totalPages}
              onclick={() => (currentPage = Math.min(totalPages, currentPage + 1))}
              aria-label="Halaman berikutnya"
              class="w-8 h-8 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 hover:text-white disabled:opacity-30 disabled:pointer-events-none flex items-center justify-center transition-colors cursor-pointer"
            >
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
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
          class="w-8 h-8 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition-colors cursor-pointer"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
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
          class="w-8 h-8 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition-colors cursor-pointer"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
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
