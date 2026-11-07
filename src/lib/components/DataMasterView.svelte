<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { ActiveChannel, BaganItem } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    onRefreshEvent: () => void;
    onNavigate: (channel: ActiveChannel) => void;
    onShowToast: (msg: string) => void;
  }

  let { eventId, eventName = "Event Turnamen", onRefreshEvent, onNavigate, onShowToast }: Props = $props();

  // Statistics State
  let isLoading = $state(true);
  let totalBagans = $state(0);
  let totalFestivalBagans = $state(0);
  let totalTournamentBagans = $state(0);
  let totalAthletes = $state(0);
  let totalCategories = $state(0);
  let totalContingents = $state(0);
  let totalPerguruans = $state(0);

  // Modal Confirmations
  let showDeleteFestivalBagansModal = $state(false);
  let confirmDeleteFestivalBagansChecked = $state(false);
  let isDeletingFestivalBagans = $state(false);

  let showDeleteBagansModal = $state(false);
  let confirmDeleteBagansChecked = $state(false);
  let isDeletingBagans = $state(false);

  let showDeleteAthletesModal = $state(false);
  let confirmDeleteAthletesChecked = $state(false);
  let isDeletingAthletes = $state(false);

  let showDeleteCategoriesModal = $state(false);
  let confirmDeleteCategoriesChecked = $state(false);
  let isDeletingCategories = $state(false);

  let showDeleteContingentsModal = $state(false);
  let confirmDeleteContingentsChecked = $state(false);
  let isDeletingContingents = $state(false);

  let showDeletePerguruansModal = $state(false);
  let confirmDeletePerguruansChecked = $state(false);
  let isDeletingPerguruans = $state(false);

  let showResetAllModal = $state(false);
  let confirmResetAllChecked = $state(false);
  let isResettingAll = $state(false);
  let isBackingUpQuick = $state(false);
  let isBackingUpCustom = $state(false);
  let lastBackupPath = $state<string | null>(null);

  async function handleQuickBackup() {
    isBackingUpQuick = true;
    errorMessage = null;
    try {
      const path = await invoke<string>("backup_database");
      lastBackupPath = path;
      const filename = path.split(/[\\/]/).pop() || path;
      onShowToast(`Cadangan database berhasil dibuat: ${filename}`);
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal mencadangkan database.";
    } finally {
      isBackingUpQuick = false;
    }
  }

  async function handleCustomBackup() {
    isBackingUpCustom = true;
    errorMessage = null;
    try {
      const savedPath = await invoke<string | null>("backup_database_to_file");
      if (savedPath) {
        lastBackupPath = savedPath;
        const filename = savedPath.split(/[\\/]/).pop() || savedPath;
        onShowToast(`Cadangan tersimpan di: ${filename}`);
      }
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menyimpan cadangan.";
    } finally {
      isBackingUpCustom = false;
    }
  }

  let errorMessage = $state<string | null>(null);

  async function loadStatistics() {
    isLoading = true;
    errorMessage = null;
    try {
      const [baganList, athleteList, categoryList, contingentList, perguruanList] = await Promise.all([
        invoke<BaganItem[]>("get_bagans", { eventId }).catch(() => []),
        invoke<any[]>("get_athletes", { eventId }).catch(() => []),
        invoke<any[]>("get_categories", { eventId }).catch(() => []),
        invoke<any[]>("get_contingents", { eventId }).catch(() => []),
        invoke<any[]>("get_perguruans", { eventId }).catch(() => []),
      ]);

      totalBagans = baganList.length;
      totalFestivalBagans = baganList.filter((b) =>
        b.id !== 0 &&
        b.kode !== "000" &&
        ((b.nama_bagan || "").toUpperCase().includes("FESTIVAL") || (b.kode || "").toUpperCase().startsWith("F"))
      ).length;
      totalTournamentBagans = Math.max(0, totalBagans - totalFestivalBagans);

      totalAthletes = athleteList.length;
      totalCategories = categoryList.length;
      totalContingents = contingentList.length;
      totalPerguruans = (perguruanList || []).length;
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal memuat ringkasan data master.";
    } finally {
      isLoading = false;
    }
  }

  async function handleDeleteAllFestivalBagans() {
    if (!confirmDeleteFestivalBagansChecked) return;
    isDeletingFestivalBagans = true;
    errorMessage = null;
    try {
      const count = await invoke<number>("delete_all_festival_bagans", { eventId });
      showDeleteFestivalBagansModal = false;
      confirmDeleteFestivalBagansChecked = false;
      onShowToast(`Berhasil menghapus seluruh bagan festival (${count} bagan terhapus)!`);
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menghapus bagan festival.";
    } finally {
      isDeletingFestivalBagans = false;
    }
  }

  async function handleDeleteAllBagans() {
    if (!confirmDeleteBagansChecked) return;
    isDeletingBagans = true;
    errorMessage = null;
    try {
      await invoke("delete_all_bagans", { eventId });
      showDeleteBagansModal = false;
      confirmDeleteBagansChecked = false;
      onShowToast("Seluruh bagan pertandingan dan jadwal partai berhasil dihapus!");
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menghapus bagan.";
    } finally {
      isDeletingBagans = false;
    }
  }

  async function handleDeleteAllAthletes() {
    if (!confirmDeleteAthletesChecked) return;
    isDeletingAthletes = true;
    errorMessage = null;
    try {
      const deletedCount = await invoke<number>("delete_all_athletes", { eventId });
      showDeleteAthletesModal = false;
      confirmDeleteAthletesChecked = false;
      onShowToast(`Berhasil menghapus seluruh data atlet (${deletedCount} atlet terhapus)!`);
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menghapus data atlet.";
    } finally {
      isDeletingAthletes = false;
    }
  }

  interface ResetTournamentSummary {
    deleted_bagans: number;
    deleted_matches: number;
    deleted_athletes: number;
    deleted_categories: number;
    deleted_contingents: number;
    deleted_perguruans: number;
    deleted_tatamis: number;
    deleted_timetable_days: number;
    deleted_wasits: number;
  }

  async function handleDeleteAllCategories() {
    if (!confirmDeleteCategoriesChecked) return;
    isDeletingCategories = true;
    errorMessage = null;
    try {
      const count = await invoke<number>("delete_all_categories", { eventId });
      showDeleteCategoriesModal = false;
      confirmDeleteCategoriesChecked = false;
      onShowToast(`Berhasil menghapus seluruh nomor tanding (${count} kategori terhapus)!`);
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menghapus kategori.";
    } finally {
      isDeletingCategories = false;
    }
  }

  async function handleDeleteAllContingents() {
    if (!confirmDeleteContingentsChecked) return;
    isDeletingContingents = true;
    errorMessage = null;
    try {
      const count = await invoke<number>("delete_all_contingents", { eventId });
      showDeleteContingentsModal = false;
      confirmDeleteContingentsChecked = false;
      onShowToast(`Berhasil menghapus seluruh kontingen (${count} kontingen terhapus)!`);
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menghapus kontingen.";
    } finally {
      isDeletingContingents = false;
    }
  }

  async function handleDeleteAllPerguruans() {
    if (!confirmDeletePerguruansChecked) return;
    isDeletingPerguruans = true;
    errorMessage = null;
    try {
      const count = await invoke<number>("delete_all_perguruans", { eventId });
      showDeletePerguruansModal = false;
      confirmDeletePerguruansChecked = false;
      onShowToast(`Berhasil menghapus seluruh perguruan (${count} perguruan terhapus)!`);
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal menghapus perguruan.";
    } finally {
      isDeletingPerguruans = false;
    }
  }

  async function handleResetAll() {
    if (!confirmResetAllChecked) return;
    isResettingAll = true;
    errorMessage = null;
    try {
      const summary = await invoke<ResetTournamentSummary>("reset_total_tournament", { eventId });
      showResetAllModal = false;
      confirmResetAllChecked = false;
      onShowToast(
        `Reset total selesai: Seluruh bagan, ${summary.deleted_athletes} atlet, ${summary.deleted_categories} kategori, ${summary.deleted_contingents} kontingen, dan ${summary.deleted_perguruans} perguruan berhasil dibersihkan!`
      );
      await loadStatistics();
      onRefreshEvent();
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal melakukan reset total.";
    } finally {
      isResettingAll = false;
    }
  }

  onMount(() => {
    loadStatistics();
  });
