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
  let totalMatches = $state(0);

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
      errorMessage = typeof err === "string" ? err : err.message || "Gagal memuat ringkasan data master.";
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
      errorMessage = typeof err === "string" ? err : err.message || "Gagal menghapus bagan.";
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
      errorMessage = typeof err === "string" ? err : err.message || "Gagal menghapus data atlet.";
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
      errorMessage = typeof err === "string" ? err : err.message || "Gagal melakukan reset total.";
    } finally {
      isResettingAll = false;
    }
  }

  onMount(() => {
    loadStatistics();
  });
</script>

<div class="h-full flex flex-col p-6 overflow-y-auto select-none font-sans bg-[#101216] text-zinc-200">
  <!-- Header Bar -->
  <div class="flex items-center justify-between pb-4 border-b border-white/[0.08] mb-6 shrink-0">
    <div>
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-red-500/10 border border-red-500/20 text-red-400 flex items-center justify-center text-lg font-bold">
          ⚙️
        </div>
        <div>
          <h1 class="text-lg font-extrabold text-white tracking-wide flex items-center gap-2">
            Manajemen Data Master
            <span class="text-xs px-2 py-0.5 rounded-full bg-white/[0.06] text-zinc-400 font-normal border border-white/[0.08]">
              Admin Tools
            </span>
          </h1>
          <p class="text-xs text-zinc-400 mt-0.5">
            Kontrol basis data, reset bagan pertandingan, dan pembersihan data peserta turnamen untuk <span class="text-white font-semibold">{eventName}</span>.
          </p>
        </div>
      </div>
    </div>

    <!-- Refresh Button -->
    <button
      onclick={loadStatistics}
      disabled={isLoading}
      class="px-3 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] active:scale-95 border border-white/[0.1] text-xs font-semibold text-zinc-200 flex items-center space-x-1.5 transition disabled:opacity-50"
      title="Segarkan data ringkasan"
    >
      <span class={isLoading ? "animate-spin inline-block" : ""}>🔄</span>
      <span>Segarkan</span>
    </button>
  </div>

  <!-- Error Alert Banner -->
  {#if errorMessage}
    <div class="mb-6 p-4 rounded-xl bg-red-500/10 border border-red-500/30 text-red-300 text-xs flex items-start justify-between">
      <div class="flex items-start space-x-2">
        <span class="text-base shrink-0">⚠️</span>
        <span class="leading-relaxed">{errorMessage}</span>
      </div>
      <button onclick={() => (errorMessage = null)} class="text-red-400 hover:text-red-200 font-bold ml-4">✕</button>
    </div>
  {/if}

  <!-- Section 1: Ringkasan Entitas Master -->
  <div class="mb-8">
    <div class="flex items-center justify-between mb-3">
      <h2 class="text-xs font-mono font-bold tracking-wider text-zinc-400 uppercase">
        Ringkasan Data Turnamen Saat Ini
      </h2>
      <span class="text-[11px] text-zinc-500">Database Lokal SQLite</span>
    </div>

    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      <!-- Card 1: Bagan Pertandingan -->
      <div class="bg-[#14161b] border border-white/[0.08] rounded-xl p-4 flex flex-col justify-between hover:border-white/20 transition relative overflow-hidden group">
        <div class="flex items-start justify-between">
          <div>
            <span class="text-[11px] font-mono font-bold text-zinc-400 uppercase tracking-wide">Bagan Pertandingan</span>
            <div class="text-2xl font-black text-white mt-1">
              {#if isLoading}
                <span class="text-zinc-600 text-lg">Memuat...</span>
              {:else}
                {totalBagans}
                <span class="text-xs font-normal text-zinc-400 ml-1">Kategori</span>
              {/if}
            </div>
          </div>
          <div class="w-10 h-10 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center justify-center text-lg">
            🏆
          </div>
        </div>
        <p class="text-[11px] text-zinc-500 mt-3">
          {totalBagans > 0 ? "Bagan sudah diundi dan siap dipertandingkan." : "Belum ada bagan yang diundi."}
        </p>
      </div>

      <!-- Card 2: Total Peserta Atlet -->
      <div class="bg-[#14161b] border border-white/[0.08] rounded-xl p-4 flex flex-col justify-between hover:border-white/20 transition relative overflow-hidden group">
        <div class="flex items-start justify-between">
          <div>
            <span class="text-[11px] font-mono font-bold text-zinc-400 uppercase tracking-wide">Data Peserta (Atlet)</span>
            <div class="text-2xl font-black text-white mt-1">
              {#if isLoading}
                <span class="text-zinc-600 text-lg">Memuat...</span>
              {:else}
                {totalAthletes}
                <span class="text-xs font-normal text-zinc-400 ml-1">Atlet</span>
              {/if}
            </div>
          </div>
          <div class="w-10 h-10 rounded-lg bg-blue-500/10 border border-blue-500/20 text-blue-400 flex items-center justify-center text-lg">
            🥋
          </div>
        </div>
        <div class="mt-3 flex items-center justify-between">
          <p class="text-[11px] text-zinc-500">Roster atlet terdaftar</p>
          <button
            onclick={() => onNavigate({ type: "roster", id: "athletes", name: "data-atlet", subtitle: "Daftar Atlet Peserta" })}
            class="text-[11px] text-blue-400 hover:text-blue-300 font-semibold"
          >
            Lihat Roster →
          </button>
        </div>
      </div>

      <!-- Card 3: Nomor Tanding / Kategori -->
      <div class="bg-[#14161b] border border-white/[0.08] rounded-xl p-4 flex flex-col justify-between hover:border-white/20 transition relative overflow-hidden group">
        <div class="flex items-start justify-between">
          <div>
            <span class="text-[11px] font-mono font-bold text-zinc-400 uppercase tracking-wide">Nomor Pertandingan</span>
            <div class="text-2xl font-black text-white mt-1">
              {#if isLoading}
                <span class="text-zinc-600 text-lg">Memuat...</span>
              {:else}
                {totalCategories}
                <span class="text-xs font-normal text-zinc-400 ml-1">Nomor</span>
              {/if}
            </div>
          </div>
          <div class="w-10 h-10 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 flex items-center justify-center text-lg">
            📋
          </div>
        </div>
        <div class="mt-3 flex items-center justify-between">
          <p class="text-[11px] text-zinc-500">Kata & Kumite terdaftar</p>
          <button
            onclick={() => onNavigate({ type: "roster", id: "categories", name: "nomor-tanding", subtitle: "Kategori Nomor Pertandingan" })}
            class="text-[11px] text-emerald-400 hover:text-emerald-300 font-semibold"
          >
            Lihat Kategori →
          </button>
        </div>
      </div>

      <!-- Card 4: Kontingen & Utusan -->
      <div class="bg-[#14161b] border border-white/[0.08] rounded-xl p-4 flex flex-col justify-between hover:border-white/20 transition relative overflow-hidden group">
        <div class="flex items-start justify-between">
          <div>
            <span class="text-[11px] font-mono font-bold text-zinc-400 uppercase tracking-wide">Kontingen / Utusan</span>
            <div class="text-2xl font-black text-white mt-1">
              {#if isLoading}
                <span class="text-zinc-600 text-lg">Memuat...</span>
              {:else}
                {totalContingents}
                <span class="text-xs font-normal text-zinc-400 ml-1">Klub/Dojo</span>
              {/if}
            </div>
          </div>
          <div class="w-10 h-10 rounded-lg bg-purple-500/10 border border-purple-500/20 text-purple-400 flex items-center justify-center text-lg">
            🚩
          </div>
        </div>
        <div class="mt-3 flex items-center justify-between">
          <p class="text-[11px] text-zinc-500">Kontingen terdaftar</p>
          <button
            onclick={() => onNavigate({ type: "contingent", id: "contingents", name: "kontingen-utusan", subtitle: "Daftar Kontingen Peserta" })}
            class="text-[11px] text-purple-400 hover:text-purple-300 font-semibold"
          >
            Lihat Kontingen →
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- Section 2: Danger Zone / Reset & Pembersihan Data -->
  <div>
    <div class="flex items-center space-x-2 mb-3">
      <span class="text-red-400 text-sm font-bold">⚠️</span>
      <h2 class="text-xs font-mono font-bold tracking-wider text-red-400 uppercase">
        Zona Bahaya — Reset & Pembersihan Data
      </h2>
    </div>

    <div class="bg-[#14161b] border border-red-500/20 rounded-2xl p-5 space-y-4">
      <div class="p-3.5 rounded-xl bg-red-500/5 border border-red-500/15 text-xs text-zinc-300 leading-relaxed flex items-center space-x-3">
        <span class="text-xl shrink-0">🛡️</span>
        <p>
          Tindakan di bawah ini langsung memodifikasi basis data lokal turnamen. Pastikan pertandingan yang sedang berjalan telah selesai atau Anda telah memiliki cadangan data sebelum melanjutkan.
        </p>
      </div>

      <div class="divide-y divide-white/[0.06]">
        <!-- Action Row 1: Hapus Semua Bagan -->
        <div class="py-4 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-sm font-bold text-white tracking-wide">Hapus Seluruh Bagan Pertandingan</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                {totalBagans} Bagan Aktif
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh struktur bagan eliminasi, partai pertandingan, skor, serta mengosongkan antrean tatami untuk event ini. Data atlet dan nomor tanding tidak akan terhapus sehingga Anda dapat melakukan pengundian (drawing) ulang.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteBagansChecked = false;
              showDeleteBagansModal = true;
            }}
            disabled={totalBagans === 0 || isLoading}
            class="shrink-0 px-4 py-2 rounded-xl bg-red-600/20 hover:bg-red-600/30 text-red-400 hover:text-red-300 border border-red-500/30 text-xs font-bold transition flex items-center space-x-2 disabled:opacity-40 disabled:pointer-events-none active:scale-95"
          >
            <span>🗑️</span>
            <span>Hapus Semua Bagan</span>
          </button>
        </div>

        <!-- Action Row 2: Hapus Semua Data Atlet -->
        <div class="py-4 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-sm font-bold text-white tracking-wide">Hapus Seluruh Data Atlet</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20">
                {totalAthletes} Atlet Terdaftar
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus seluruh data peserta turnamen pada event ini, serta membersihkan tautan atlet pada partai pertandingan. Sangat cocok jika Anda ingin menarik ulang (<span class="font-mono text-zinc-300">re-sync</span>) data atlet baru dari Web Publik.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmDeleteAthletesChecked = false;
              showDeleteAthletesModal = true;
            }}
            disabled={totalAthletes === 0 || isLoading}
            class="shrink-0 px-4 py-2 rounded-xl bg-red-600/20 hover:bg-red-600/30 text-red-400 hover:text-red-300 border border-red-500/30 text-xs font-bold transition flex items-center space-x-2 disabled:opacity-40 disabled:pointer-events-none active:scale-95"
          >
            <span>🥋</span>
            <span>Hapus Semua Data Atlet</span>
          </button>
        </div>

        <!-- Action Row 3: Reset Total Event (Bagan + Atlet) -->
        <div class="py-4 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div class="max-w-xl">
            <div class="flex items-center space-x-2">
              <h3 class="text-sm font-bold text-white tracking-wide">Reset Total Turnamen (Bagan + Atlet)</h3>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-red-500/15 text-red-400 border border-red-500/30">
                Pembersihan Penuh
              </span>
            </div>
            <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
              Menghapus sekaligus seluruh bagan pertandingan dan seluruh data atlet pada event ini. Database kembali bersih untuk import data baru dari awal.
            </p>
          </div>

          <button
            type="button"
            onclick={() => {
              confirmResetAllChecked = false;
              showResetAllModal = true;
            }}
            disabled={(totalBagans === 0 && totalAthletes === 0) || isLoading}
            class="shrink-0 px-4 py-2 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-bold transition shadow-md flex items-center space-x-2 disabled:opacity-40 disabled:pointer-events-none active:scale-95"
          >
            <span>💥</span>
            <span>Reset Total (Bagan & Atlet)</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<!-- ================= MODAL 1: KONFIRMASI HAPUS SEMUA BAGAN ================= -->
{#if showDeleteBagansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-xl shrink-0">
          ⚠️
        </div>
        <div>
          <h3 class="text-base font-extrabold text-white">Konfirmasi Hapus Semua Bagan</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/10 border border-red-500/20 text-xs text-zinc-300 leading-relaxed space-y-2">
        <p>Anda akan menghapus secara permanen:</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-red-300">
          <li><strong>{totalBagans} Bagan Pertandingan</strong></li>
          <li>Seluruh partai (detail bagan), skor, & pemenang</li>
          <li>Seluruh jadwal penugasan di tatami turnamen</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1">
          Data atlet dan kategori pertandingan tetap aman di database.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteBagansChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-0 w-4 h-4"
        />
        <span class="text-xs text-zinc-300 font-medium leading-tight">
          Saya yakin ingin menghapus seluruh bagan pertandingan pada event ini.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2.5 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteBagansModal = false)}
          disabled={isDeletingBagans}
          class="px-4 py-2 rounded-xl text-xs font-bold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.1] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllBagans}
          disabled={!confirmDeleteBagansChecked || isDeletingBagans}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-2 shadow-md"
        >
          {#if isDeletingBagans}
            <span class="animate-spin inline-block">⏳</span>
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
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-xl shrink-0">
          🥋
        </div>
        <div>
          <h3 class="text-base font-extrabold text-white">Konfirmasi Hapus Semua Atlet</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/10 border border-red-500/20 text-xs text-zinc-300 leading-relaxed space-y-2">
        <p>Anda akan menghapus secara permanen:</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-red-300">
          <li><strong>{totalAthletes} Atlet / Peserta</strong> pada event ini</li>
          <li>Tautan atlet pada bagan dan partai pertandingan</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1">
          Setelah dihapus, Anda dapat menarik kembali atlet baru via sinkronisasi portal publik.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteAthletesChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-0 w-4 h-4"
        />
        <span class="text-xs text-zinc-300 font-medium leading-tight">
          Saya yakin ingin menghapus seluruh data atlet turnamen ini.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2.5 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteAthletesModal = false)}
          disabled={isDeletingAthletes}
          class="px-4 py-2 rounded-xl text-xs font-bold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.1] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteAllAthletes}
          disabled={!confirmDeleteAthletesChecked || isDeletingAthletes}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-2 shadow-md"
        >
          {#if isDeletingAthletes}
            <span class="animate-spin inline-block">⏳</span>
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
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center text-xl shrink-0">
          💥
        </div>
        <div>
          <h3 class="text-base font-extrabold text-white">Reset Total Turnamen</h3>
          <p class="text-xs text-zinc-400">Membersihkan seluruh bagan dan atlet</p>
        </div>
      </div>

      <div class="my-4 p-3 rounded-xl bg-red-500/15 border border-red-500/30 text-xs text-zinc-200 leading-relaxed space-y-2">
        <p class="font-bold text-red-300">Peringatan: Seluruh data pertandingan akan dibersihkan!</p>
        <ul class="list-disc pl-5 space-y-1 font-mono text-[11px] text-red-200">
          <li>{totalBagans} Bagan & seluruh partai pertandingan</li>
          <li>{totalAthletes} Data atlet & kontingen</li>
        </ul>
        <p class="text-[11px] text-zinc-400 pt-1">
          Hanya gunakan fitur ini jika Anda ingin memulai turnamen baru dari awal tanpa membuat event baru.
        </p>
      </div>

      <!-- Double-check checkbox -->
      <label class="flex items-start space-x-2.5 my-4 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmResetAllChecked}
          class="mt-0.5 rounded border-white/20 bg-black/40 text-red-600 focus:ring-0 w-4 h-4"
        />
        <span class="text-xs text-red-300 font-bold leading-tight">
          Saya menyatakan paham sepenuhnya dan menyetujui pembersihan seluruh bagan & atlet.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2.5 mt-5">
        <button
          type="button"
          onclick={() => (showResetAllModal = false)}
          disabled={isResettingAll}
          class="px-4 py-2 rounded-xl text-xs font-bold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.1] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleResetAll}
          disabled={!confirmResetAllChecked || isResettingAll}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 flex items-center space-x-2 shadow-lg"
        >
          {#if isResettingAll}
            <span class="animate-spin inline-block">⏳</span>
            <span>Mereset Total...</span>
          {:else}
            <span>Reset Total Sekarang</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
