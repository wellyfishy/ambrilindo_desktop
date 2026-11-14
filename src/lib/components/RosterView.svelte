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
    { value: "all", label: "Semua nomor tanding" },
    ...categories.map((c) => ({
      value: String(c.id),
      label: c.nama_nomor_tanding || `[#${c.id}] Tanpa Nama`,
      badge: getCategoryBadge(c),
    })),
  ]);

  let contingentFilterOptions = $derived<SelectOption[]>([
    { value: "all", label: "Semua kontingen" },
    ...contingents.map((c) => ({
      value: String(c.id),
      label: c.nama_utusan || `[#${c.id}] Tanpa Nama`,
    })),
  ]);

  let perguruanFilterOptions = $derived<SelectOption[]>([
    { value: "all", label: "Semua perguruan" },
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

  // Stat cards double as the status filter (one control instead of cards + pills).
  type StatusKey = 'all' | 'tournament' | 'festival' | 'seed' | 'tanpa_nik';
  let statusTabs = $derived<{ key: StatusKey; label: string; count: number; warn?: boolean; star?: boolean }[]>([
    { key: 'all', label: 'Semua', count: totalCount },
    { key: 'tournament', label: 'Prestasi', count: tournamentCount },
    { key: 'festival', label: 'Festival', count: festivalCount },
    { key: 'seed', label: 'Seed', count: seedCount, star: true },
    { key: 'tanpa_nik', label: 'Tanpa NIK', count: missingNikCount, warn: missingNikCount > 0 },
  ]);

  let hasActiveFilters = $derived(
    !!searchKeyword.trim() ||
      selectedCategoryFilter !== "all" ||
      selectedContingentFilter !== "all" ||
      selectedPerguruanFilter !== "all" ||
      selectedStatusFilter !== "all"
  );

  function resetFilters() {
    searchKeyword = "";
    selectedCategoryFilter = "all";
    selectedContingentFilter = "all";
    selectedPerguruanFilter = "all";
    selectedStatusFilter = "all";
  }

  function isKataAtlet(a: EnrichedAtletItem): boolean {
    return (a.nomor_tanding_nama || "").toLowerCase().includes("kata");
  }

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

  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-4 pb-4 border-b border-white/[0.08] shrink-0">
    <div class="flex items-center gap-2.5">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">Atlet</h2>
      <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">{totalCount}</span>
    </div>

    <div class="flex items-center gap-2 shrink-0">
      <button
        type="button"
        onclick={handleSyncPull}
        disabled={isSyncing}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 disabled:opacity-50 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        title="Tarik data pendaftaran atlet terbaru dari ambrilindo.com"
      >
        <svg class="w-4 h-4 text-sky-400 {isSyncing ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 13v8m0 0l-3-3m3 3l3-3M20 16.6A5 5 0 0018 7h-1.3A8 8 0 104 15.3" />
        </svg>
        <span>{isSyncing ? "Menarik…" : "Tarik dari web"}</span>
      </button>

      <button
        type="button"
        onclick={openCreateModal}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center gap-1.5 shadow-md cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:ring-offset-2 focus-visible:ring-offset-[#14161b]"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14" /></svg>
        <span>Tambah atlet</span>
      </button>
    </div>
  </div>

  <!-- Status: stat cards that are also the filter -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-2 my-4 shrink-0" role="radiogroup" aria-label="Filter status atlet">
    {#each statusTabs as tab (tab.key)}
      {@const selected = selectedStatusFilter === tab.key}
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        onclick={() => (selectedStatusFilter = tab.key)}
        class="text-left rounded-xl px-3 py-2.5 border transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {selected
          ? 'bg-white/[0.08] border-white/50'
          : tab.warn
            ? 'bg-[#181a20] border-amber-500/30 hover:border-amber-500/50'
            : 'bg-[#181a20] border-white/[0.08] hover:border-white/20'}"
      >
        <span class="flex items-center gap-1.5 text-xs font-semibold {tab.warn ? 'text-amber-300' : 'text-zinc-400'}">
          {#if tab.star}
            <svg class="w-3.5 h-3.5 text-amber-300" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2l3.1 6.3 6.9 1-5 4.9 1.2 6.8L12 17.8 5.8 21l1.2-6.8-5-4.9 6.9-1z" /></svg>
          {:else if tab.warn}
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
          {/if}
          {tab.label}
        </span>
        <span class="block mt-1 text-2xl font-black font-mono tabular-nums {tab.warn ? 'text-amber-300' : 'text-white'}">{tab.count}</span>
      </button>
    {/each}
  </div>

  <!-- Toolbar: search first, then filters -->
  <div class="flex flex-wrap items-center gap-2 mb-3 shrink-0">
    <div class="relative flex-1 min-w-[220px]">
      <svg class="w-4 h-4 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
      <input
        type="text"
        bind:value={searchKeyword}
        placeholder="Cari nama, NIK, atau kode atlet"
        aria-label="Cari atlet"
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

    <FuzzySelect
      items={categoryFilterOptions}
      bind:value={selectedCategoryFilter}
      placeholder="Nomor tanding"
      allowClear={true}
      clearLabel="Semua nomor tanding"
      clearValue="all"
      compact={true}
      class="w-48"
    />
    <FuzzySelect
      items={perguruanFilterOptions}
      bind:value={selectedPerguruanFilter}
      placeholder="Perguruan"
      allowClear={true}
      clearLabel="Semua perguruan"
      clearValue="all"
      compact={true}
      class="w-44"
    />
    <FuzzySelect
      items={contingentFilterOptions}
      bind:value={selectedContingentFilter}
      placeholder="Kontingen"
      allowClear={true}
      clearLabel="Semua kontingen"
      clearValue="all"
      compact={true}
      class="w-44"
    />

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

  <!-- Athletes table -->
  <div class="flex-1 bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden flex flex-col shadow-lg min-h-0">
    {#if isLoading && athletes.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 gap-3" role="status">
        <svg class="w-8 h-8 animate-spin text-zinc-500" fill="none" viewBox="0 0 24 24" aria-hidden="true"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" /><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" /></svg>
        <p class="text-sm text-zinc-400">Memuat data atlet…</p>
      </div>
    {:else if filteredAthletes.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 gap-3 text-center">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-zinc-500" aria-hidden="true">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" /></svg>
        </div>
        {#if athletes.length === 0}
          <p class="text-sm font-bold text-white">Belum ada atlet</p>
          <p class="text-sm text-zinc-400">Tarik pendaftaran dari web atau tambahkan atlet manual.</p>
        {:else}
          <p class="text-sm font-bold text-white">Tidak ada atlet yang cocok</p>
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
              <th scope="col" class="px-4 py-2.5 font-semibold">Atlet</th>
              <th scope="col" class="px-4 py-2.5 font-semibold">Kontingen</th>
              <th scope="col" class="px-4 py-2.5 font-semibold">Nomor tanding</th>
              <th scope="col" class="w-16 px-2 py-2.5 font-semibold text-center">Seed</th>
              <th scope="col" class="w-24 px-4 py-2.5"><span class="sr-only">Aksi</span></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.05]">
            {#each paginatedAthletes as atlet (atlet.id)}
              <tr class="hover:bg-white/[0.025] transition-colors">
                <!-- Atlet -->
                <td class="px-4 py-2.5 align-top">
                  <p class="font-semibold text-white leading-snug">{atlet.nama_atlet}</p>
                  <p class="mt-0.5 text-xs flex flex-wrap items-center gap-x-2">
                    {#if atlet.nik}
                      <span class="font-mono text-zinc-500 tabular-nums">NIK {atlet.nik}</span>
                    {:else}
                      <span class="inline-flex items-center gap-1 text-amber-300">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
                        NIK belum ada
                      </span>
                    {/if}
                    {#if atlet.kode_atlet}<span class="font-mono text-zinc-500">#{atlet.kode_atlet}</span>{/if}
                    {#if atlet.additional_code}<span class="font-mono text-zinc-500">KTA {atlet.additional_code}</span>{/if}
                  </p>
                </td>

                <!-- Kontingen + perguruan -->
                <td class="px-4 py-2.5 align-top">
                  <div class="flex items-center gap-2 min-w-0">
                    {#if atlet.utusan_logo && logoMap[atlet.utusan_logo]}
                      <img src={logoMap[atlet.utusan_logo]} alt="" class="w-5 h-5 object-contain shrink-0 rounded bg-white/[0.04]" />
                    {/if}
                    <span class="text-zinc-100 truncate max-w-[220px]" title={atlet.utusan_nama}>
                      {atlet.utusan_nama || "Kontingen #" + (atlet.utusan_id || "-")}
                    </span>
                  </div>
                  <p class="mt-0.5 text-xs text-zinc-500 truncate max-w-[240px]" title={atlet.perguruan_nama || ""}>
                    {atlet.perguruan_nama || "Tanpa perguruan"}
                  </p>
                </td>

                <!-- Nomor tanding -->
                <td class="px-4 py-2.5 align-top">
                  <p class="text-zinc-100 leading-snug">{atlet.nomor_tanding_nama || "Kategori #" + (atlet.nomor_tanding_id || "-")}</p>
                  <p class="mt-0.5 text-xs text-zinc-500 flex items-center gap-1.5">
                    <span>{isKataAtlet(atlet) ? "Kata" : "Kumite"}</span>
                    <span aria-hidden="true">·</span>
                    {#if isFestival(atlet)}
                      <span class="text-amber-300/90">Festival</span>
                    {:else if atlet.is_bob}
                      <span class="px-1.5 rounded bg-purple-500/15 text-purple-300 border border-purple-500/30 font-semibold">BOB</span>
                    {:else}
                      <span>Prestasi</span>
                    {/if}
                  </p>
                </td>

                <!-- Seed toggle -->
                <td class="px-2 py-2.5 text-center align-top">
                  <button
                    type="button"
                    onclick={() => handleTogglePriority(atlet)}
                    aria-pressed={!!atlet.is_priority}
                    aria-label={atlet.is_priority ? `Hapus seed ${atlet.nama_atlet}` : `Jadikan ${atlet.nama_atlet} seed`}
                    title={atlet.is_priority ? "Seed (klik untuk hapus)" : "Jadikan seed"}
                    class="w-9 h-9 rounded-lg inline-flex items-center justify-center transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-300/50 {atlet.is_priority
                      ? 'text-amber-300 hover:bg-amber-500/10'
                      : 'text-zinc-600 hover:text-zinc-300 hover:bg-white/[0.06]'}"
                  >
                    <svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill={atlet.is_priority ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"><path d="M12 2l3.1 6.3 6.9 1-5 4.9 1.2 6.8L12 17.8 5.8 21l1.2-6.8-5-4.9 6.9-1z" /></svg>
                  </button>
                </td>

                <!-- Actions -->
                <td class="px-4 py-2.5 align-top">
                  <div class="flex items-center justify-end gap-1">
                    <button
                      type="button"
                      onclick={() => openEditModal(atlet)}
                      class="w-9 h-9 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                      title={`Ubah data ${atlet.nama_atlet}`}
                      aria-label={`Ubah data ${atlet.nama_atlet}`}
                    >
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M16.9 4.5l1.6-1.7a1.9 1.9 0 112.7 2.7L10.6 16.1a4.5 4.5 0 01-1.9 1.1L6 18l.8-2.7a4.5 4.5 0 011.1-1.9l9-8.9z" /></svg>
                    </button>
                    <button
                      type="button"
                      onclick={() => openDeleteSingleModal(atlet)}
                      class="w-9 h-9 rounded-lg text-zinc-400 hover:text-rose-300 hover:bg-rose-500/10 flex items-center justify-center transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-400/50"
                      title={`Hapus ${atlet.nama_atlet}`}
                      aria-label={`Hapus ${atlet.nama_atlet}`}
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
          <span>{paginatedAthletes.length > 0 ? (currentPage - 1) * (pageSize === -1 ? 0 : pageSize) + 1 : 0}–{pageSize === -1 ? filteredAthletes.length : Math.min(currentPage * pageSize, filteredAthletes.length)} dari {filteredAthletes.length}</span>
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

<!-- Modal: Tambah / Edit Data Atlet -->
{#if athleteModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-lg shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <!-- Modal Header -->
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight">
          {isEditMode ? "Edit Data Atlet" : "Tambah Atlet Baru"}
        </h3>
        <button
          type="button"
          onclick={() => (athleteModalOpen = false)}
          aria-label="Tutup"
          class="w-8 h-8 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition-colors cursor-pointer"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
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
            <span>
              <span class="block text-sm font-semibold text-zinc-200">Unggulan (seed)</span>
              <span class="block text-xs text-zinc-500">Didahulukan saat drawing</span>
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
