<script lang="ts">
  import { invoke, isClientMode } from "$lib/api_client";
  import type { ActiveChannel, BaganItem } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    onRefreshEvent: () => void;
    onNavigate: (channel: ActiveChannel) => void;
    onShowToast: (msg: string) => void;
    /** Called after this event was deleted; the shell switches to another event. */
    onEventDeleted?: (eventId: number) => void;
  }

  let { eventId, eventName = "Event Turnamen", onRefreshEvent, onNavigate, onShowToast, onEventDeleted }: Props = $props();

  // Backup and the clean-up actions run on the host's own database and are not
  // available over LAN, so an operator-tambahan laptop only sees them disabled.
  const hostOnly = isClientMode();

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

  // Bumped on every load so a slow response for the previous event is ignored.
  let statsLoadSeq = 0;
  // True when the last load failed: counts are unknown, so destructive actions stay off.
  let statsFailed = $state(false);

  async function loadStatistics() {
    const seq = ++statsLoadSeq;
    const forEvent = eventId;
    isLoading = true;
    errorMessage = null;
    try {
      // No per-call fallbacks: a failed query must show an error, not a silent 0.
      const [baganList, athleteList, categoryList, contingentList, perguruanList] = await Promise.all([
        invoke<BaganItem[]>("get_bagans", { eventId: forEvent }),
        invoke<any[]>("get_athletes", { eventId: forEvent }),
        invoke<any[]>("get_categories", { eventId: forEvent }),
        invoke<any[]>("get_contingents", { eventId: forEvent }),
        invoke<any[]>("get_perguruans", { eventId: forEvent }),
      ]);
      if (seq !== statsLoadSeq) return;
      statsFailed = false;

      totalBagans = baganList.length;
      totalFestivalBagans = baganList.filter((b) =>
        b.id !== 0 &&
        b.kode !== "000" &&
        ((b.nama_bagan || "").toUpperCase().includes("FESTIVAL") || (b.kode || "").toUpperCase().startsWith("F"))
      ).length;
      totalTournamentBagans = Math.max(0, totalBagans - totalFestivalBagans);

      // Team member rows (MEMBER_…) belong to a team entry and are not separate entrants.
      totalAthletes = athleteList.filter((a) => !String(a.additional_code || "").startsWith("MEMBER_")).length;
      totalCategories = categoryList.length;
      totalContingents = contingentList.length;
      totalPerguruans = (perguruanList || []).length;
    } catch (err: any) {
      if (seq !== statsLoadSeq) return;
      statsFailed = true;
      totalBagans = totalFestivalBagans = totalTournamentBagans = 0;
      totalAthletes = totalCategories = totalContingents = totalPerguruans = 0;
      const msg = typeof err === "string" ? err : err?.message || "";
      errorMessage = `Gagal memuat ringkasan data master. Tombol hapus dinonaktifkan sampai data berhasil dimuat.${msg ? " (" + msg + ")" : ""}`;
    } finally {
      if (seq === statsLoadSeq) isLoading = false;
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

  // Hapus event dialog state
  let showDeleteEventModal = $state(false);
  let deleteEventConfirmText = $state("");
  let deleteEventBackupFirst = $state(true);
  let isDeletingEvent = $state(false);

  // Reload whenever the selected event changes, not only on first mount: otherwise the
  // counts (and the confirmation dialogs) would describe the previous event while the
  // buttons act on the new one.
  $effect(() => {
    if (eventId) {
      // Close any open confirmation that was opened for the previous event.
      closeAllModals();
      loadStatistics();
    }
  });

  let anyModalOpen = $derived(
    showDeleteFestivalBagansModal ||
      showDeleteBagansModal ||
      showDeleteAthletesModal ||
      showDeleteCategoriesModal ||
      showDeleteContingentsModal ||
      showDeletePerguruansModal ||
      showResetAllModal ||
      showDeleteEventModal
  );

  let anyActionBusy = $derived(
    isDeletingFestivalBagans ||
      isDeletingBagans ||
      isDeletingAthletes ||
      isDeletingCategories ||
      isDeletingContingents ||
      isDeletingPerguruans ||
      isResettingAll ||
      isDeletingEvent
  );

  function closeAllModals() {
    showDeleteFestivalBagansModal = false;
    showDeleteBagansModal = false;
    showDeleteAthletesModal = false;
    showDeleteCategoriesModal = false;
    showDeleteContingentsModal = false;
    showDeletePerguruansModal = false;
    showResetAllModal = false;
    showDeleteEventModal = false;
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && anyModalOpen && !anyActionBusy) {
      closeAllModals();
      errorMessage = null;
    }
  }

  // ==================== HAPUS EVENT ====================

  let deleteEventNameMatches = $derived(
    deleteEventConfirmText.trim().toUpperCase() === (eventName || "").trim().toUpperCase() &&
      deleteEventConfirmText.trim().length > 0
  );

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
  }

  function openDeleteEventModal() {
    deleteEventConfirmText = "";
    deleteEventBackupFirst = true;
    errorMessage = null;
    showDeleteEventModal = true;
  }

  async function handleDeleteEvent() {
    if (!deleteEventNameMatches || isDeletingEvent) return;
    isDeletingEvent = true;
    errorMessage = null;
    const deletedId = eventId;
    const deletedName = eventName;
    try {
      if (deleteEventBackupFirst) {
        const path = await invoke<string>("backup_database");
        lastBackupPath = path;
      }
      await invoke("delete_event", { eventId: deletedId });
      showDeleteEventModal = false;
      onShowToast(`Event "${deletedName}" beserta seluruh datanya telah dihapus.`);
      onEventDeleted?.(deletedId);
    } catch (err: any) {
      const msg = typeof err === "string" ? err : err?.message || "Gagal menghapus event.";
      errorMessage = deleteEventBackupFirst && !lastBackupPath
        ? `Cadangan gagal dibuat, event TIDAK dihapus. ${msg}`
        : msg;
    } finally {
      isDeletingEvent = false;
    }
  }

  // ==================== LAYOUT DATA ====================

  type CountCard = {
    label: string;
    count: number;
    note?: string;
    channel?: ActiveChannel;
  };

  let countCards = $derived<CountCard[]>([
    {
      label: "Atlet",
      count: totalAthletes,
      channel: { type: "roster", id: "athletes", name: "Atlet", subtitle: "" },
    },
    {
      label: "Nomor tanding",
      count: totalCategories,
      channel: { type: "nomor_tanding", id: "categories", name: "Nomor tanding", subtitle: "" },
    },
    {
      label: "Bagan",
      count: totalBagans,
      note: `${totalTournamentBagans} prestasi · ${totalFestivalBagans} festival`,
    },
    {
      label: "Kontingen",
      count: totalContingents,
      channel: { type: "contingent", id: "contingents", name: "Kontingen", subtitle: "" },
    },
    {
      label: "Perguruan",
      count: totalPerguruans,
      channel: { type: "perguruan", id: "perguruans", name: "Perguruan", subtitle: "" },
    },
  ]);

  type CleanupRow = {
    key: string;
    title: string;
    count: number;
    unit: string;
    note: string;
    open: () => void;
  };

  let cleanupRows = $derived<CleanupRow[]>([
    {
      key: "festival",
      title: "Bagan festival",
      count: totalFestivalBagans,
      unit: "bagan",
      note: "Partai, skor dan juara festival ikut terhapus. Bagan prestasi tetap ada.",
      open: () => {
        confirmDeleteFestivalBagansChecked = false;
        showDeleteFestivalBagansModal = true;
      },
    },
    {
      key: "bagan",
      title: "Semua bagan",
      count: totalBagans,
      unit: "bagan",
      note: "Prestasi dan festival, beserta partai, skor dan penugasan tatami. Atlet dan nomor tanding tetap ada untuk drawing ulang.",
      open: () => {
        confirmDeleteBagansChecked = false;
        showDeleteBagansModal = true;
      },
    },
    {
      key: "atlet",
      title: "Atlet",
      count: totalAthletes,
      unit: "atlet",
      note: "Atlet di partai bagan ikut dikosongkan. Biasanya dipakai sebelum menarik ulang atlet dari web.",
      open: () => {
        confirmDeleteAthletesChecked = false;
        showDeleteAthletesModal = true;
      },
    },
    {
      key: "kategori",
      title: "Nomor tanding",
      count: totalCategories,
      unit: "nomor",
      note: "Bagan dan partai yang memakai nomor tanding ini ikut terhapus.",
      open: () => {
        confirmDeleteCategoriesChecked = false;
        showDeleteCategoriesModal = true;
      },
    },
    {
      key: "kontingen",
      title: "Kontingen",
      count: totalContingents,
      unit: "kontingen",
      note: "Kontingen pada atlet dikosongkan.",
      open: () => {
        confirmDeleteContingentsChecked = false;
        showDeleteContingentsModal = true;
      },
    },
    {
      key: "perguruan",
      title: "Perguruan",
      count: totalPerguruans,
      unit: "perguruan",
      note: "Perguruan pada atlet dan wasit dikosongkan.",
      open: () => {
        confirmDeletePerguruansChecked = false;
        showDeletePerguruansModal = true;
      },
    },
  ]);

  let nothingToReset = $derived(
    totalBagans === 0 && totalAthletes === 0 && totalCategories === 0 && totalContingents === 0 && totalPerguruans === 0
  );
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#snippet trashIcon(cls: string)}
  <svg class={cls} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
  </svg>
{/snippet}

