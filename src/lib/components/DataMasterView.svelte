<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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
  let totalAthletes = $state(0);
  let totalCategories = $state(0);
  let totalContingents = $state(0);

  // Modal Confirmations
  let showDeleteBagansModal = $state(false);
  let confirmDeleteBagansChecked = $state(false);
  let isDeletingBagans = $state(false);

  let showDeleteAthletesModal = $state(false);
  let confirmDeleteAthletesChecked = $state(false);
  let isDeletingAthletes = $state(false);

  let showResetAllModal = $state(false);
  let confirmResetAllChecked = $state(false);
  let isResettingAll = $state(false);

  let errorMessage = $state<string | null>(null);

  async function loadStatistics() {
    isLoading = true;
    errorMessage = null;
    try {
      const [baganList, athleteList, categoryList, contingentList] = await Promise.all([
        invoke<BaganItem[]>("get_bagans", { eventId }).catch(() => []),
        invoke<any[]>("get_athletes", { eventId }).catch(() => []),
        invoke<any[]>("get_categories", { eventId }).catch(() => []),
        invoke<any[]>("get_contingents", { eventId }).catch(() => []),
      ]);

      totalBagans = baganList.length;
      totalAthletes = athleteList.length;
      totalCategories = categoryList.length;
      totalContingents = contingentList.length;
    } catch (err: any) {
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal memuat ringkasan data master.";
    } finally {
      isLoading = false;
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

  async function handleResetAll() {
    if (!confirmResetAllChecked) return;
    isResettingAll = true;
    errorMessage = null;
    try {
      await invoke("delete_all_bagans", { eventId });
      const deletedCount = await invoke<number>("delete_all_athletes", { eventId });
      showResetAllModal = false;
      confirmResetAllChecked = false;
      onShowToast(`Reset total selesai: Seluruh bagan dan ${deletedCount} atlet berhasil dibersihkan!`);
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

<div class="h-full flex flex-col p-4 md:p-6 select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans">
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
        class="px-3 py-1.5 bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1] text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95 disabled:opacity-50"
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
        class="text-zinc-400 hover:text-white transition ml-3 p-0.5"
        title="Tutup pesan error"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  {/if}

  <!-- Section 1: Summary Stat Cards -->
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 mb-6 shrink-0">
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
        <p class="text-[11px] text-zinc-500 mt-1">Kategori eliminasi aktif</p>
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
  </div>

  <!-- Section 2: Akses Cepat Data Master (Navigation Cards) -->
  <div class="mb-6">
    <div class="flex items-center justify-between mb-3">
      <h2 class="text-xs font-mono font-bold tracking-wider text-zinc-400 uppercase">
        Pintasan Navigasi Data
      </h2>
      <span class="text-[11px] text-zinc-500">Akses langsung ke channel entitas</span>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
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
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between"
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

      <!-- Nav Card 2: Kategori Nomor Tanding -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "roster",
            id: "categories",
            name: "nomor-tanding",
            subtitle: "Kategori Nomor Pertandingan",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between"
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

      <!-- Nav Card 3: Summary Juara Bagan -->
      <button
        type="button"
        onclick={() =>
          onNavigate({
            type: "summary_bagan",
            id: "summary-bagan",
            name: "summary-bagan",
            subtitle: "Daftar Juara Seluruh Bagan Pertandingan",
          })}
        class="bg-[#181a20] border border-white/[0.08] hover:border-white/[0.2] p-4 rounded-xl text-left transition group active:scale-[0.99] flex flex-col justify-between"
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
    </div>
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
        <!-- Action Row 1: Hapus Semua Bagan -->
        <div class="py-3.5 first:pt-0 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Reset Seluruh Bagan Pertandingan</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                {totalBagans} Bagan Aktif
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus struktur bagan eliminasi, jadwal partai, skor, dan penugasan tatami. Data atlet dan nomor tanding tetap tersimpan untuk pengundian (drawing) ulang.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteBagansChecked = false;
              showDeleteBagansModal = true;
            }}
            disabled={totalBagans === 0 || isLoading}
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
            <span>Hapus Semua Bagan</span>
          </button>
        </div>

        <!-- Action Row 2: Hapus Semua Data Atlet -->
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
            class="shrink-0 px-3.5 py-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 border border-red-500/25 text-xs font-semibold transition flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13 7a4 4 0 11-8 0 4 4 0 018 0zM9 14a6 6 0 00-6 6v1h12v-1a6 6 0 00-6-6zM21 12h-6" />
            </svg>
            <span>Hapus Semua Atlet</span>
          </button>
        </div>

        <!-- Action Row 3: Reset Total Event (Bagan + Atlet) -->
        <div class="py-3.5 last:pb-0 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-xs sm:text-sm font-bold text-white tracking-wide">Reset Total Turnamen (Bagan & Atlet)</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-red-500/15 text-red-400 border border-red-500/30">
                Pembersihan Penuh
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Membersihkan sekaligus seluruh bagan pertandingan dan seluruh data atlet peserta pada event ini. Database kembali ke kondisi awal tanpa perlu membuat event baru.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmResetAllChecked = false;
              showResetAllModal = true;
            }}
            disabled={(totalBagans === 0 && totalAthletes === 0) || isLoading}
            class="shrink-0 px-4 py-2 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-bold transition shadow-md flex items-center space-x-1.5 disabled:opacity-40 disabled:pointer-events-none active:scale-95"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <span>Reset Total Event</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<!-- ================= MODAL 1: KONFIRMASI HAPUS SEMUA BAGAN ================= -->
{#if showDeleteBagansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xs select-none font-sans">
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
          <li><strong>{totalBagans} Bagan Pertandingan</strong></li>
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
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-0 w-4 h-4 cursor-pointer"
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
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllBagans}
          disabled={!confirmDeleteBagansChecked || isDeletingBagans}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-md"
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

<!-- ================= MODAL 2: KONFIRMASI HAPUS SEMUA DATA ATLET ================= -->
{#if showDeleteAthletesModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xs select-none font-sans">
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
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-0 w-4 h-4 cursor-pointer"
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
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllAthletes}
          disabled={!confirmDeleteAthletesChecked || isDeletingAthletes}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-md"
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

<!-- ================= MODAL 3: KONFIRMASI RESET TOTAL ================= -->
{#if showResetAllModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xs select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Reset Total Turnamen</h3>
          <p class="text-xs text-zinc-400">Membersihkan seluruh bagan dan atlet</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/15 border border-red-500/30 text-xs text-zinc-200 leading-relaxed space-y-2">
        <p class="font-bold text-red-300">Peringatan: Seluruh data pertandingan akan dibersihkan!</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-red-200">
          <li><strong>{totalBagans} Bagan</strong> & seluruh jadwal partai pertandingan</li>
          <li><strong>{totalAthletes} Data atlet</strong> & penugasan kontingen</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1 border-t border-red-500/20">
          Gunakan fitur ini apabila Anda ingin memulai turnamen baru dari awal tanpa perlu membuat event baru.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmResetAllChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-0 w-4 h-4 cursor-pointer"
        />
        <span class="text-xs text-red-300 font-bold leading-tight">
          Saya memahami sepenuhnya dan menyetujui pembersihan seluruh bagan & atlet.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2 mt-5">
        <button
          type="button"
          onclick={() => (showResetAllModal = false)}
          disabled={isResettingAll}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleResetAll}
          disabled={!confirmResetAllChecked || isResettingAll}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-1.5 shadow-lg"
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
