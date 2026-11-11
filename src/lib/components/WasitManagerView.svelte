<script lang="ts">
  // Wasit & Juri channel: the referee database of the event (add, edit, import, photos).
  // Assigning referees to a tatami / bout happens only in the Tatami Manager.
  import { onMount } from "svelte";
  import { apiCall } from "$lib/api_client";
  import { subscribeRefereeUpdates } from "$lib/liveSync";
  import { photoToWebp } from "$lib/refereeSeats";
  import FuzzySelect, { type SelectOption } from "./FuzzySelect.svelte";
  import RefereeAvatar from "./RefereeAvatar.svelte";
  import WasitImportModal from "./WasitImportModal.svelte";
  import WasitPhotoBulkModal from "./WasitPhotoBulkModal.svelte";
  import type { WasitItem, SaveWasitPayload, PerguruanItem } from "$lib/types";

  interface Props {
    eventId: number;
    onShowToast?: (msg: string) => void;
  }
  let { eventId, onShowToast }: Props = $props();

  let wasits = $state<WasitItem[]>([]);
  let perguruans = $state<PerguruanItem[]>([]);
  let isLoading = $state(true);

  // Toolbar
  let statusFilter = $state<"all" | "aktif" | "nonaktif" | "tanpa_foto">("all");
  let perguruanFilter = $state<string>("all");
  let searchKeyword = $state("");

  // Add / edit modal
  let isFormOpen = $state(false);
  let editing = $state<WasitItem | null>(null);
  let form = $state({ nama: "", perguruanId: null as number | null, kabKota: "", lisensi: "", noLisensi: "", keterangan: "", isActive: true });
  let formError = $state("");
  let isSaving = $state(false);
  let isPhotoSaving = $state(false);

  // Delete confirmation
  let toDelete = $state<WasitItem | null>(null);
  let isDeleting = $state(false);

  // Import & bulk photos
  let isImportOpen = $state(false);
  let isBulkPhotoOpen = $state(false);

  const LICENSE_OPTIONS = ["Wasit WKF / AKF", "Wasit Nasional A", "Wasit Nasional B", "Juri Nasional A", "Juri Nasional B", "Wasit Daerah", "Juri Daerah"];

  // Event setting: bouts need the chief referee's approval before they start
  let requireApproval = $state(false);
  let approvalSaving = $state(false);
  async function loadSettings() {
    try {
      const s = await apiCall<{ require_chief_approval: boolean }>("get_event_referee_settings", { eventId });
      requireApproval = !!s.require_chief_approval;
    } catch (e) {
      console.warn("loadSettings err:", e);
    }
  }
  async function toggleApproval() {
    approvalSaving = true;
    try {
      await apiCall("set_event_referee_settings", { eventId, requireChiefApproval: !requireApproval });
      requireApproval = !requireApproval;
      onShowToast?.(requireApproval ? "Partai wajib disetujui Chief Referee sebelum dimulai." : "Persetujuan Chief Referee dimatikan.");
    } catch (e: any) {
      onShowToast?.("Gagal menyimpan pengaturan: " + (e?.message || e));
    } finally {
      approvalSaving = false;
    }
  }

  onMount(() => {
    Promise.all([loadWasits(), loadPerguruans(), loadSettings()]).finally(() => (isLoading = false));
    // another laptop / tablet changed referees: reload
    return subscribeRefereeUpdates(() => {
      loadWasits();
      loadSettings();
    });
  });

  async function loadWasits() {
    try {
      wasits = await apiCall<WasitItem[]>("get_wasits", { eventId, event_id: eventId });
    } catch (e: any) {
      onShowToast?.("Gagal memuat wasit: " + (e?.message || e));
    }
  }

  async function loadPerguruans() {
    try {
      perguruans = await apiCall<PerguruanItem[]>("get_perguruans", { eventId, event_id: eventId });
    } catch (e) {
      console.warn("loadPerguruans err:", e);
    }
  }

  // get_wasits returns one row per tatami assignment: one row per referee here
  let uniqueWasits = $derived([...new Map(wasits.map((w) => [w.id, w])).values()]);

  let activeCount = $derived(uniqueWasits.filter((w) => w.is_active).length);
  let inactiveCount = $derived(uniqueWasits.length - activeCount);
  let photoCount = $derived(uniqueWasits.filter((w) => w.foto).length);
  let noPhotoCount = $derived(uniqueWasits.length - photoCount);
  let perguruanCount = $derived(new Set(uniqueWasits.map((w) => w.perguruan_id).filter(Boolean)).size);

  let perguruanFilterOptions = $derived<SelectOption[]>([
    { value: "all", label: "Semua Perguruan" },
    ...[...new Map(uniqueWasits.filter((w) => w.perguruan_id).map((w) => [w.perguruan_id, w.nama_perguruan || `#${w.perguruan_id}`])).entries()]
      .sort((a, b) => String(a[1]).localeCompare(String(b[1])))
      .map(([id, nama]) => ({ value: String(id), label: String(nama) })),
  ]);

  let modalPerguruanOptions = $derived<SelectOption[]>([
    { value: null, label: "-- Tanpa Perguruan --" },
    ...perguruans.map((p) => ({ value: p.id, label: p.nama_perguruan || `[#${p.id}] Tanpa Nama` })),
  ]);

  let filtered = $derived(
    uniqueWasits
      .filter((w) => {
        if (statusFilter === "aktif" && !w.is_active) return false;
        if (statusFilter === "nonaktif" && w.is_active) return false;
        if (statusFilter === "tanpa_foto" && w.foto) return false;
        if (perguruanFilter !== "all" && String(w.perguruan_id) !== perguruanFilter) return false;
        const q = searchKeyword.trim().toLowerCase();
        if (q) {
          const hay = [w.nama_wasit, w.nama_perguruan, w.kab_kota, w.lisensi, w.no_lisensi, w.keterangan].join(" ").toLowerCase();
          if (!hay.includes(q)) return false;
        }
        return true;
      })
      .sort((a, b) => a.nama_wasit.localeCompare(b.nama_wasit))
  );

  function resetFilters() {
    statusFilter = "all";
    perguruanFilter = "all";
    searchKeyword = "";
  }

  function openCreate() {
    editing = null;
    form = { nama: "", perguruanId: null, kabKota: "", lisensi: "", noLisensi: "", keterangan: "", isActive: true };
    formError = "";
    isFormOpen = true;
  }

  function openEdit(w: WasitItem) {
    editing = w;
    form = {
      nama: w.nama_wasit,
      perguruanId: w.perguruan_id,
      kabKota: w.kab_kota || "",
      lisensi: w.lisensi || "",
      noLisensi: w.no_lisensi || "",
      keterangan: w.keterangan || "",
      isActive: w.is_active,
    };
    formError = "";
    isFormOpen = true;
  }

  async function handleSave() {
    if (!form.nama.trim()) {
      formError = "Nama wasit wajib diisi.";
      return;
    }
    const payload: SaveWasitPayload = {
      id: editing ? editing.id : null,
      event_id: eventId,
      nama_wasit: form.nama.trim().toUpperCase(),
      perguruan_id: form.perguruanId,
      kab_kota: form.kabKota.trim() || null,
      lisensi: form.lisensi.trim() || null,
      no_lisensi: form.noLisensi.trim() || null,
      is_active: form.isActive,
      keterangan: form.keterangan.trim() || null,
    };
    isSaving = true;
    try {
      await apiCall("save_wasit", { input: payload });
      isFormOpen = false;
      onShowToast?.(editing ? "Data wasit diperbarui." : "Wasit baru ditambahkan.");
      await loadWasits();
    } catch (e: any) {
      formError = "Gagal menyimpan: " + (e?.message || e);
    } finally {
      isSaving = false;
    }
  }

  async function handlePhotoPick(file: File | undefined) {
    if (!file || !editing) return;
    isPhotoSaving = true;
    try {
      const { dataUrl } = await photoToWebp(file);
      const path = await apiCall<string>("save_wasit_photo", { wasitId: editing.id, base64Data: dataUrl });
      editing = { ...editing, foto: path };
      onShowToast?.("Foto wasit tersimpan.");
      await loadWasits();
    } catch (e: any) {
      formError = "Gagal menyimpan foto: " + (e?.message || e);
    } finally {
      isPhotoSaving = false;
    }
  }

  async function confirmDelete() {
    if (!toDelete) return;
    isDeleting = true;
    try {
      await apiCall("delete_wasit", { wasitId: toDelete.id, wasit_id: toDelete.id, id: toDelete.id });
      onShowToast?.(`Wasit "${toDelete.nama_wasit}" dihapus.`);
      toDelete = null;
      await loadWasits();
    } catch (e: any) {
      onShowToast?.("Gagal menghapus wasit: " + (e?.message || e));
    } finally {
      isDeleting = false;
    }
  }

  const pill = (active: boolean, on: string) =>
    `px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer ${active ? on + " shadow-xs font-bold" : "bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white"}`;
  const inputCls = "w-full bg-[#111215] text-xs text-zinc-200 px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors placeholder-zinc-500";
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4 shrink-0">
    <div>
      <div class="flex items-center space-x-2.5">
        <h2 class="text-xl font-black text-white tracking-tight uppercase">WASIT &amp; JURI</h2>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">
          {uniqueWasits.length} WASIT
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1 font-normal">Penugasan ke tatami dan partai dilakukan di Tatami Manager.</p>
    </div>

    <div class="flex items-center space-x-2 shrink-0 self-start sm:self-auto">
      <button
        type="button"
        role="switch"
        aria-checked={requireApproval}
        disabled={approvalSaving}
        onclick={toggleApproval}
        title="Partai baru bisa dimulai di control panel setelah wasit & juri disetujui Chief Referee"
        class="h-9 px-3.5 font-semibold text-xs rounded-xl border transition-colors flex items-center gap-2 cursor-pointer disabled:opacity-50 {requireApproval
          ? 'bg-violet-500/15 text-violet-100 border-violet-400/50'
          : 'bg-white/[0.04] text-zinc-300 border-white/[0.1] hover:bg-white/[0.08]'}"
      >
        <span class="relative inline-block w-7 h-4 rounded-full transition {requireApproval ? 'bg-violet-400' : 'bg-zinc-600'}">
          <span class="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all {requireApproval ? 'left-3.5' : 'left-0.5'}"></span>
        </span>
        <span>Persetujuan Chief Referee</span>
      </button>
      <button
        type="button"
        onclick={() => (isImportOpen = true)}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
        </svg>
        <span>Import Excel / CSV</span>
      </button>
      <button
        type="button"
        onclick={() => (isBulkPhotoOpen = true)}
        disabled={uniqueWasits.length === 0}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-40 cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-sky-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
        </svg>
        <span>Foto Massal</span>
      </button>
      <button
        type="button"
        onclick={openCreate}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center space-x-1.5 active:scale-95 shadow-md cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
        </svg>
        <span>Tambah Wasit</span>
      </button>
    </div>
  </div>

  <!-- Stat cards -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
    {#each [
      { t: "TOTAL WASIT", n: uniqueWasits.length, u: "wasit", c: "text-white" },
      { t: "AKTIF", n: activeCount, u: "wasit", c: "text-emerald-400" },
      { t: "NON-AKTIF", n: inactiveCount, u: "wasit", c: "text-zinc-400" },
      { t: "BERFOTO", n: photoCount, u: "foto", c: "text-sky-400" },
      { t: "PERGURUAN", n: perguruanCount, u: "perguruan", c: "text-amber-400" },
    ] as s (s.t)}
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider">{s.t}</span>
        <div class="flex items-baseline justify-between mt-2">
          <span class="text-2xl font-black font-mono tabular-nums {s.c}">{s.n}</span>
          <span class="text-[10px] font-mono text-zinc-500">{s.u}</span>
        </div>
      </div>
    {/each}
  </div>

  <!-- Toolbar -->
  <div class="flex flex-col lg:flex-row lg:items-center justify-between gap-3 mb-3 shrink-0">
    <div class="flex flex-wrap items-center gap-1.5 py-0.5">
      <button type="button" onclick={() => (statusFilter = "all")} class={pill(statusFilter === "all", "bg-white text-black border-white")}>Semua ({uniqueWasits.length})</button>
      <button type="button" onclick={() => (statusFilter = "aktif")} class={pill(statusFilter === "aktif", "bg-emerald-400 text-black border-emerald-400")}>Aktif ({activeCount})</button>
      <button type="button" onclick={() => (statusFilter = "nonaktif")} class={pill(statusFilter === "nonaktif", "bg-zinc-300 text-black border-zinc-300")}>Non-aktif ({inactiveCount})</button>
      <button type="button" onclick={() => (statusFilter = "tanpa_foto")} class={pill(statusFilter === "tanpa_foto", "bg-rose-500 text-white border-rose-500")}>Tanpa Foto ({noPhotoCount})</button>
    </div>

    <div class="flex items-center space-x-2 shrink-0">
      <FuzzySelect
        items={perguruanFilterOptions}
        bind:value={perguruanFilter}
        placeholder="Filter Perguruan"
        allowClear={true}
        clearLabel="Semua Perguruan"
        clearValue="all"
        compact={true}
        class="w-40"
      />
      <div class="relative w-56">
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari nama, perguruan, kota..."
          class="w-full bg-[#181a20] text-xs text-white pl-8 pr-7 py-1.5 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors placeholder-zinc-500"
        />
        <svg class="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        {#if searchKeyword}
          <button type="button" onclick={() => (searchKeyword = "")} aria-label="Hapus pencarian" class="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-white text-xs cursor-pointer">✕</button>
        {/if}
      </div>
    </div>
  </div>

  <!-- Table -->
  <div class="flex-1 bg-[#181a20] border border-white/[0.08] rounded-2xl overflow-hidden flex flex-col shadow-inner min-h-0">
    {#if isLoading && uniqueWasits.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
        <svg class="w-8 h-8 animate-spin text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <p class="text-xs text-zinc-400 font-mono">Memuat data wasit...</p>
      </div>
    {:else if filtered.length === 0}
      <div class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-zinc-500">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
          </svg>
        </div>
        {#if uniqueWasits.length === 0}
          <p class="text-sm font-bold text-white">Belum Ada Wasit</p>
          <button type="button" onclick={() => (isImportOpen = true)} class="h-8 px-3 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-xs font-semibold text-zinc-300 hover:text-white transition-colors cursor-pointer">
            Import Excel / CSV
          </button>
        {:else}
          <p class="text-sm font-bold text-white">Tidak Ada Wasit Ditemukan</p>
          <button type="button" onclick={resetFilters} class="h-8 px-3 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-xs font-semibold text-zinc-300 hover:text-white transition-colors cursor-pointer">
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
              <th class="px-4 py-3 font-bold">NAMA WASIT / JURI</th>
              <th class="px-4 py-3 font-bold">PERGURUAN</th>
              <th class="px-4 py-3 font-bold">KAB / KOTA</th>
              <th class="px-4 py-3 font-bold">LISENSI</th>
              <th class="w-24 px-3 py-3 text-center font-bold">STATUS</th>
              <th class="w-24 px-4 py-3 text-center pr-6 font-bold">AKSI</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.04]">
            {#each filtered as w, index (w.id)}
              <tr class="hover:bg-white/[0.02] transition-colors {w.is_active ? '' : 'opacity-60'}">
                <td class="px-3 py-3 text-center font-mono text-zinc-500 text-[11px] tabular-nums">{index + 1}</td>
                <td class="px-4 py-2.5">
                  <div class="flex items-center gap-3">
                    <RefereeAvatar foto={w.foto} nama={w.nama_wasit} size={36} />
                    <div class="min-w-0">
                      <div class="flex items-center space-x-2">
                        <span class="font-bold text-white text-xs uppercase tracking-tight">{w.nama_wasit}</span>
                        {#if w.keterangan}
                          <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-black bg-amber-500/15 text-amber-300 border border-amber-500/30 uppercase">{w.keterangan}</span>
                        {/if}
                      </div>
                      {#if w.no_lisensi}
                        <div class="font-mono text-[10px] text-zinc-500 mt-0.5">{w.no_lisensi}</div>
                      {/if}
                    </div>
                  </div>
                </td>
                <td class="px-4 py-3">
                  {#if w.nama_perguruan}
                    <span class="inline-flex items-center space-x-1.5 text-xs font-semibold text-amber-300 bg-amber-500/10 px-2 py-0.5 rounded-lg border border-amber-500/20 uppercase tracking-tight">
                      <span class="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0"></span>
                      <span class="truncate max-w-[130px]" title={w.nama_perguruan}>{w.nama_perguruan}</span>
                    </span>
                  {:else}
                    <span class="text-xs font-mono text-zinc-500">—</span>
                  {/if}
                </td>
                <td class="px-4 py-3 font-bold text-zinc-200 text-xs uppercase">{w.kab_kota || "—"}</td>
                <td class="px-4 py-3">
                  {#if w.lisensi}
                    <span class="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.08]">{w.lisensi}</span>
                  {:else}
                    <span class="text-xs font-mono text-zinc-500">—</span>
                  {/if}
                </td>
                <td class="w-24 px-3 py-3 text-center">
                  {#if w.is_active}
                    <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-emerald-500/15 text-emerald-300 border border-emerald-500/30">AKTIF</span>
                  {:else}
                    <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.06] text-zinc-400 border border-white/[0.08]">NON-AKTIF</span>
                  {/if}
                </td>
                <td class="w-24 px-4 py-3 text-center pr-6">
                  <div class="flex items-center justify-center space-x-1.5">
                    <button
                      type="button"
                      onclick={() => openEdit(w)}
                      class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-sky-500/15 text-zinc-400 hover:text-sky-300 border border-white/[0.08] hover:border-sky-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95"
                      title={`Ubah data ${w.nama_wasit}`}
                      aria-label={`Ubah data ${w.nama_wasit}`}
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125" />
                      </svg>
                    </button>
                    <button
                      type="button"
                      onclick={() => (toDelete = w)}
                      class="h-7 w-7 rounded-lg bg-white/[0.04] hover:bg-rose-500/15 text-zinc-400 hover:text-rose-400 border border-white/[0.08] hover:border-rose-500/30 transition-all duration-150 flex items-center justify-center cursor-pointer active:scale-95"
                      title={`Hapus ${w.nama_wasit}`}
                      aria-label={`Hapus ${w.nama_wasit}`}
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

      <div class="bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0 tabular-nums">
        <span>Menampilkan {filtered.length} dari {uniqueWasits.length} wasit</span>
        {#if noPhotoCount > 0}
          <button type="button" onclick={() => (statusFilter = "tanpa_foto")} class="text-zinc-400 hover:text-white cursor-pointer">{noPhotoCount} belum berfoto</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- Modal: Tambah / Edit Wasit -->
{#if isFormOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none" role="dialog" aria-modal="true">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-lg shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight">{editing ? "Edit Data Wasit" : "Tambah Wasit Baru"}</h3>
        <button type="button" onclick={() => (isFormOpen = false)} class="text-zinc-500 hover:text-white transition-colors cursor-pointer" aria-label="Tutup">✕</button>
      </div>

      <div class="p-6 space-y-4 overflow-y-auto max-h-[70vh]">
        {#if formError}
          <div class="bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs px-3 py-2 rounded-xl">{formError}</div>
        {/if}

        {#if editing}
          <div class="flex items-center gap-3">
            <RefereeAvatar foto={editing.foto} nama={editing.nama_wasit} size={72} rounded="rounded-xl" />
            <label class="h-9 px-3.5 inline-flex items-center bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl cursor-pointer transition-colors">
              {isPhotoSaving ? "Menyimpan…" : editing.foto ? "Ganti Foto" : "Unggah Foto"}
              <input type="file" accept="image/*" class="hidden" disabled={isPhotoSaving} onchange={(e) => handlePhotoPick(e.currentTarget.files?.[0])} />
            </label>
          </div>
        {/if}

        <div>
          <label for="wasit-nama" class="block text-xs font-semibold text-zinc-300 mb-1.5">Nama Lengkap <span class="text-rose-400">*</span></label>
          <input id="wasit-nama" type="text" bind:value={form.nama} placeholder="Contoh: SURYA DHARMA" class="{inputCls} font-semibold uppercase text-white" />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label for="wasit-perguruan" class="block text-xs font-semibold text-zinc-300 mb-1.5">Perguruan</label>
            <FuzzySelect
              id="wasit-perguruan"
              items={modalPerguruanOptions}
              bind:value={form.perguruanId}
              placeholder="Pilih Perguruan..."
              allowClear={true}
              clearLabel="-- Tanpa Perguruan --"
              clearValue={null}
              class="w-full"
            />
          </div>
          <div>
            <label for="wasit-kabkota" class="block text-xs font-semibold text-zinc-300 mb-1.5">Kab / Kota</label>
            <input id="wasit-kabkota" type="text" bind:value={form.kabKota} placeholder="Contoh: SAMARINDA" class="{inputCls} uppercase" />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label for="wasit-lisensi" class="block text-xs font-semibold text-zinc-300 mb-1.5">Lisensi</label>
            <select id="wasit-lisensi" bind:value={form.lisensi} class="{inputCls} cursor-pointer">
              <option value="">— Belum diisi —</option>
              {#each LICENSE_OPTIONS as lic (lic)}
                <option value={lic}>{lic}</option>
              {/each}
              {#if form.lisensi && !LICENSE_OPTIONS.includes(form.lisensi)}
                <option value={form.lisensi}>{form.lisensi}</option>
              {/if}
            </select>
          </div>
          <div>
            <label for="wasit-nolisensi" class="block text-xs font-semibold text-zinc-300 mb-1.5">No. Lisensi</label>
            <input id="wasit-nolisensi" type="text" bind:value={form.noLisensi} class="{inputCls} font-mono" />
          </div>
        </div>

        <div>
          <label for="wasit-keterangan" class="block text-xs font-semibold text-zinc-300 mb-1.5">Keterangan</label>
          <input id="wasit-keterangan" type="text" bind:value={form.keterangan} placeholder="Contoh: CHIEF REFEREE" class="{inputCls} uppercase" />
        </div>

        <label class="flex items-center space-x-2.5 cursor-pointer pt-1">
          <input type="checkbox" bind:checked={form.isActive} class="rounded border-white/20 bg-black/40 text-white focus:ring-0 cursor-pointer w-4 h-4" />
          <span class="text-xs font-semibold text-zinc-200">Aktif (bisa dipilih di Tatami Manager)</span>
        </label>
      </div>

      <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-end space-x-2">
        <button
          type="button"
          onclick={() => (isFormOpen = false)}
          disabled={isSaving}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleSave}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          {isSaving ? "Menyimpan..." : editing ? "Simpan Perubahan" : "Simpan Wasit"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus -->
{#if toDelete}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none" role="dialog" aria-modal="true">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-sm shadow-2xl p-6 animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <h3 class="text-sm font-bold text-white tracking-tight">Hapus Data Wasit</h3>
      </div>
      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Hapus <strong class="text-white uppercase">{toDelete.nama_wasit}</strong> dari database? Penugasannya di tatami juga dicabut. Tindakan ini permanen.
      </p>
      <div class="mt-5 flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          type="button"
          onclick={() => (toDelete = null)}
          disabled={isDeleting}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={confirmDelete}
          disabled={isDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          {isDeleting ? "Menghapus..." : "Hapus Wasit"}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if isImportOpen}
  <WasitImportModal
    {eventId}
    onClose={() => (isImportOpen = false)}
    onDone={(sum) => {
      isImportOpen = false;
      onShowToast?.(`Import selesai: ${sum.created} baru, ${sum.updated} diperbarui, ${sum.unchanged} sudah ada.`);
      loadWasits();
      loadPerguruans();
    }}
  />
{/if}

{#if isBulkPhotoOpen}
  <WasitPhotoBulkModal
    {wasits}
    onClose={() => {
      isBulkPhotoOpen = false;
      loadWasits();
    }}
    onDone={(n) => {
      isBulkPhotoOpen = false;
      onShowToast?.(`${n} foto wasit tersimpan.`);
      loadWasits();
    }}
  />
{/if}