<div class="h-full p-6 pb-12 select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4">
    <div class="min-w-0">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">Data master</h2>
      <p class="text-xs text-zinc-400 mt-1 truncate select-text">{eventName}</p>
    </div>
    <button
      type="button"
      onclick={loadStatistics}
      disabled={isLoading}
      class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 disabled:opacity-50 cursor-pointer self-start sm:self-auto"
    >
      <svg class="w-3.5 h-3.5 text-zinc-400 {isLoading ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
      <span>Segarkan</span>
    </button>
  </div>

  {#if errorMessage}
    <div class="mt-4 p-3.5 rounded-xl bg-red-500/10 border border-red-500/25 text-red-300 text-xs flex items-start justify-between gap-3" role="alert">
      <div class="flex items-start gap-2.5">
        <svg class="w-4 h-4 text-red-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" />
        </svg>
        <span class="leading-relaxed select-text">{errorMessage}</span>
      </div>
      <button
        type="button"
        onclick={() => (errorMessage = null)}
        class="w-7 h-7 -m-1 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition shrink-0 cursor-pointer"
        aria-label="Tutup pesan error"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  {/if}

  <!-- Counts: each card opens its channel -->
  <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-2 my-4">
    {#each countCards as card (card.label)}
      {#if card.channel}
        {@const target = card.channel}
        <button
          type="button"
          onclick={() => onNavigate(target)}
          aria-label={`${card.label}: ${card.count}. Buka channel ${card.label}`}
          class="group text-left rounded-xl px-3 py-2.5 border bg-[#181a20] border-white/[0.08] hover:border-white/25 transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        >
          <span class="flex items-center justify-between text-xs font-semibold text-zinc-400">
            {card.label}
            <svg class="w-3.5 h-3.5 text-zinc-600 group-hover:text-zinc-300 transition" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
            </svg>
          </span>
          <span class="block mt-1 text-2xl font-black font-mono tabular-nums text-white">{isLoading ? "…" : card.count}</span>
        </button>
      {:else}
        <div class="rounded-xl px-3 py-2.5 border bg-[#181a20] border-white/[0.08]">
          <span class="block text-xs font-semibold text-zinc-400">{card.label}</span>
          <span class="flex items-baseline gap-2 mt-1">
            <span class="text-2xl font-black font-mono tabular-nums text-white">{isLoading ? "…" : card.count}</span>
            {#if card.note && !isLoading}
              <span class="text-[11px] text-zinc-500 truncate">{card.note}</span>
            {/if}
          </span>
        </div>
      {/if}
    {/each}
  </div>

  {#if hostOnly}
    <div class="mb-4 p-3 rounded-xl bg-white/[0.04] border border-white/[0.1] text-xs text-zinc-300 flex items-center gap-2">
      <svg class="w-4 h-4 text-zinc-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
      </svg>
      <span>Cadangan, hapus data dan hapus event hanya bisa dilakukan dari <strong class="text-white">komputer utama</strong>.</span>
    </div>
  {/if}

  <!-- Backup -->
  <section class="rounded-2xl border border-white/[0.08] bg-[#181a20] p-4 mb-6" aria-labelledby="dm-backup-title">
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-3">
      <div class="flex items-start gap-3 min-w-0">
        <div class="w-9 h-9 rounded-xl bg-emerald-500/10 text-emerald-400 border border-emerald-500/25 flex items-center justify-center shrink-0">
          <svg class="w-4.5 h-4.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
        </div>
        <div class="min-w-0">
          <h3 id="dm-backup-title" class="text-sm font-bold text-white">Cadangan database</h3>
          <p class="text-xs text-zinc-400 mt-0.5">Salinan lengkap semua event ke file .sqlite3, misalnya ke flashdisk.</p>
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <button
          type="button"
          onclick={handleCustomBackup}
          disabled={isBackingUpQuick || isBackingUpCustom || hostOnly}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 disabled:opacity-40 cursor-pointer"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
          </svg>
          <span>{isBackingUpCustom ? "Menyimpan…" : "Simpan ke…"}</span>
        </button>
        <button
          type="button"
          onclick={handleQuickBackup}
          disabled={isBackingUpQuick || isBackingUpCustom || hostOnly}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center gap-1.5 shadow-md disabled:opacity-40 cursor-pointer"
        >
          {#if isBackingUpQuick}
            <svg class="w-3.5 h-3.5 animate-spin" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
            </svg>
            <span>Mencadangkan…</span>
          {:else}
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
              <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
            <span>Cadangkan sekarang</span>
          {/if}
        </button>
      </div>
    </div>

    {#if lastBackupPath}
      <div class="mt-3 pt-3 border-t border-white/[0.06] text-xs text-zinc-400 flex items-center gap-2 min-w-0">
        <svg class="w-4 h-4 shrink-0 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
        </svg>
        <span class="shrink-0">Tersimpan di</span>
        <span class="font-mono text-[11px] text-zinc-300 truncate select-text" title={lastBackupPath}>{lastBackupPath}</span>
      </div>
    {/if}
  </section>

  <!-- Clean-up -->
  <section aria-labelledby="dm-cleanup-title">
    <div class="flex items-baseline justify-between gap-3 mb-2">
      <h3 id="dm-cleanup-title" class="text-sm font-bold text-white">Hapus data event</h3>
      <span class="text-xs text-zinc-500">Permanen. Cadangkan dulu bila ragu.</span>
    </div>

    <div class="rounded-2xl border border-white/[0.08] bg-[#181a20] divide-y divide-white/[0.06]">
      {#each cleanupRows as row (row.key)}
        <div class="px-4 py-3 flex flex-col md:flex-row md:items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="flex items-baseline gap-2">
              <span class="text-sm font-semibold text-white">{row.title}</span>
              <span class="text-xs text-zinc-500 tabular-nums">{isLoading ? "…" : `${row.count} ${row.unit}`}</span>
            </div>
            <p class="text-xs text-zinc-400 mt-0.5">{row.note}</p>
          </div>
          <button
            type="button"
            onclick={row.open}
            disabled={row.count === 0 || isLoading || statsFailed || hostOnly}
            aria-label={`Hapus ${row.title.toLowerCase()}`}
            class="shrink-0 h-9 px-3.5 rounded-xl bg-transparent hover:bg-red-500/10 text-red-300 border border-red-500/30 hover:border-red-500/50 text-xs font-semibold transition-colors flex items-center gap-1.5 disabled:opacity-35 disabled:pointer-events-none cursor-pointer self-start md:self-auto"
          >
            {@render trashIcon("w-3.5 h-3.5")}
            <span>Hapus…</span>
          </button>
        </div>
      {/each}
    </div>

    <!-- The two irreversible, event-wide actions -->
    <div class="mt-3 rounded-2xl border border-red-500/35 bg-red-950/10 divide-y divide-red-500/15">
      <div class="px-4 py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-3">
        <div class="min-w-0">
          <span class="text-sm font-bold text-white">Reset total event</span>
          <p class="text-xs text-zinc-400 mt-0.5">
            Hapus bagan, atlet, nomor tanding, kontingen, perguruan, tatami, jadwal, wasit, kop surat dan berkas festival. Event tetap ada dalam keadaan kosong.
          </p>
        </div>
        <button
          type="button"
          onclick={() => {
            confirmResetAllChecked = false;
            showResetAllModal = true;
          }}
          disabled={nothingToReset || isLoading || statsFailed || hostOnly}
          class="shrink-0 h-9 px-4 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-bold transition-colors flex items-center gap-1.5 disabled:opacity-35 disabled:pointer-events-none cursor-pointer self-start md:self-auto"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          <span>Reset total…</span>
        </button>
      </div>
      <div class="px-4 py-3.5 flex flex-col md:flex-row md:items-center justify-between gap-3">
        <div class="min-w-0">
          <span class="text-sm font-bold text-white">Hapus event ini</span>
          <p class="text-xs text-zinc-400 mt-0.5">
            Semua isi reset total, ditambah event itu sendiri dan hubungannya ke event web. Event lain tidak terpengaruh.
          </p>
        </div>
        <button
          type="button"
          onclick={openDeleteEventModal}
          disabled={isLoading || hostOnly}
          class="shrink-0 h-9 px-4 rounded-xl bg-transparent hover:bg-red-600 text-red-300 hover:text-white border border-red-500/50 text-xs font-bold transition-colors flex items-center gap-1.5 disabled:opacity-35 disabled:pointer-events-none cursor-pointer self-start md:self-auto"
        >
          {@render trashIcon("w-3.5 h-3.5")}
          <span>Hapus event…</span>
        </button>
      </div>
    </div>
  </section>
</div>

<!-- Errors from an action must be visible above its confirmation dialog. -->
{#if errorMessage && anyModalOpen}
  <div class="fixed top-4 left-1/2 -translate-x-1/2 z-[60] w-[min(36rem,calc(100%-2rem))] p-3.5 rounded-xl bg-[#1f1214] border border-red-500/40 text-red-200 text-xs shadow-2xl flex items-start justify-between gap-3" role="alert">
    <span class="leading-relaxed select-text">{errorMessage}</span>
    <button
      type="button"
      onclick={() => (errorMessage = null)}
      class="w-7 h-7 -m-1 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] shrink-0 cursor-pointer"
      aria-label="Tutup pesan error"
    >
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
{/if}

<!-- ================= MODAL: HAPUS EVENT ================= -->
{#if showDeleteEventModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div
      class="bg-[#181a20] border border-red-500/40 w-full max-w-lg rounded-2xl shadow-2xl p-6 text-zinc-200"
      role="dialog"
      aria-modal="true"
      aria-labelledby="delete-event-title"
    >
      <h3 id="delete-event-title" class="text-base font-extrabold text-white">Hapus event "{eventName}"?</h3>
      <p class="text-xs text-zinc-400 mt-1">Tindakan ini tidak dapat dibatalkan dari aplikasi.</p>

      <div class="my-4 p-3.5 rounded-xl bg-red-500/10 border border-red-500/25 text-xs text-zinc-200 leading-relaxed">
        <p class="font-semibold text-red-300 mb-1.5">Yang akan dihapus:</p>
        <ul class="list-disc pl-5 space-y-0.5 font-mono text-[11px] text-red-100/90">
          <li><span class="tabular-nums">{totalAthletes}</span> atlet, <span class="tabular-nums">{totalCategories}</span> nomor tanding, <span class="tabular-nums">{totalBagans}</span> bagan</li>
          <li><span class="tabular-nums">{totalContingents}</span> kontingen, <span class="tabular-nums">{totalPerguruans}</span> perguruan</li>
          <li>Tatami, jadwal, wasit, kop surat, berkas festival & pemetaan sinkron web</li>
          <li>Event itu sendiri dari daftar event</li>
        </ul>
      </div>

      <label class="flex items-start gap-2.5 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={deleteEventBackupFirst}
          class="mt-0.5 w-4 h-4 accent-emerald-500 cursor-pointer"
        />
        <span class="text-xs text-zinc-300 leading-snug">
          Buat cadangan database dulu <span class="text-zinc-500">(disarankan; kalau cadangan gagal, event tidak dihapus)</span>
        </span>
      </label>

      <label class="block mt-4">
        <span class="text-xs text-zinc-300">Ketik nama event untuk konfirmasi:</span>
        <span class="block mt-1 font-mono text-[11px] text-zinc-400 select-text">{eventName}</span>
        <input
          use:focusOnMount
          type="text"
          bind:value={deleteEventConfirmText}
          onkeydown={(e) => {
            if (e.key === "Enter") handleDeleteEvent();
          }}
          autocomplete="off"
          spellcheck="false"
          class="mt-1.5 w-full bg-[#14161b] text-sm text-white px-3 py-2 rounded-lg border outline-none {deleteEventNameMatches ? 'border-red-500/60' : 'border-white/[0.1] focus:border-white/30'}"
        />
      </label>

      <div class="flex items-center justify-end gap-2 mt-5">
        <button
          type="button"
          onclick={() => (showDeleteEventModal = false)}
          disabled={isDeletingEvent}
          class="px-4 py-2 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white bg-white/[0.05] hover:bg-white/[0.08] transition cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleDeleteEvent}
          disabled={!deleteEventNameMatches || isDeletingEvent}
          class="px-4 py-2 rounded-xl text-xs font-bold text-white bg-red-600 hover:bg-red-500 transition disabled:opacity-40 active:scale-95 cursor-pointer"
        >
          {isDeletingEvent ? (deleteEventBackupFirst ? "Mencadangkan & menghapus…" : "Menghapus…") : "Hapus event permanen"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 1: KONFIRMASI HAPUS SEMUA BAGAN FESTIVAL (NEW) ================= -->
{#if showDeleteFestivalBagansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-amber-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-amber-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-amber-500/10 border border-amber-500/20 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus semua bagan festival?</h3>
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
            <span>Menghapus…</span>
          {:else}
            <span>Hapus bagan festival</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 2: KONFIRMASI HAPUS SEMUA BAGAN (SEMUA) ================= -->
{#if showDeleteBagansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus semua bagan?</h3>
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
            <span>Menghapus…</span>
          {:else}
            <span>Hapus semua bagan</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 3: KONFIRMASI HAPUS SEMUA DATA ATLET ================= -->
{#if showDeleteAthletesModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 7a4 4 0 11-8 0 4 4 0 018 0zM9 14a6 6 0 00-6 6v1h12v-1a6 6 0 00-6-6zM21 12h-6" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus semua atlet?</h3>
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
            <span>Menghapus…</span>
          {:else}
            <span>Hapus semua atlet</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL: HAPUS SEMUA KATEGORI ================= -->
{#if showDeleteCategoriesModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus semua nomor tanding?</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
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
            <span>Menghapus…</span>
          {:else}
            <span>Hapus nomor tanding</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL: HAPUS SEMUA KONTINGEN ================= -->
{#if showDeleteContingentsModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus semua kontingen?</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
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
            <span>Menghapus…</span>
          {:else}
            <span>Hapus kontingen</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL: HAPUS SEMUA PERGURUAN ================= -->
{#if showDeletePerguruansModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Hapus semua perguruan?</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
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
            <span>Menghapus…</span>
          {:else}
            <span>Hapus perguruan</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ================= MODAL 4: KONFIRMASI RESET TOTAL ================= -->
{#if showResetAllModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
    <div class="bg-[#181a20] border border-red-500/40 w-full max-w-lg rounded-2xl shadow-2xl p-6 text-zinc-200" role="dialog" aria-modal="true">
      <div class="flex items-center space-x-3 text-red-400 mb-3">
        <div class="w-10 h-10 rounded-xl bg-red-600/20 border border-red-500/30 flex items-center justify-center shrink-0">
          <svg class="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm sm:text-base font-extrabold text-white">Reset total event?</h3>
          <p class="text-xs text-zinc-400">Tindakan ini tidak dapat dibatalkan</p>
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
            <span>Mereset…</span>
          {:else}
            <span>Reset total</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
