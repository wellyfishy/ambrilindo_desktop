<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { EnrichedAtletItem, UtusanItem, PerguruanItem, NomorTandingItem, PullResult } from "$lib/types";
  import FuzzySelect, { type SelectOption } from "./FuzzySelect.svelte";

  interface Props {
    viewType?: 'roster' | 'contingent' | 'sync';
    eventId: number;
    hostAddress: string;
    onShowToast?: (msg: string) => void;
  }

  let { viewType = 'roster', eventId, hostAddress, onShowToast }: Props = $props();

  // Data State
  let athletes = $state<EnrichedAtletItem[]>([]);
  let contingents = $state<UtusanItem[]>([]);
  let perguruans = $state<PerguruanItem[]>([]);
  let categories = $state<NomorTandingItem[]>([]);
  let isLoading = $state(false);
  let isSyncing = $state(false);

  // Logo Cache Map
  let logoMap = $state<Record<string, string>>({});

  // Filter & Search State
  let searchKeyword = $state("");
  let selectedCategoryFilter = $state<string>("all");
  let selectedContingentFilter = $state<string>("all");
  let selectedPerguruanFilter = $state<string>("all");
  let selectedStatusFilter = $state<'all' | 'tournament' | 'festival' | 'seed' | 'tanpa_nik'>('all');

  // Pagination State
  let currentPage = $state(1);
  let pageSize = $state(50);

  // Modals State
  let athleteModalOpen = $state(false);
  let isEditMode = $state(false);
  let isSaving = $state(false);
  let formError = $state<string | null>(null);

  let formInput = $state<{
    id?: number | null;
    nama_atlet: string;
    nik: string;
    kode_atlet: string;
    additional_code: string;
    is_priority: boolean;
    perguruan_id: number | null;
    utusan_id: number | null;
    nomor_tanding_id: number | null;
  }>({
    nama_atlet: "",
    nik: "",
    kode_atlet: "",
    additional_code: "",
    is_priority: false,
    perguruan_id: null,
    utusan_id: null,
    nomor_tanding_id: null,
  });

  let deleteModalOpen = $state(false);
  let athleteToDelete = $state<EnrichedAtletItem | null>(null);
  let isDeleting = $state(false);

  // Toast State
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

  // Resolves Utusan logos to base64 Data URLs via IPC or HTTP
  async function resolveLogos(items: EnrichedAtletItem[]) {
    const needed = new Set<string>();
    for (const a of items) {
      if (a.utusan_logo) needed.add(a.utusan_logo);
    }
    for (const path of needed) {
      if (logoMap[path]) continue;
      if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("data:")) {
        logoMap[path] = path;
        continue;
      }
      try {
        const dataUrl = await invoke<string | null>("get_media_data_url", { relativePath: path });
        if (dataUrl) {
          logoMap[path] = dataUrl;
          continue;
        }
      } catch {
        // Fallback to HTTP
      }
      if (hostAddress) {
        logoMap[path] = `${hostAddress.replace(/\/$/, "")}/media/${path.replace(/^\//, "")}`;
      }
    }
  }

  async function loadData() {
    if (!eventId) return;
    isLoading = true;
    try {
      const [atletList, catList, utusanList, perguruanList] = await Promise.all([
        invoke<EnrichedAtletItem[]>("get_enriched_athletes", { eventId }),
        invoke<NomorTandingItem[]>("get_categories", { eventId }),
        invoke<UtusanItem[]>("get_contingents", { eventId }),
        invoke<PerguruanItem[]>("get_perguruans", { eventId }),
      ]);
      athletes = atletList || [];
      categories = catList || [];
      contingents = utusanList || [];
      perguruans = perguruanList || [];
      resolveLogos(athletes);
    } catch (err: any) {
      console.error("Gagal memuat data roster:", err);
      const msg = typeof err === "string" ? err : err?.message || JSON.stringify(err);
      showToast(`Gagal memuat data: ${msg}`);
    } finally {
      isLoading = false;
    }
  }

  async function handleSyncPull() {
    if (!eventId) return;
    isSyncing = true;
    try {
      const res = await invoke<PullResult>("pull_athletes_from_hosted", { localEventId: eventId });
      showToast(res.message);
      await loadData();
    } catch (err: any) {
      showToast(`Gagal sinkron atlet: ${err}`);
    } finally {
      isSyncing = false;
    }
  }

  function isFestival(a: EnrichedAtletItem): boolean {
    const nt = (a.nomor_tanding_nama || "").toLowerCase();
    const tipe = (a.nomor_tanding_tipe || "").toLowerCase();
    return nt.includes("festival") || tipe.includes("festival");
  }

  function getCategoryBadge(c: NomorTandingItem): string {
    if (c.is_bob) return "BOB";
    const nt = (c.nama_nomor_tanding || "").toLowerCase();
    const tipe = (c.tipe || "").toLowerCase();
    if (nt.includes("festival") || tipe.includes("festival")) return "FESTIVAL";
    return "PRESTASI";
  }

  // --- Option Lists for FuzzySelect ---
  let categoryFilterOptions = $derived<SelectOption[]>([
    { value: "all", label: "Semua Kategori" },
    ...categories.map((c) => ({
      value: String(c.id),
      label: c.nama_nomor_tanding || `[#${c.id}] Tanpa Nama`,
      badge: getCategoryBadge(c),
    })),
  ]);

  let contingentFilterOptions = $derived<SelectOption[]>([
    { value: "all", label: "Semua Kontingen" },
    ...contingents.map((c) => ({
      value: String(c.id),
      label: c.nama_utusan || `[#${c.id}] Tanpa Nama`,
    })),
  ]);

  let perguruanFilterOptions = $derived<SelectOption[]>([
    { value: "all", label: "Semua Perguruan" },
    ...perguruans.map((p) => ({
      value: String(p.id),
      label: p.nama_perguruan || `[#${p.id}] Tanpa Nama`,
    })),
  ]);

  let modalCategoryOptions = $derived<SelectOption[]>(
    categories.map((c) => ({
      value: c.id,
      label: c.nama_nomor_tanding || `[#${c.id}] Tanpa Nama`,
      badge: getCategoryBadge(c),
    }))
  );

  let modalContingentOptions = $derived<SelectOption[]>(
    contingents.map((c) => ({
      value: c.id,
      label: c.nama_utusan || `[#${c.id}] Tanpa Nama`,
    }))
  );

  let modalPerguruanOptions = $derived<SelectOption[]>([
    { value: null, label: "-- Tanpa Perguruan --" },
    ...perguruans.map((p) => ({
      value: p.id,
      label: p.nama_perguruan || `[#${p.id}] Tanpa Nama`,
    })),
  ]);

  // --- Metrics Derivation ---
  let totalCount = $derived(athletes.length);
  let tournamentCount = $derived(athletes.filter((a) => !isFestival(a) && !a.is_bob).length);
  let festivalCount = $derived(athletes.filter((a) => isFestival(a)).length);
  let seedCount = $derived(athletes.filter((a) => a.is_priority).length);
  let missingNikCount = $derived(athletes.filter((a) => !a.nik || a.nik.trim() === "").length);

  // --- Filtering ---
  let filteredAthletes = $derived.by(() => {
    const q = searchKeyword.trim().toLowerCase();
    return athletes.filter((a) => {
      // Search keyword filter
      if (q) {
        const matchName = (a.nama_atlet || "").toLowerCase().includes(q);
        const matchNik = (a.nik || "").toLowerCase().includes(q);
        const matchKode = (a.kode_atlet || "").toLowerCase().includes(q);
        const matchKta = (a.additional_code || "").toLowerCase().includes(q);
        const matchUtusan = (a.utusan_nama || "").toLowerCase().includes(q);
        const matchPerguruan = (a.perguruan_nama || "").toLowerCase().includes(q);
        const matchNt = (a.nomor_tanding_nama || "").toLowerCase().includes(q);
        if (!matchName && !matchNik && !matchKode && !matchKta && !matchUtusan && !matchPerguruan && !matchNt) {
          return false;
        }
      }

      // Category dropdown filter
      if (selectedCategoryFilter !== "all") {
        if (String(a.nomor_tanding_id) !== selectedCategoryFilter) return false;
      }

      // Contingent dropdown filter
      if (selectedContingentFilter !== "all") {
        if (String(a.utusan_id) !== selectedContingentFilter) return false;
      }

      // Perguruan dropdown filter
      if (selectedPerguruanFilter !== "all") {
        if (String(a.perguruan_id) !== selectedPerguruanFilter) return false;
      }

      // Quick status pill filter
      if (selectedStatusFilter === 'tournament') {
        if (isFestival(a) || a.is_bob) return false;
      } else if (selectedStatusFilter === 'festival') {
        if (!isFestival(a)) return false;
      } else if (selectedStatusFilter === 'seed') {
        if (!a.is_priority) return false;
      } else if (selectedStatusFilter === 'tanpa_nik') {
        if (a.nik && a.nik.trim() !== "") return false;
      }

      return true;
    });
  });

  // --- Pagination ---
  let totalPages = $derived(Math.max(1, Math.ceil(filteredAthletes.length / pageSize)));
  let paginatedAthletes = $derived.by(() => {
    if (pageSize === -1) return filteredAthletes;
    const start = (currentPage - 1) * pageSize;
    return filteredAthletes.slice(start, start + pageSize);
  });

  $effect(() => {
    // Reset to page 1 whenever filters change
    if (searchKeyword || selectedCategoryFilter || selectedContingentFilter || selectedPerguruanFilter || selectedStatusFilter) {
      currentPage = 1;
    }
  });

  // --- CRUD Operations ---
  function openCreateModal() {
    isEditMode = false;
    formError = null;
    formInput = {
      nama_atlet: "",
      nik: "",
      kode_atlet: "",
      additional_code: "",
      is_priority: false,
      perguruan_id: perguruans[0]?.id || null,
      utusan_id: contingents[0]?.id || null,
      nomor_tanding_id: categories[0]?.id || null,
    };
    athleteModalOpen = true;
  }

  function openEditModal(atlet: EnrichedAtletItem) {
    isEditMode = true;
    formError = null;
    formInput = {
      id: atlet.id,
      nama_atlet: atlet.nama_atlet || "",
      nik: atlet.nik || "",
      kode_atlet: atlet.kode_atlet || "",
      additional_code: atlet.additional_code || "",
      is_priority: atlet.is_priority,
      perguruan_id: atlet.perguruan_id,
      utusan_id: atlet.utusan_id,
      nomor_tanding_id: atlet.nomor_tanding_id,
    };
    athleteModalOpen = true;
  }

  async function handleSaveAthlete() {
    if (!formInput.nama_atlet.trim()) {
      formError = "Nama atlet wajib diisi.";
      return;
    }
    formError = null;
    isSaving = true;

    try {
      const payload = {
        id: formInput.id || null,
        event_id: eventId,
        nama_atlet: formInput.nama_atlet.trim().toUpperCase(),
        nik: formInput.nik.trim() || null,
        kode_atlet: formInput.kode_atlet.trim() || null,
        additional_code: formInput.additional_code.trim() || null,
        is_priority: formInput.is_priority,
        perguruan_id: formInput.perguruan_id ? Number(formInput.perguruan_id) : null,
        utusan_id: formInput.utusan_id ? Number(formInput.utusan_id) : null,
        nomor_tanding_id: formInput.nomor_tanding_id ? Number(formInput.nomor_tanding_id) : null,
      };

      if (isEditMode) {
        await invoke("update_athlete", { id: payload.id, input: payload });
        showToast(`Data ${payload.nama_atlet} berhasil diperbarui.`);
      } else {
        await invoke("create_athlete", { input: payload });
        showToast(`Atlet ${payload.nama_atlet} berhasil ditambahkan.`);
      }
      athleteModalOpen = false;
      await loadData();
    } catch (err: any) {
      formError = typeof err === "string" ? err : JSON.stringify(err);
    } finally {
      isSaving = false;
    }
  }

  async function handleTogglePriority(atlet: EnrichedAtletItem) {
    const nextVal = !atlet.is_priority;
    try {
      await invoke("toggle_athlete_priority", { id: atlet.id, isPriority: nextVal });
      athletes = athletes.map((a) => (a.id === atlet.id ? { ...a, is_priority: nextVal } : a));
      showToast(`${atlet.nama_atlet}: ${nextVal ? 'Status Seed Aktif' : 'Status Seed Dicabut'}`);
    } catch (err: any) {
      showToast(`Gagal mengubah seed: ${err}`);
    }
  }

  function openDeleteSingleModal(atlet: EnrichedAtletItem) {
    athleteToDelete = atlet;
    deleteModalOpen = true;
  }

  async function confirmDeleteSingle() {
    if (!athleteToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_athlete", { id: athleteToDelete.id });
      athletes = athletes.filter((a) => a.id !== athleteToDelete!.id);
      showToast(`Atlet ${athleteToDelete.nama_atlet} berhasil dihapus`);
      deleteModalOpen = false;
      athleteToDelete = null;
    } catch (err: any) {
      showToast(`Gagal menghapus atlet: ${err?.message || err}`);
    } finally {
      isDeleting = false;
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
          DATA ATLET &amp; PESERTA
        </h2>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">
          {totalCount} PESERTA
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1 font-normal">
        Verifikasi peserta turnamen, penomoran atlet, filter perguruan dan kontingen, serta penetapan status unggulan (seeding)
      </p>
    </div>

    <!-- Action Buttons (No manual refresh needed, realtime on action) -->
    <div class="flex items-center space-x-2 shrink-0 self-start sm:self-auto">
      <button
        type="button"
        onclick={handleSyncPull}
        disabled={isSyncing}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-50 cursor-pointer shadow-xs"
        title="Tarik data pendaftaran atlet terbaru dari portal ambrilindo.com"
      >
        <svg class="w-3.5 h-3.5 text-sky-400 {isSyncing ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
        </svg>
        <span>{isSyncing ? "Menarik..." : "Tarik Atlet Web"}</span>
      </button>

      <button
        type="button"
        onclick={openCreateModal}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center space-x-1.5 active:scale-95 shadow-md cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
        </svg>
        <span>Tambah Atlet</span>
      </button>
    </div>
  </div>

  <!-- Stat Cards Strip (5-Column Clean Grid) -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
    <!-- 1. Total Atlet -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">TOTAL ATLET</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-white tabular-nums">{totalCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">atlet</span>
      </div>
    </div>

    <!-- 2. Prestasi / Open -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">PRESTASI (WKF)</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-emerald-400 tabular-nums">{tournamentCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">atlet</span>
      </div>
    </div>

    <!-- 3. Festival -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">FESTIVAL</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-amber-400 tabular-nums">{festivalCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">atlet</span>
      </div>
    </div>

    <!-- 4. Unggulan (Seed) -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">UNGGULAN (SEED)</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-purple-400 tabular-nums">{seedCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">prioritas</span>
      </div>
    </div>

    <!-- 5. Belum Ada NIK -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">BELUM ADA NIK</span>
      <div class="flex items-baseline justify-between mt-2">
        <span class="text-2xl font-black font-mono text-rose-400 tabular-nums">{missingNikCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">verifikasi</span>
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
        onclick={() => (selectedStatusFilter = 'tournament')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'tournament'
          ? 'bg-emerald-400 text-black border-emerald-400 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Prestasi ({tournamentCount})
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
        onclick={() => (selectedStatusFilter = 'seed')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'seed'
          ? 'bg-purple-400 text-black border-purple-400 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Seed ({seedCount})
      </button>
      <button
        onclick={() => (selectedStatusFilter = 'tanpa_nik')}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === 'tanpa_nik'
          ? 'bg-rose-500 text-white border-rose-500 shadow-xs font-bold'
          : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
      >
        Tanpa NIK ({missingNikCount})
      </button>
    </div>

    <!-- Dropdown Filters & Search Bar -->
    <div class="flex items-center space-x-2 shrink-0">
      <!-- Filter Nomor Tanding -->
      <FuzzySelect
        items={categoryFilterOptions}
        bind:value={selectedCategoryFilter}
        placeholder="Filter Kategori"
        allowClear={true}
        clearLabel="Semua Kategori"
        clearValue="all"
        compact={true}
        class="w-40"
      />

      <!-- Filter Perguruan FORKI (Dulu) -->
      <FuzzySelect
        items={perguruanFilterOptions}
        bind:value={selectedPerguruanFilter}
        placeholder="Filter Perguruan"
        allowClear={true}
        clearLabel="Semua Perguruan"
        clearValue="all"
        compact={true}
        class="w-36"
      />

      <!-- Filter Kontingen (Kemudian) -->
      <FuzzySelect
        items={contingentFilterOptions}
        bind:value={selectedContingentFilter}
        placeholder="Filter Kontingen"
        allowClear={true}
        clearLabel="Semua Kontingen"
        clearValue="all"
        compact={true}
        class="w-36"
      />

      <!-- Search Input -->
      <div class="relative w-48">
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari nama, perguruan, kontingen..."
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

  <!-- Athletes Table View Container (Clean 6-Column Layout, No Checkboxes) -->
  <div class="flex-1 bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden flex flex-col shadow-inner min-h-0">
    {#if isLoading && athletes.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
        <svg class="w-8 h-8 animate-spin text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <p class="text-xs text-zinc-400 font-mono">Memuat database atlet...</p>
      </div>
    {:else if filteredAthletes.length === 0}
      <!-- Empty State -->
      <div class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-zinc-500">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
          </svg>
        </div>
        <p class="text-sm font-bold text-white">Tidak Ada Data Atlet Ditemukan</p>
        <p class="text-xs text-zinc-500 max-w-sm text-center">
          Tidak ada atlet yang cocok dengan filter atau kata kunci yang dipilih. Coba reset filter atau tambahkan atlet baru.
        </p>
        {#if searchKeyword || selectedCategoryFilter !== "all" || selectedContingentFilter !== "all" || selectedPerguruanFilter !== "all" || selectedStatusFilter !== "all"}
          <button
            onclick={() => {
              searchKeyword = "";
              selectedCategoryFilter = "all";
              selectedContingentFilter = "all";
              selectedPerguruanFilter = "all";
              selectedStatusFilter = "all";
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
              <th class="px-4 py-3 font-bold">NAMA ATLET &amp; IDENTITAS</th>
              <th class="px-4 py-3 font-bold">PERGURUAN</th>
              <th class="px-4 py-3 font-bold">KONTINGEN</th>
              <th class="px-4 py-3 font-bold">NOMOR TANDING / KELAS</th>
              <th class="w-24 px-3 py-3 text-center font-bold">UNGGULAN</th>
              <th class="w-24 px-4 py-3 text-center pr-6 font-bold">AKSI</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.04]">
            {#each paginatedAthletes as atlet, index (atlet.id)}
              <tr class="hover:bg-white/[0.02] transition-colors">
                <!-- Row Number -->
                <td class="px-3 py-3 text-center font-mono text-zinc-500 text-[11px] tabular-nums">
                  {(currentPage - 1) * pageSize + index + 1}
                </td>

                <!-- Name & Identitas -->
                <td class="px-4 py-3">
                  <div class="flex items-center space-x-2">
                    <span class="font-bold text-white text-xs uppercase tracking-tight">
                      {atlet.nama_atlet}
                    </span>
                    {#if atlet.is_priority}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-black bg-amber-500/15 text-amber-300 border border-amber-500/30">
                        SEED
                      </span>
                    {/if}
                  </div>
                  <div class="flex items-center space-x-2 mt-1">
                    {#if atlet.nik}
                      <span class="font-mono text-[10px] text-zinc-400 tabular-nums">
                        NIK: {atlet.nik}
                      </span>
                    {:else}
                      <span class="font-mono text-[9px] font-bold text-rose-400 bg-rose-500/10 px-1.5 py-0.5 rounded border border-rose-500/20">
                        TANPA NIK
                      </span>
                    {/if}

                    {#if atlet.kode_atlet}
                      <span class="font-mono text-[10px] text-zinc-400 bg-white/[0.04] px-1.5 py-0.5 rounded border border-white/[0.08]">
                        #{atlet.kode_atlet}
                      </span>
                    {/if}

                    {#if atlet.additional_code}
                      <span class="font-mono text-[10px] text-zinc-500">
                        KTA: {atlet.additional_code}
                      </span>
                    {/if}
                  </div>
                </td>

                <!-- Perguruan (FORKI) - Dulu -->
                <td class="px-4 py-3">
                  {#if atlet.perguruan_nama}
                    <span class="inline-flex items-center space-x-1.5 text-xs font-semibold text-amber-300 bg-amber-500/10 px-2 py-0.5 rounded-lg border border-amber-500/20 uppercase tracking-tight">
                      <span class="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0"></span>
                      <span class="truncate max-w-[130px]" title={atlet.perguruan_nama}>{atlet.perguruan_nama}</span>
                    </span>
                  {:else}
                    <span class="text-xs font-mono text-zinc-500">—</span>
                  {/if}
                </td>

                <!-- Kontingen / Utusan - Kemudian -->
                <td class="px-4 py-3">
                  <div class="flex items-center space-x-2">
                    {#if atlet.utusan_logo && logoMap[atlet.utusan_logo]}
                      <img
                        src={logoMap[atlet.utusan_logo]}
                        alt="Logo"
                        class="w-5 h-5 object-contain shrink-0 rounded bg-white/[0.04]"
                      />
                    {/if}
                    <span class="font-bold text-zinc-200 text-xs uppercase truncate max-w-[180px]" title={atlet.utusan_nama}>
                      {atlet.utusan_nama || "KONTINGEN #" + (atlet.utusan_id || "-")}
                    </span>
                  </div>
                </td>

                <!-- Category -->
                <td class="px-4 py-3">
                  <div class="font-bold text-zinc-200 text-xs">
                    {atlet.nomor_tanding_nama || "KATEGORI #" + (atlet.nomor_tanding_id || "-")}
                  </div>
                  <div class="flex items-center space-x-1.5 mt-1">
                    {#if (atlet.nomor_tanding_nama || "").toLowerCase().includes("kata")}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.08]">
                        KATA
                      </span>
                    {:else}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.08]">
                        KUMITE
                      </span>
                    {/if}

                    {#if isFestival(atlet)}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/15 text-amber-300 border border-amber-500/30">
                        FESTIVAL
                      </span>
                    {:else if atlet.is_bob}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-purple-500/15 text-purple-300 border border-purple-500/30">
                        BOB
                      </span>
                    {:else}
                      <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-emerald-500/15 text-emerald-300 border border-emerald-500/30">
                        PRESTASI
                      </span>
                    {/if}
                  </div>
                </td>

                <!-- Seed Toggle -->
                <td class="w-24 px-3 py-3 text-center">
                  <button
                    onclick={() => handleTogglePriority(atlet)}
                    class="h-6 px-2.5 rounded-lg text-[10px] font-mono font-bold transition-all border active:scale-95 cursor-pointer {atlet.is_priority
                      ? 'bg-amber-500/15 text-amber-300 border-amber-500/30 hover:bg-amber-500/25 shadow-xs'
                      : 'bg-white/[0.04] text-zinc-500 border-white/[0.08] hover:text-zinc-300'}"
                    title="Klik untuk mengubah status unggulan (seeding)"
                    aria-label="Toggle status unggulan"
                  >
                    {atlet.is_priority ? "★ SEED" : "—"}
                  </button>
                </td>

                <!-- Actions (Dedicated Edit & Delete Buttons) -->
                <td class="w-24 px-4 py-3 text-center pr-6">
                  <div class="flex items-center justify-center space-x-1.5">
                    <!-- Edit Button -->
                    <button
                      onclick={() => openEditModal(atlet)}
                      class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-sky-500/15 text-zinc-400 hover:text-sky-300 border border-white/[0.08] hover:border-sky-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                      title={`Ubah data ${atlet.nama_atlet}`}
                      aria-label={`Ubah data ${atlet.nama_atlet}`}
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125" />
                      </svg>
                    </button>

                    <!-- Delete Button -->
                    <button
                      onclick={() => openDeleteSingleModal(atlet)}
                      class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-rose-500/15 text-zinc-400 hover:text-rose-400 border border-white/[0.08] hover:border-rose-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95 shadow-xs"
                      title={`Hapus ${atlet.nama_atlet}`}
                      aria-label={`Hapus ${atlet.nama_atlet}`}
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

      <!-- Pagination Bar -->
      <div class="bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0 font-sans">
        <div class="flex items-center space-x-2 tabular-nums">
          <span>Menampilkan {paginatedAthletes.length > 0 ? (currentPage - 1) * pageSize + 1 : 0} - {Math.min(currentPage * pageSize, filteredAthletes.length)} dari {filteredAthletes.length} atlet</span>
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

<!-- Modal: Tambah / Edit Data Atlet -->
{#if athleteModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-lg shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <!-- Modal Header -->
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight">
          {isEditMode ? "Edit Data Atlet" : "Tambah Atlet Baru"}
        </h3>
        <button onclick={() => (athleteModalOpen = false)} class="text-zinc-500 hover:text-white transition-colors cursor-pointer">
          ✕
        </button>
      </div>

      <!-- Modal Body Form -->
      <div class="p-6 space-y-4 overflow-y-auto max-h-[70vh]">
        {#if formError}
          <div class="bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs px-3 py-2 rounded-xl">
            {formError}
          </div>
        {/if}

        <!-- Nama Atlet -->
        <div>
          <label for="nama_atlet" class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Nama Lengkap Atlet <span class="text-rose-400">*</span>
          </label>
          <input
            id="nama_atlet"
            type="text"
            bind:value={formInput.nama_atlet}
            placeholder="Contoh: AHMAD FADILLAH"
            class="w-full bg-[#111215] text-xs font-semibold text-white uppercase px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors"
          />
        </div>

        <!-- NIK & KTA Row -->
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label for="nik" class="block text-xs font-semibold text-zinc-300 mb-1.5">NIK (16 Digit)</label>
            <input
              id="nik"
              type="text"
              bind:value={formInput.nik}
              placeholder="3507..."
              class="w-full bg-[#111215] text-xs font-mono text-zinc-200 px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors tabular-nums"
            />
          </div>
          <div>
            <label for="additional_code" class="block text-xs font-semibold text-zinc-300 mb-1.5">No. KTA / Kode Tambahan</label>
            <input
              id="additional_code"
              type="text"
              bind:value={formInput.additional_code}
              placeholder="KTA-..."
              class="w-full bg-[#111215] text-xs font-mono text-zinc-200 px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors"
            />
          </div>
        </div>

        <!-- Nomor Tanding Dropdown -->
        <div>
          <label for="nomor_tanding" class="block text-xs font-semibold text-zinc-300 mb-1.5">Nomor Tanding / Kelas</label>
          <FuzzySelect
            id="nomor_tanding"
            items={modalCategoryOptions}
            bind:value={formInput.nomor_tanding_id}
            placeholder="Pilih Nomor Tanding..."
            class="w-full"
          />
        </div>

        <!-- Perguruan & Kontingen Row (Perguruan DULU, baru Kontingen) -->
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label for="perguruan" class="block text-xs font-semibold text-zinc-300 mb-1.5">Perguruan (FORKI)</label>
            <FuzzySelect
              id="perguruan"
              items={modalPerguruanOptions}
              bind:value={formInput.perguruan_id}
              placeholder="Pilih Perguruan..."
              allowClear={true}
              clearLabel="-- Tanpa Perguruan --"
              clearValue={null}
              class="w-full"
            />
          </div>
          <div>
            <label for="utusan" class="block text-xs font-semibold text-zinc-300 mb-1.5">Kontingen / Utusan</label>
            <FuzzySelect
              id="utusan"
              items={modalContingentOptions}
              bind:value={formInput.utusan_id}
              placeholder="Pilih Kontingen..."
              class="w-full"
            />
          </div>
        </div>

        <!-- Seed / Prioritas Toggle -->
        <div class="pt-1">
          <label class="flex items-center space-x-2.5 cursor-pointer">
            <input
              type="checkbox"
              bind:checked={formInput.is_priority}
              class="rounded border-white/20 bg-black/40 text-white focus:ring-0 cursor-pointer w-4 h-4"
            />
            <span class="text-xs font-semibold text-zinc-200">
              Unggulan / Seed (Mendapatkan prioritas penempatan drawing bagan)
            </span>
          </label>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-end space-x-2">
        <button
          onclick={() => (athleteModalOpen = false)}
          disabled={isSaving}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
        >
          Batal
        </button>
        <button
          onclick={handleSaveAthlete}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          {isSaving ? "Menyimpan..." : isEditMode ? "Simpan Perubahan" : "Simpan Atlet"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Atlet Tunggal -->
{#if deleteModalOpen && athleteToDelete}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-sm shadow-2xl p-6 animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">Hapus Data Atlet</h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Apakah Anda yakin ingin menghapus atlet <strong class="text-white uppercase">{athleteToDelete.nama_atlet}</strong> dari database turnamen? Tindakan ini bersifat permanen.
      </p>

      <div class="mt-5 flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          onclick={() => { deleteModalOpen = false; athleteToDelete = null; }}
          disabled={isDeleting}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
        >
          Batal
        </button>
        <button
          onclick={confirmDeleteSingle}
          disabled={isDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          {isDeleting ? "Menghapus..." : "Hapus Atlet"}
        </button>
      </div>
    </div>
  </div>
{/if}