</script>

<div class="h-full flex flex-col p-4 md:p-6 pb-12 select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans">
  <!-- Top Bar -->
  <div class="flex items-center justify-between pb-4 mb-5 border-b border-white/[0.08] shrink-0">
    <div>
      <div class="flex items-center space-x-2.5">
        <span class="w-2.5 h-2.5 rounded-full bg-indigo-500"></span>
        <h1 class="text-sm md:text-base font-black tracking-tight text-white uppercase">
          Data Master & Operasional Event
        </h1>
        <span class="text-xs text-zinc-500 font-mono">
          {eventName}
        </span>
      </div>
      <p class="text-[11px] text-zinc-400 mt-0.5">
        Ringkasan entitas turnamen, pintasan navigasi data, dan manajemen pembersihan basis data
      </p>
    </div>

    <!-- Right Actions -->
    <div class="flex items-center space-x-2">
      <button
        type="button"
        onclick={loadStatistics}
        disabled={isLoading}
        class="px-3 py-1.5 bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1] text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95 disabled:opacity-50 cursor-pointer"
        title="Segarkan data ringkasan"
      >
        <svg
          class="w-3.5 h-3.5 text-zinc-400 {isLoading ? 'animate-spin' : ''}"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
          />
        </svg>
        <span>Segarkan</span>
      </button>
    </div>
  </div>

  <!-- Error Alert Banner -->
  {#if errorMessage}
    <div class="mb-5 p-3.5 rounded-xl bg-red-500/10 border border-red-500/25 text-red-300 text-xs flex items-start justify-between">
      <div class="flex items-start space-x-2.5">
        <svg class="w-4 h-4 text-red-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <span class="leading-relaxed">{errorMessage}</span>
      </div>
      <button
        onclick={() => (errorMessage = null)}
        class="text-zinc-400 hover:text-white transition ml-3 p-0.5 cursor-pointer"
        title="Tutup pesan error"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  {/if}

  <!-- Section 1: Summary Stat Cards (Balanced 5 Columns across Widescreen) -->
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3 mb-6 shrink-0">
    <!-- Card 1: Bagan Pertandingan -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3.5 flex items-center justify-between hover:border-white/[0.15] transition shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Bagan Pertandingan</span>
        <div class="flex items-baseline space-x-1.5 mt-1">
          <span class="text-2xl font-black font-mono text-white">
            {#if isLoading}
              <span class="text-zinc-600 text-lg">...</span>
            {:else}
              {totalBagans}
            {/if}
          </span>
          <span class="text-xs text-zinc-500 font-medium">Bagan</span>
        </div>
        <p class="text-[11px] text-zinc-400 mt-1">
          {#if isLoading}
            Kategori eliminasi aktif
          {:else}
            <span class="text-emerald-400 font-bold">{totalTournamentBagans}</span> Prestasi • <span class="text-amber-400 font-bold">{totalFestivalBagans}</span> Festival
          {/if}
        </p>
      </div>
      <div class="w-10 h-10 rounded-xl bg-amber-500/10 text-amber-400 border border-amber-500/20 flex items-center justify-center shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h4v4H4V6zm12 0h4v4h-4V6zM4 14h4v4H4v-4zm12 0h4v4h-4v-4zm-8-5h8M8 17h8" />
        </svg>
      </div>
    </div>

    <!-- Card 2: Total Peserta Atlet -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3.5 flex items-center justify-between hover:border-white/[0.15] transition shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Atlet Terdaftar</span>
        <div class="flex items-baseline space-x-1.5 mt-1">
          <span class="text-2xl font-black font-mono text-white">
            {#if isLoading}
              <span class="text-zinc-600 text-lg">...</span>
            {:else}
              {totalAthletes}
            {/if}
          </span>
          <span class="text-xs text-zinc-500 font-medium">Peserta</span>
        </div>
        <p class="text-[11px] text-zinc-500 mt-1">Roster terdaftar di event</p>
      </div>
      <div class="w-10 h-10 rounded-xl bg-sky-500/10 text-sky-400 border border-sky-500/20 flex items-center justify-center shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
        </svg>
      </div>
    </div>

    <!-- Card 3: Nomor Tanding / Kategori -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3.5 flex items-center justify-between hover:border-white/[0.15] transition shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Nomor Tanding</span>
        <div class="flex items-baseline space-x-1.5 mt-1">
          <span class="text-2xl font-black font-mono text-white">
            {#if isLoading}
              <span class="text-zinc-600 text-lg">...</span>
            {:else}
              {totalCategories}
            {/if}
          </span>
          <span class="text-xs text-zinc-500 font-medium">Kategori</span>
        </div>
        <p class="text-[11px] text-zinc-500 mt-1">Kata & Kumite terdaftar</p>
      </div>
      <div class="w-10 h-10 rounded-xl bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center justify-center shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
        </svg>
      </div>
    </div>

    <!-- Card 4: Kontingen & Utusan -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3.5 flex items-center justify-between hover:border-white/[0.15] transition shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Kontingen & Utusan</span>
        <div class="flex items-baseline space-x-1.5 mt-1">
          <span class="text-2xl font-black font-mono text-white">
            {#if isLoading}
              <span class="text-zinc-600 text-lg">...</span>
            {:else}
              {totalContingents}
            {/if}
          </span>
          <span class="text-xs text-zinc-500 font-medium">Utusan</span>
        </div>
        <p class="text-[11px] text-zinc-500 mt-1">Dojo & tim daerah</p>
      </div>
      <div class="w-10 h-10 rounded-xl bg-purple-500/10 text-purple-400 border border-purple-500/20 flex items-center justify-center shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
        </svg>
      </div>
    </div>

    <!-- Card 5: Perguruan Karate FORKI (Distinct Indigo Theme) -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3.5 flex items-center justify-between hover:border-white/[0.15] transition shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Perguruan Karate</span>
        <div class="flex items-baseline space-x-1.5 mt-1">
          <span class="text-2xl font-black font-mono text-white">
            {#if isLoading}
              <span class="text-zinc-600 text-lg">...</span>
            {:else}
              {totalPerguruans}
            {/if}
          </span>
          <span class="text-xs text-zinc-500 font-medium">FORKI</span>
        </div>
        <p class="text-[11px] text-zinc-500 mt-1">Aliran & perguruan</p>
      </div>
      <div class="w-10 h-10 rounded-xl bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 flex items-center justify-center shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
      </div>
    </div>
  </div>

  <!-- Section 2: Akses Cepat Data Master (Symmetrical 8-Card Grid) -->
  <div class="mb-6">
    <div class="flex items-center justify-between mb-3">
      <h2 class="text-xs font-mono font-bold tracking-wider text-zinc-400 uppercase">
        Pintasan Navigasi Data & Operasional
      </h2>
      <span class="text-[11px] text-zinc-500">Akses langsung ke channel entitas turnamen</span>
    </div>

    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
      <!-- Nav Card 1: Data Atlet -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "roster",
            id: "athletes",
            name: "data-atlet",
            subtitle: "Daftar Atlet Peserta",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-sky-500/10 border border-sky-500/20 text-sky-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-sky-500/10 text-sky-400 border border-sky-500/20">
              {totalAthletes} Atlet
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-sky-300 transition">
            Data Atlet Peserta
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Kelola data atlet, penomoran kode, verifikasi NIK, dan seed prioritas turnamen.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-sky-400 group-hover:text-sky-300 space-x-1">
          <span>Buka Data Atlet</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 2: Kontingen & Utusan -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "contingent",
            id: "contingents",
            name: "kontingen",
            subtitle: "Master Data Kontingen & Logo Official",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-purple-500/10 border border-purple-500/20 text-purple-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-purple-500/10 text-purple-400 border border-purple-500/20">
              {totalContingents} Utusan
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-purple-300 transition">
            Kontingen & Utusan
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Kelola kontingen dojo/daerah, unggah logo official untuk scoring board & bagan.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-purple-400 group-hover:text-purple-300 space-x-1">
          <span>Buka Kontingen</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 3: Perguruan Karate FORKI -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "perguruan",
            id: "perguruans",
            name: "perguruan",
            subtitle: "Master Data Perguruan Karate FORKI",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-indigo-500/10 border border-indigo-500/20 text-indigo-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
              {totalPerguruans} FORKI
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-indigo-300 transition">
            Perguruan Karate (FORKI)
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Daftar perguruan karate FORKI (INKAI, LEMKARI, KKI, INKANAS, dll.) dan afiliasi atlet.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-indigo-400 group-hover:text-indigo-300 space-x-1">
          <span>Buka Perguruan</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 4: Kategori Nomor Tanding -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "nomor_tanding",
            id: "categories",
            name: "nomor-tanding",
            subtitle: "Kategori Nomor Pertandingan",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              {totalCategories} Nomor
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-emerald-300 transition">
            Nomor Pertandingan
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Daftar kategori kelas Kata & Kumite, pembagian kelompok umur, dan berat badan.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-emerald-400 group-hover:text-emerald-300 space-x-1">
          <span>Buka Nomor Tanding</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 5: Wasit & Dewan Juri (NEW) -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "wasit",
            id: "wasit-manager",
            name: "wasit-juri",
            subtitle: "Manajemen Wasit, Penugasan Panel Tatami & Deteksi Bentrok",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-rose-500/10 text-rose-400 border border-rose-500/20">
              Panel Wasit
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-rose-300 transition">
            Wasit & Dewan Juri
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Kelola data wasit juri, penugasan panel tatami, dan deteksi otomatis anti-bentrok daerah.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-rose-400 group-hover:text-rose-300 space-x-1">
          <span>Buka Wasit & Juri</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 6: Jadwal Pertandingan / Timetable (NEW) -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "timetable",
            id: "timetable-editor",
            name: "jadwal-pertandingan",
            subtitle: "Editor Matriks Jadwal Pertandingan Per Tatami",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-cyan-500/10 border border-cyan-500/20 text-cyan-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">
              Timetable
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-cyan-300 transition">
            Jadwal Pertandingan
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Matriks alokasi nomor tanding per tatami dan penjadwalan estimasi waktu partai.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-cyan-400 group-hover:text-cyan-300 space-x-1">
          <span>Buka Jadwal</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 7: Summary Juara Bagan -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "summary_bagan",
            id: "summary-bagan",
            name: "summary-bagan",
            subtitle: "Daftar Juara Seluruh Bagan Pertandingan",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 18.75h-9m9 0a3 3 0 013 3h-15a3 3 0 013-3m9 0v-3.375c0-.621-.504-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872m5.003 0V7.875A3.375 3.375 0 0011.25 4.5h-2.25A3.375 3.375 0 005.625 7.875v6.375" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
              {totalBagans} Bagan
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-amber-300 transition">
            Daftar Juara Bagan
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Rekapitulasi peraih medali juara 1, 2, 3 untuk seluruh bagan pertandingan.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-amber-400 group-hover:text-amber-300 space-x-1">
          <span>Buka Summary Bagan</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>

      <!-- Nav Card 8: Buku Hasil Turnamen (NEW) -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "buku_hasil",
            id: "results-booklet",
            name: "buku-hasil-turnamen",
            subtitle: "Buku Hasil Resmi & Rekapitulasi Medali Turnamen",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between cursor-pointer"
      >
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="w-8 h-8 rounded-lg bg-orange-500/10 border border-orange-500/20 text-orange-400 flex items-center justify-center">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-orange-500/10 text-orange-400 border border-orange-500/20">
              Laporan Resmi
            </span>
          </div>
          <h3 class="text-xs font-bold text-white group-hover:text-orange-300 transition">
            Buku Hasil Turnamen
          </h3>
          <p class="text-[11px] text-zinc-400 mt-1 leading-relaxed">
            Kompilasi hasil akhir, klasemen medali umum, dan ekspor PDF resmi kejuaraan.
          </p>
        </div>
        <div class="mt-3 pt-3 border-t border-white/[0.06] flex items-center text-[11px] font-medium text-orange-400 group-hover:text-orange-300 space-x-1">
          <span>Buka Buku Hasil</span>
          <svg class="w-3.5 h-3.5 transform group-hover:translate-x-0.5 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>
    </div>
  </div>

  <!-- Section: Cadangan & Keamanan Database (With Proper Bottom Margin) -->
  <div class="rounded-2xl border border-emerald-500/20 bg-emerald-950/10 p-5 sm:p-6 shadow-xl relative overflow-hidden mb-6">
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
      <div class="flex items-start space-x-3.5 min-w-0">
        <div class="w-10 h-10 rounded-xl bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 flex items-center justify-center shrink-0 mt-0.5">
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
        </div>
        <div class="min-w-0">
          <div class="flex items-center space-x-2 flex-wrap gap-y-1">
            <h2 class="text-sm sm:text-base font-bold text-white tracking-wide">Pencadangan Database Turnamen (.sqlite3)</h2>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
              Snapshot Mandiri
            </span>
          </div>
          <p class="text-xs text-zinc-400 mt-1 leading-relaxed max-w-2xl">
            Duplikasi seluruh data pertandingan, skor, bagan, dan atlet ke file SQLite terpisah dengan proteksi WAL checkpoint. Lindungi turnamen dari risiko kehilangan data akibat laptop mati atau force close.
          </p>
        </div>
      </div>

      <div class="flex items-center gap-2.5 shrink-0 flex-wrap sm:flex-nowrap">
        <!-- 1-Click Quick Backup -->
        <button
          type="button"
          onclick={handleQuickBackup}
          disabled={isBackingUpQuick || isBackingUpCustom}
          class="px-4 py-2.5 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-bold transition flex items-center space-x-2 shadow-lg active:scale-95 disabled:opacity-50 cursor-pointer"
          title="Buat snapshot instan dengan timestamp ke folder backups"
        >
          {#if isBackingUpQuick}
            <svg class="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
            </svg>
            <span>Mencadangkan...</span>
          {:else}
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
            <span>Cadangkan Cepat (1-Klik)</span>
          {/if}
        </button>

        <!-- Custom Destination / Save As -->
        <button
          type="button"
          onclick={handleCustomBackup}
          disabled={isBackingUpQuick || isBackingUpCustom}
          class="px-3.5 py-2.5 rounded-xl bg-white/[0.05] hover:bg-white/[0.1] text-zinc-300 hover:text-white border border-white/10 text-xs font-semibold transition flex items-center space-x-2 active:scale-95 disabled:opacity-50 cursor-pointer"
          title="Simpan cadangan ke lokasi tertentu, misal Flashdisk USB"
        >
          {#if isBackingUpCustom}
            <span>Menyimpan...</span>
          {:else}
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
            </svg>
            <span>Pilih Lokasi Simpan...</span>
          {/if}
        </button>
      </div>
    </div>

    {#if lastBackupPath}
      <div class="mt-3 p-2.5 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-xs text-emerald-300 flex items-center gap-2 font-mono min-w-0">
        <svg class="w-4 h-4 shrink-0 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
        </svg>
        <span class="truncate" title={lastBackupPath}>Snapshot tersimpan: {lastBackupPath}</span>
      </div>
    {/if}
  </div>

  <!-- Section 3: Danger Zone / Reset & Pembersihan Data -->
  <div>
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center space-x-2">
        <svg class="w-4 h-4 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <h2 class="text-xs font-mono font-bold tracking-wider text-red-400 uppercase">
          Operasi Pembersihan & Reset Data
        </h2>
      </div>
      <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-red-500/10 text-red-400 border border-red-500/20">
        Zona Operasi Kritis
      </span>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden shadow-xs">
      <!-- Notice Bar -->
      <div class="p-3.5 bg-red-500/[0.04] border-b border-red-500/15 text-xs text-zinc-400 leading-relaxed flex items-center space-x-2.5">
        <svg class="w-4 h-4 text-red-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <span>
          Tindakan pada bagian ini mengubah basis data lokal secara langsung dan permanen. Pastikan pertandingan telah selesai atau data sudah dicadangkan sebelum mengeksekusi reset.
        </span>
      </div>

      <div class="divide-y divide-white/[0.06] p-4 sm:p-5">
        <!-- Action Row 1: Hapus Semua Bagan Festival (NEW) -->
        <div class="py-3.5 first:pt-0 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Hapus Khusus Bagan Festival</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/15 text-amber-400 border border-amber-500/30">
                {totalFestivalBagans} Bagan Festival Aktif
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh bagan pertandingan kategori festival beserta jadwal partai, skor, dan data juara festival. Bagan Prestasi/Open, data atlet, dan nomor tanding tetap aman tersimpan.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteFestivalBagansChecked = false;
              showDeleteFestivalBagansModal = true;
            }}
            disabled={totalFestivalBagans === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-amber-500/10 hover:bg-amber-500/20 text-amber-400 hover:text-amber-300 border border-amber-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
            <span>Hapus Bagan Festival</span>
          </button>
        </div>

        <!-- Action Row 2: Reset Seluruh Bagan Pertandingan (Prestasi + Festival) -->
        <div class="py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Reset Seluruh Bagan Pertandingan</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                {totalBagans} Bagan Aktif ({totalTournamentBagans} Open + {totalFestivalBagans} Fest)
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh struktur bagan (Prestasi & Festival), jadwal partai, skor, dan penugasan tatami. Data atlet dan nomor tanding tetap tersimpan untuk pengundian (drawing) ulang.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteBagansChecked = false;
              showDeleteBagansModal = true;
            }}
            disabled={totalBagans === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
            <span>Hapus Semua Bagan</span>
          </button>
        </div>

        <!-- Action Row 3: Hapus Semua Data Atlet -->
        <div class="py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Hapus Seluruh Data Atlet</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-sky-500/10 text-sky-400 border border-sky-500/20">
                {totalAthletes} Atlet Terdaftar
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh peserta pada event ini dan membersihkan referensi atlet pada partai bagan. Dianjurkan sebelum melakukan penarikan ulang (<span class="font-mono text-zinc-300">re-sync</span>) data atlet baru dari Web Publik.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteAthletesChecked = false;
              showDeleteAthletesModal = true;
            }}
            disabled={totalAthletes === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13 7a4 4 0 11-8 0 4 4 0 018 0zM9 14a6 6 0 00-6 6v1h12v-1a6 6 0 00-6-6zM21 12h-6" />
            </svg>
            <span>Hapus Semua Atlet</span>
          </button>
        </div>

        <!-- Action Row 4: Hapus Seluruh Nomor Tanding (Kategori) -->
        <div class="py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Hapus Seluruh Nomor Tanding (Kategori)</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                {totalCategories} Kategori Terdaftar
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh kelas nomor tanding pada event ini beserta jadwal partai dan bagan terkait. Dianjurkan sebelum mengimpor ulang daftar kategori baru.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteCategoriesChecked = false;
              showDeleteCategoriesModal = true;
            }}
            disabled={totalCategories === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
            <span>Hapus Semua Kategori</span>
          </button>
        </div>

        <!-- Action Row 5: Hapus Seluruh Kontingen (Utusan) -->
        <div class="py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Hapus Seluruh Kontingen (Utusan)</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                {totalContingents} Kontingen Terdaftar
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh daftar kontingen/dojo pada event ini. Relasi atlet dengan kontingen akan dikosongkan.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteContingentsChecked = false;
              showDeleteContingentsModal = true;
            }}
            disabled={totalContingents === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
            </svg>
            <span>Hapus Semua Kontingen</span>
          </button>
        </div>

        <!-- Action Row 6: Hapus Seluruh Perguruan -->
        <div class="py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Hapus Seluruh Perguruan</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-purple-500/10 text-purple-400 border border-purple-500/20">
                {totalPerguruans} Perguruan Terdaftar
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh daftar perguruan karate pada event ini. Relasi atlet dan wasit dengan perguruan akan dikosongkan.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeletePerguruansChecked = false;
              showDeletePerguruansModal = true;
            }}
            disabled={totalPerguruans === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
            </svg>
            <span>Hapus Semua Perguruan</span>
          </button>
        </div>

        <!-- Action Row 7: Reset Total Turnamen (Pembersihan Menyeluruh) -->
        <div class="p-4 rounded-2xl bg-gradient-to-r from-red-950/40 via-red-900/20 to-black/40 border border-red-500/40 flex flex-col md:flex-row md:items-center justify-between gap-4 mt-2">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <div class="w-6 h-6 rounded-lg bg-red-600/30 border border-red-500/50 flex items-center justify-center text-red-400 text-xs font-black">
                ⚠️
              </div>
              <h3 class="text-sm font-extrabold text-white tracking-wide">Reset Total Turnamen (Pembersihan Menyeluruh)</h3>
              <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded-full bg-red-600 text-white shadow-sm">
                FULL WIPE
              </span>
            </div>
            <p class="text-xs text-zinc-300 mt-1.5 leading-relaxed">
              Mereset total seluruh data pada turnamen ini: <strong>bagan pertandingan</strong>, seluruh <strong>data atlet</strong>, <strong>nomor tanding / kategori</strong>, <strong>kontingen</strong>, <strong>perguruan</strong>, jadwal tatami, penugasan wasit/juri, kop surat, dan berkas festival. Event kembali bersih ke kondisi awal siap pakai.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmResetAllChecked = false;
              showResetAllModal = true;
            }}
            disabled={(totalBagans === 0 && totalAthletes === 0 && totalCategories === 0 && totalContingents === 0 && totalPerguruans === 0) || isLoading}
            class="shrink-0 px-5 py-2.5 rounded-xl bg-gradient-to-r from-red-600 to-red-700 hover:from-red-500 hover:to-red-600 text-white text-xs font-black transition shadow-lg shadow-red-600/30 flex items-center space-x-2 disabled:opacity-40 disabled:pointer-events-none active:scale-95 cursor-pointer"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <span>Reset Total Turnamen</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<!-- ================= MODAL 1: KONFIRMASI HAPUS SEMUA BAGAN FESTIVAL (NEW) ================= -->
{#if showDeleteFestivalBagansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-amber-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-amber-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-amber-500/10 border border-amber-500/20 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Konfirmasi Hapus Bagan Festival</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-xs text-zinc-300 leading-relaxed space-y-2">
        <p class="font-medium text-amber-300">Data festival berikut akan dihapus secara permanen:</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-zinc-300">
          <li><strong>{totalFestivalBagans} Bagan Festival</strong> & seluruh kuadran pool</li>
          <li>Seluruh jadwal partai, skor pertandingan, & data juara festival</li>
          <li>Seluruh penugasan partai festival pada tatami aktif</li>
        </ul>
        <p class="text-[11px] text-emerald-400 pt-1 border-t border-amber-500/15 flex items-center gap-1.5">
          <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
          </svg>
          <span>Bagan Prestasi/Open ({totalTournamentBagans} bagan) & data atlet tetap aman tersimpan.</span>
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteFestivalBagansChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-amber-500 focus:ring-2 focus:ring-amber-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-zinc-300 font-medium leading-tight">
          Saya memahami dan menyetujui penghapusan seluruh bagan festival pada event ini.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteFestivalBagansModal = false)}
          disabled={isDeletingFestivalBagans}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllFestivalBagans}
          disabled={!confirmDeleteFestivalBagansChecked || isDeletingFestivalBagans}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-amber-600 hover:bg-amber-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-md cursor-pointer"
        >
          {#if isDeletingFestivalBagans}
            <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            <span>Menghapus...</span>
          {:else}
            <span>Hapus Festival Sekarang</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 2: KONFIRMASI HAPUS SEMUA BAGAN (SEMUA) ================= -->
{#if showDeleteBagansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Konfirmasi Reset Semua Bagan</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/10 border border-red-500/20 text-xs text-zinc-300 leading-relaxed space-y-2">
        <p class="font-medium text-red-300">Data berikut akan dihapus secara permanen:</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-zinc-300">
          <li><strong>{totalBagans} Bagan Pertandingan</strong> (Prestasi & Festival)</li>
          <li>Seluruh jadwal partai, skor pertandingan, & data juara</li>
          <li>Seluruh penugasan partai pada tatami aktif</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1 border-t border-red-500/15">
          Data atlet dan kategori nomor tanding tetap tersimpan di database.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteBagansChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-2 focus:ring-red-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-zinc-300 font-medium leading-tight">
          Saya memahami dan menyetujui penghapusan seluruh bagan pertandingan pada event ini.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteBagansModal = false)}
          disabled={isDeletingBagans}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllBagans}
          disabled={!confirmDeleteBagansChecked || isDeletingBagans}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-md cursor-pointer"
        >
          {#if isDeletingBagans}
            <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            <span>Menghapus...</span>
          {:else}
            <span>Hapus Sekarang</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 3: KONFIRMASI HAPUS SEMUA DATA ATLET ================= -->
{#if showDeleteAthletesModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 7a4 4 0 11-8 0 4 4 0 018 0zM9 14a6 6 0 00-6 6v1h12v-1a6 6 0 00-6-6zM21 12h-6" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Konfirmasi Hapus Semua Data Atlet</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/10 border border-red-500/20 text-xs text-zinc-300 leading-relaxed space-y-2">
        <p class="font-medium text-red-300">Data berikut akan dihapus secara permanen:</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-zinc-300">
          <li><strong>{totalAthletes} Atlet / Peserta</strong> pada event ini</li>
          <li>Tautan atlet pada susunan partai bagan pertandingan</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1 border-t border-red-500/15">
          Setelah pembersihan, Anda dapat menarik kembali pendaftaran baru via menu sinkronisasi web.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteAthletesChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-2 focus:ring-red-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-zinc-300 font-medium leading-tight">
          Saya memahami dan menyetujui penghapusan seluruh data atlet turnamen ini.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteAthletesModal = false)}
          disabled={isDeletingAthletes}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllAthletes}
          disabled={!confirmDeleteAthletesChecked || isDeletingAthletes}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-md cursor-pointer"
        >
          {#if isDeletingAthletes}
            <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            <span>Menghapus...</span>
          {:else}
            <span>Hapus Atlet</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL: HAPUS SEMUA KATEGORI ================= -->
{#if showDeleteCategoriesModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus Seluruh Nomor Tanding</h3>
          <p class="text-xs text-zinc-400">Menghapus semua kelas kategori nomor tanding</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/15 border border-red-500/30 text-xs text-zinc-200 leading-relaxed space-y-2">
        <p class="font-bold text-red-300">Peringatan: Seluruh nomor tanding akan dihapus!</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-red-200">
          <li><strong>{totalCategories} Nomor Tanding / Kategori</strong> kelas pertandingan</li>
          <li>Seluruh jadwal bagan & partai yang bergantung pada kategori ini</li>
        </ul>
      </div>

      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteCategoriesChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-2 focus:ring-red-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-red-300 font-bold leading-tight">
          Saya memahami dan menyetujui penghapusan seluruh nomor tanding ({totalCategories} kelas).
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteCategoriesModal = false)}
          disabled={isDeletingCategories}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllCategories}
          disabled={!confirmDeleteCategoriesChecked || isDeletingCategories}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-lg cursor-pointer"
        >
          {#if isDeletingCategories}
            <span>Menghapus...</span>
          {:else}
            <span>Hapus Kategori</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL: HAPUS SEMUA KONTINGEN ================= -->
{#if showDeleteContingentsModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus Seluruh Kontingen</h3>
          <p class="text-xs text-zinc-400">Menghapus semua kontingen / dojo pada event ini</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/15 border border-red-500/30 text-xs text-zinc-200 leading-relaxed space-y-2">
        <p class="font-bold text-red-300">Peringatan: Seluruh data kontingen akan dihapus!</p>
        <p class="font-mono text-[11px] text-red-200">
          Total: <strong>{totalContingents} Kontingen / Utusan</strong>
        </p>
        <p class="text-[11px] text-zinc-400">
          Relasi kontingen pada atlet yang masih terdaftar akan dikosongkan.
        </p>
      </div>

      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteContingentsChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-2 focus:ring-red-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-red-300 font-bold leading-tight">
          Saya memahami dan menyetujui penghapusan seluruh kontingen ({totalContingents} kontingen).
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteContingentsModal = false)}
          disabled={isDeletingContingents}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllContingents}
          disabled={!confirmDeleteContingentsChecked || isDeletingContingents}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-lg cursor-pointer"
        >
          {#if isDeletingContingents}
            <span>Menghapus...</span>
          {:else}
            <span>Hapus Kontingen</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL: HAPUS SEMUA PERGURUAN ================= -->
{#if showDeletePerguruansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus Seluruh Perguruan</h3>
          <p class="text-xs text-zinc-400">Menghapus semua perguruan karate pada event ini</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/15 border border-red-500/30 text-xs text-zinc-200 leading-relaxed space-y-2">
        <p class="font-bold text-red-300">Peringatan: Seluruh data perguruan akan dihapus!</p>
        <p class="font-mono text-[11px] text-red-200">
          Total: <strong>{totalPerguruans} Perguruan Karate</strong>
        </p>
        <p class="text-[11px] text-zinc-400">
          Relasi perguruan pada atlet dan wasit akan dikosongkan.
        </p>
      </div>

      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeletePerguruansChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-2 focus:ring-red-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-red-300 font-bold leading-tight">
          Saya memahami dan menyetujui penghapusan seluruh perguruan ({totalPerguruans} perguruan).
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeletePerguruansModal = false)}
          disabled={isDeletingPerguruans}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllPerguruans}
          disabled={!confirmDeletePerguruansChecked || isDeletingPerguruans}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-lg cursor-pointer"
        >
          {#if isDeletingPerguruans}
            <span>Menghapus...</span>
          {:else}
            <span>Hapus Perguruan</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 4: KONFIRMASI RESET TOTAL ================= -->
{#if showResetAllModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-lg rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Reset Total Turnamen (Pembersihan Penuh)</h3>
          <p class="text-xs text-zinc-400">Membersihkan seluruh bagan, atlet, kategori, kontingen, perguruan & jadwal</p>
        </div>
      </div>

      <div class="my-4 p-3.5 rounded-xl bg-red-500/15 border border-red-500/30 text-xs text-zinc-200 leading-relaxed space-y-2.5">
        <p class="font-bold text-red-300">Peringatan: Seluruh data master turnamen ini akan dibersihkan total!</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-red-200">
          <li><strong>{totalBagans} Bagan Pertandingan</strong> & seluruh jadwal partai pertandingan</li>
          <li><strong>{totalAthletes} Data Atlet</strong> terdaftar</li>
          <li><strong>{totalCategories} Nomor Tanding / Kategori</strong> kelas pertandingan</li>
          <li><strong>{totalContingents} Kontingen / Utusan</strong></li>
          <li><strong>{totalPerguruans} Perguruan Karate</strong></li>
          <li>Seluruh jadwal tatami, penugasan wasit/juri, kop surat & berkas festival</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1 border-t border-red-500/20">
          Event ini akan kembali bersih ke kondisi awal tanpa perlu membuat event baru. Tindakan ini tidak dapat dibatalkan.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmResetAllChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-2 focus:ring-red-500/40 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-red-300 font-bold leading-tight">
          Saya memahami sepenuhnya dan menyetujui reset total seluruh data turnamen ini (Bagan, Atlet, Kategori, Kontingen, Perguruan, Tatami & Jadwal).
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showResetAllModal = false)}
          disabled={isResettingAll}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleResetAll}
          disabled={!confirmResetAllChecked || isResettingAll}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-lg cursor-pointer"
        >
          {#if isResettingAll}
            <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            <span>Mereset Total...</span>
          {:else}
            <span>Reset Total Sekarang</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
