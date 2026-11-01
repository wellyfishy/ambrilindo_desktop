<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { EnrichedAtletItem, UtusanItem, PerguruanItem, EnrichedUtusanItem, EnrichedPerguruanItem, NomorTandingItem, PullResult } from "$lib/types";
  import FuzzySelect, { type SelectOption } from "./FuzzySelect.svelte";

  interface Props {
    viewType: 'roster' | 'contingent' | 'sync';
    eventId: number;
    hostAddress: string;
  }

  let { viewType, eventId, hostAddress }: Props = $props();

  // Data State
  let athletes = $state<EnrichedAtletItem[]>([]);
  let contingents = $state<UtusanItem[]>([]);
  let perguruans = $state<PerguruanItem[]>([]);
  let categories = $state<NomorTandingItem[]>([]);
  let isLoading = $state(false);
  let isSyncing = $state(false);

  // Master Data (Contingent & Perguruan) State
  let activeContingentSubTab = $state<'contingent' | 'perguruan'>('contingent');
  let enrichedContingents = $state<EnrichedUtusanItem[]>([]);
  let enrichedPerguruans = $state<EnrichedPerguruanItem[]>([]);
  let masterSearchKeyword = $state("");

  let filteredEnrichedContingents = $derived<EnrichedUtusanItem[]>(
    enrichedContingents.filter((c) =>
      !masterSearchKeyword ||
      (c.nama_utusan && c.nama_utusan.toLowerCase().includes(masterSearchKeyword.toLowerCase()))
    )
  );

  let filteredEnrichedPerguruans = $derived<EnrichedPerguruanItem[]>(
    enrichedPerguruans.filter((p) =>
      !masterSearchKeyword ||
      (p.nama_perguruan && p.nama_perguruan.toLowerCase().includes(masterSearchKeyword.toLowerCase()))
    )
  );

  let totalContingentAthletes = $derived(
    enrichedContingents.reduce((acc, c) => acc + (c.athlete_count || 0), 0)
  );
  let totalPerguruanAthletes = $derived(
    enrichedPerguruans.reduce((acc, p) => acc + (p.athlete_count || 0), 0)
  );

  // Contingent Modal State
  let contingentModalOpen = $state(false);
  let isContingentEdit = $state(false);
  let isSavingContingent = $state(false);
  let contingentFormError = $state<string | null>(null);
  let contingentForm = $state<{
    id: number | null;
    nama_utusan: string;
    logo: string | null;
    new_logo_data: string | null;
    new_logo_filename: string | null;
  }>({
    id: null,
    nama_utusan: "",
    logo: null,
    new_logo_data: null,
    new_logo_filename: null,
  });

  // Perguruan Modal State
  let perguruanModalOpen = $state(false);
  let isPerguruanEdit = $state(false);
  let isSavingPerguruan = $state(false);
  let perguruanFormError = $state<string | null>(null);
  let perguruanForm = $state<{
    id: number | null;
    nama_perguruan: string;
  }>({
    id: null,
    nama_perguruan: "",
  });

  // Delete Master Modal State
  let deleteMasterModalOpen = $state(false);
  let masterTypeToDelete = $state<'contingent' | 'perguruan'>('contingent');
  let contingentToDelete = $state<EnrichedUtusanItem | null>(null);
  let perguruanToDelete = $state<EnrichedPerguruanItem | null>(null);
  let isDeletingMaster = $state(false);

  // Logo Cache Map
  let logoMap = $state<Record<string, string>>({});

  // Filter & Search State
  let searchKeyword = $state("");
  let selectedCategoryFilter = $state<string>("all");
  let selectedContingentFilter = $state<string>("all");
  let selectedStatusFilter = $state<'all' | 'tournament' | 'festival' | 'seed' | 'tanpa_nik'>('all');

  // Selection & Bulk State
  let selectedIds = $state<Set<number>>(new Set());

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

  let batchDeleteModalOpen = $state(false);
  let isBatchDeleting = $state(false);

  // Toast State
  let toastMsg = $state<string | null>(null);
  let toastTimer: any = null;

  function showToast(msg: string) {
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
      if (viewType === 'roster') {
        const [atletList, catList, utusanList, perguruanList] = await Promise.all([
          invoke<EnrichedAtletItem[]>("get_enriched_athletes", { eventId }),
          invoke<NomorTandingItem[]>("get_categories", { eventId }),
          invoke<UtusanItem[]>("get_contingents", { eventId }),
          invoke<PerguruanItem[]>("get_perguruans", { eventId }),
        ]);
        athletes = atletList;
        categories = catList;
        contingents = utusanList;
        perguruans = perguruanList;
        resolveLogos(atletList);
      } else if (viewType === 'contingent') {
        contingents = await invoke<UtusanItem[]>("get_contingents", { eventId });
      }
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
  let contingentCount = $derived(contingents.length);

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
    if (searchKeyword || selectedCategoryFilter || selectedContingentFilter || selectedStatusFilter) {
      currentPage = 1;
    }
  });

  // --- Selection Handlers ---
  function toggleSelectAll() {
    const currentIds = paginatedAthletes.map((a) => a.id);
    const allSelected = currentIds.every((id) => selectedIds.has(id));
    const next = new Set(selectedIds);
    if (allSelected) {
      currentIds.forEach((id) => next.delete(id));
    } else {
      currentIds.forEach((id) => next.add(id));
    }
    selectedIds = next;
  }

  function toggleSelectAthlete(id: number) {
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedIds = next;
  }

  function clearSelection() {
    selectedIds = new Set();
  }

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

      if (isEditMode && formInput.id) {
        const updated = await invoke<EnrichedAtletItem>("update_athlete", {
          id: formInput.id,
          input: payload,
        });
        athletes = athletes.map((a) => (a.id === updated.id ? updated : a));
        showToast(`Data ${updated.nama_atlet} berhasil diperbarui`);
      } else {
        const created = await invoke<EnrichedAtletItem>("create_athlete", {
          input: payload,
        });
        athletes = [created, ...athletes];
        showToast(`Atlet ${created.nama_atlet} berhasil ditambahkan`);
      }

      athleteModalOpen = false;
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
      selectedIds.delete(athleteToDelete.id);
      showToast(`Atlet ${athleteToDelete.nama_atlet} berhasil dihapus`);
      deleteModalOpen = false;
      athleteToDelete = null;
    } catch (err: any) {
      alert(`Gagal menghapus atlet: ${err}`);
    } finally {
      isDeleting = false;
    }
  }

  async function handleBatchSetPriority(priority: boolean) {
    if (selectedIds.size === 0) return;
    const ids = Array.from(selectedIds);
    let successCount = 0;
    for (const id of ids) {
      try {
        await invoke("toggle_athlete_priority", { id, isPriority: priority });
        successCount++;
      } catch (e) {
        console.error("Failed priority toggle for", id, e);
      }
    }
    athletes = athletes.map((a) => (selectedIds.has(a.id) ? { ...a, is_priority: priority } : a));
    clearSelection();
    showToast(`${successCount} atlet berhasil diatur status seed: ${priority ? 'Aktif' : 'Non-aktif'}`);
  }

  async function confirmBatchDelete() {
    if (selectedIds.size === 0) return;
    isBatchDeleting = true;
    try {
      const ids = Array.from(selectedIds);
      const count = await invoke<number>("delete_athletes_batch", { ids });
      athletes = athletes.filter((a) => !selectedIds.has(a.id));
      clearSelection();
      batchDeleteModalOpen = false;
      showToast(`${count} atlet berhasil dihapus dari database`);
    } catch (err: any) {
      alert(`Gagal menghapus batch: ${err}`);
    } finally {
      isBatchDeleting = false;
    }
  }

  $effect(() => {
    if (viewType && eventId) {
      loadData();
    }
  });

  onMount(() => {
    loadData();
  });

  async function resolveContingentLogos(items: EnrichedUtusanItem[]) {
    for (const item of items) {
      if (!item.logo) continue;
      if (logoMap[item.logo]) continue;
      if (item.logo.startsWith("http://") || item.logo.startsWith("https://") || item.logo.startsWith("data:")) {
        logoMap[item.logo] = item.logo;
        continue;
      }
      try {
        const dataUrl = await invoke<string | null>("get_media_data_url", { relativePath: item.logo });
        if (dataUrl) {
          logoMap[item.logo] = dataUrl;
        }
      } catch (e) {
        console.warn("Gagal resolve logo utusan:", item.logo, e);
      }
    }
  }

  function openCreateContingentModal() {
    isContingentEdit = false;
    contingentFormError = null;
    contingentForm = {
      id: null,
      nama_utusan: "",
      logo: null,
      new_logo_data: null,
      new_logo_filename: null,
    };
    contingentModalOpen = true;
  }

  function openEditContingentModal(c: EnrichedUtusanItem) {
    isContingentEdit = true;
    contingentFormError = null;
    contingentForm = {
      id: c.id,
      nama_utusan: c.nama_utusan || "",
      logo: c.logo || null,
      new_logo_data: null,
      new_logo_filename: null,
    };
    contingentModalOpen = true;
  }

  function handleLogoFileChange(event: Event) {
    const target = event.target as HTMLInputElement;
    if (target.files && target.files[0]) {
      const file = target.files[0];
      contingentForm.new_logo_filename = file.name;
      const reader = new FileReader();
      reader.onload = (e) => {
        contingentForm.new_logo_data = e.target?.result as string;
      };
      reader.readAsDataURL(file);
    }
  }

  function clearContingentLogo() {
    contingentForm.logo = null;
    contingentForm.new_logo_data = null;
    contingentForm.new_logo_filename = null;
  }

  async function handleSaveContingent() {
    contingentFormError = null;
    const name = contingentForm.nama_utusan.trim();
    if (!name) {
      contingentFormError = "Nama kontingen wajib diisi.";
      return;
    }
    isSavingContingent = true;
    try {
      let finalLogoPath = contingentForm.logo;
      if (contingentForm.new_logo_data && contingentForm.new_logo_filename) {
        finalLogoPath = await invoke<string>("save_contingent_logo", {
          fileName: contingentForm.new_logo_filename,
          base64Data: contingentForm.new_logo_data,
        });
        if (finalLogoPath) {
          logoMap[finalLogoPath] = contingentForm.new_logo_data;
        }
      }

      if (isContingentEdit && contingentForm.id) {
        await invoke("update_contingent", {
          id: contingentForm.id,
          namaUtusan: name,
          logo: finalLogoPath,
        });
        showToast(`Kontingen "${name}" berhasil diperbarui.`);
      } else {
        await invoke("create_contingent", {
          eventId,
          namaUtusan: name,
          logo: finalLogoPath,
        });
        showToast(`Kontingen "${name}" berhasil ditambahkan.`);
      }
      contingentModalOpen = false;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menyimpan kontingen:", err);
      contingentFormError = typeof err === "string" ? err : err?.message || JSON.stringify(err);
    } finally {
      isSavingContingent = false;
    }
  }

  function openDeleteContingentModal(c: EnrichedUtusanItem) {
    masterTypeToDelete = 'contingent';
    contingentToDelete = c;
    perguruanToDelete = null;
    deleteMasterModalOpen = true;
  }

  function openCreatePerguruanModal() {
    isPerguruanEdit = false;
    perguruanFormError = null;
    perguruanForm = {
      id: null,
      nama_perguruan: "",
    };
    perguruanModalOpen = true;
  }

  function openEditPerguruanModal(p: EnrichedPerguruanItem) {
    isPerguruanEdit = true;
    perguruanFormError = null;
    perguruanForm = {
      id: p.id,
      nama_perguruan: p.nama_perguruan || "",
    };
    perguruanModalOpen = true;
  }

  async function handleSavePerguruan() {
    perguruanFormError = null;
    const name = perguruanForm.nama_perguruan.trim();
    if (!name) {
      perguruanFormError = "Nama perguruan wajib diisi.";
      return;
    }
    isSavingPerguruan = true;
    try {
      if (isPerguruanEdit && perguruanForm.id) {
        await invoke("update_perguruan", {
          id: perguruanForm.id,
          namaPerguruan: name,
        });
        showToast(`Perguruan "${name}" berhasil diperbarui.`);
      } else {
        await invoke("create_perguruan", {
          eventId,
          namaPerguruan: name,
        });
        showToast(`Perguruan "${name}" berhasil ditambahkan.`);
      }
      perguruanModalOpen = false;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menyimpan perguruan:", err);
      perguruanFormError = typeof err === "string" ? err : err?.message || JSON.stringify(err);
    } finally {
      isSavingPerguruan = false;
    }
  }

  function openDeletePerguruanModal(p: EnrichedPerguruanItem) {
    masterTypeToDelete = 'perguruan';
    perguruanToDelete = p;
    contingentToDelete = null;
    deleteMasterModalOpen = true;
  }

  async function confirmDeleteMaster() {
    isDeletingMaster = true;
    try {
      if (masterTypeToDelete === 'contingent' && contingentToDelete) {
        await invoke("delete_contingent", { id: contingentToDelete.id });
        showToast(`Kontingen "${contingentToDelete.nama_utusan}" berhasil dihapus.`);
      } else if (masterTypeToDelete === 'perguruan' && perguruanToDelete) {
        await invoke("delete_perguruan", { id: perguruanToDelete.id });
        showToast(`Perguruan "${perguruanToDelete.nama_perguruan}" berhasil dihapus.`);
      }
      deleteMasterModalOpen = false;
      contingentToDelete = null;
      perguruanToDelete = null;
      await loadData();
    } catch (err: any) {
      console.error("Gagal menghapus data master:", err);
      showToast(`Gagal menghapus data: ${typeof err === "string" ? err : err?.message || JSON.stringify(err)}`);
    } finally {
      isDeletingMaster = false;
    }
  }

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
          {viewType === 'roster' ? 'DATA ATLET & PESERTA' : viewType === 'contingent' ? 'KONTINGEN & UTUSAN' : 'SINKRONISASI WEB'}
        </h2>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">
          {totalCount} PESERTA
        </span>
      </div>
      <p class="text-xs text-zinc-400 mt-1 font-normal">
        {viewType === 'roster' ? 'Verifikasi peserta turnamen, penomoran atlet, dan status unggulan' : 'Daftar kontingen dan dojo peserta terdaftar'}
      </p>
    </div>

    <!-- Action Buttons -->
    <div class="flex items-center space-x-2">
      {#if viewType === 'roster'}
        <button
          onclick={handleSyncPull}
          disabled={isSyncing}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 disabled:opacity-50"
          title="Tarik data pendaftaran atlet terbaru dari portal ambrilindo.com"
        >
          <svg class="w-3.5 h-3.5 text-zinc-400 {isSyncing ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          <span>{isSyncing ? "Menarik..." : "Tarik Atlet Web"}</span>
        </button>

        <button
          onclick={openCreateModal}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors flex items-center space-x-1.5 active:scale-95 shadow-md"
        >
          <svg class="w-3.5 h-3.5 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
          </svg>
          <span>+ Tambah Atlet</span>
        </button>
      {/if}
    </div>
  </div>

  <!-- Stat Cards Strip -->
  {#if viewType === 'roster'}
    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
      <!-- Total Atlet -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
        <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Total Peserta</span>
        <div class="flex items-baseline justify-between mt-1">
          <span class="text-2xl font-black font-mono text-white tabular-nums">{totalCount}</span>
          <span class="text-[10px] font-mono text-zinc-500">atlet</span>
        </div>
      </div>

      <!-- Turnamen -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
        <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Turnamen</span>
        <div class="flex items-baseline justify-between mt-1">
          <span class="text-2xl font-black font-mono text-white tabular-nums">{tournamentCount}</span>
          <span class="text-[10px] font-mono text-zinc-500">prestasi</span>
        </div>
      </div>

      <!-- Festival -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
        <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Festival</span>
        <div class="flex items-baseline justify-between mt-1">
          <span class="text-2xl font-black font-mono text-white tabular-nums">{festivalCount}</span>
          <span class="text-[10px] font-mono text-zinc-500">peserta</span>
        </div>
      </div>

      <!-- Seed / Prioritas -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Unggulan (Seed)</span>
          {#if seedCount > 0}
            <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
          {/if}
        </div>
        <div class="flex items-baseline justify-between mt-1">
          <span class="text-2xl font-black font-mono text-white tabular-nums">{seedCount}</span>
          <span class="text-[10px] font-mono text-zinc-500">prioritas</span>
        </div>
      </div>

      <!-- Tanpa NIK -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Tanpa NIK</span>
          {#if missingNikCount > 0}
            <span class="w-1.5 h-1.5 rounded-full bg-rose-400"></span>
          {/if}
        </div>
        <div class="flex items-baseline justify-between mt-1">
          <span class="text-2xl font-black font-mono {missingNikCount > 0 ? 'text-rose-300' : 'text-white'} tabular-nums">{missingNikCount}</span>
          <span class="text-[10px] font-mono text-zinc-500">perlu verif</span>
        </div>
      </div>
    </div>
  {/if}

  <!-- Toolbar Filters & Search -->
  {#if viewType === 'roster'}
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
          onclick={() => (selectedStatusFilter = 'tournament')}
          class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'tournament' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
        >
          Turnamen ({tournamentCount})
        </button>
        <button
          onclick={() => (selectedStatusFilter = 'festival')}
          class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'festival' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
        >
          Festival ({festivalCount})
        </button>
        <button
          onclick={() => (selectedStatusFilter = 'seed')}
          class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'seed' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
        >
          Seed ({seedCount})
        </button>
        <button
          onclick={() => (selectedStatusFilter = 'tanpa_nik')}
          class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border {selectedStatusFilter === 'tanpa_nik' ? 'bg-white text-black border-white shadow-sm' : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
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
          class="w-44"
        />

        <!-- Filter Kontingen -->
        <FuzzySelect
          items={contingentFilterOptions}
          bind:value={selectedContingentFilter}
          placeholder="Filter Kontingen"
          allowClear={true}
          clearLabel="Semua Kontingen"
          clearValue="all"
          compact={true}
          class="w-40"
        />

        <!-- Search Input -->
        <div class="relative w-56">
          <input
            type="text"
            bind:value={searchKeyword}
            placeholder="Cari nama, NIK, kontingen..."
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
  {/if}

  <!-- Bulk Actions Bar (When Items Selected) -->
  {#if selectedIds.size > 0 && viewType === 'roster'}
    <div class="bg-[#181a20] border border-white/20 rounded-xl px-4 py-2 mb-3 flex items-center justify-between shadow-lg shrink-0">
      <div class="flex items-center space-x-2">
        <span class="w-1.5 h-1.5 rounded-full bg-white"></span>
        <span class="text-xs font-bold text-white font-mono tabular-nums">{selectedIds.size} atlet terpilih</span>
      </div>

      <div class="flex items-center space-x-2">
        <button
          onclick={() => handleBatchSetPriority(true)}
          class="h-7 px-3 bg-white/[0.06] hover:bg-white/[0.12] text-zinc-200 text-xs font-semibold rounded-lg border border-white/[0.1] transition-colors active:scale-95"
        >
          Set Seed
        </button>
        <button
          onclick={() => handleBatchSetPriority(false)}
          class="h-7 px-3 bg-white/[0.06] hover:bg-white/[0.12] text-zinc-200 text-xs font-semibold rounded-lg border border-white/[0.1] transition-colors active:scale-95"
        >
          Hapus Seed
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

  <!-- Main Content Table / Grid -->
  <div class="flex-1 overflow-y-auto min-h-0 rounded-2xl border border-white/[0.08] bg-[#181a20] shadow-lg flex flex-col">
    {#if isLoading}
      <div class="flex-1 flex flex-col items-center justify-center text-zinc-500 text-sm space-y-2">
        <svg class="w-6 h-6 animate-spin text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span class="text-xs font-mono text-zinc-400">Memuat data peserta dari database...</span>
      </div>
    {:else if viewType === 'roster'}
      <!-- Athletes Table View -->
      {#if filteredAthletes.length === 0}
        <div class="flex-1 flex flex-col items-center justify-center text-center p-8">
          <p class="text-sm font-bold text-zinc-300">Tidak ada data atlet yang sesuai</p>
          <p class="text-xs text-zinc-500 mt-1 max-w-sm font-normal">
            {searchKeyword ? "Coba gunakan kata kunci pencarian lain atau reset filter kategori." : "Belum ada atlet yang terdaftar. Tarik data dari web atau tambahkan atlet secara manual."}
          </p>
          {#if searchKeyword || selectedCategoryFilter !== 'all' || selectedContingentFilter !== 'all' || selectedStatusFilter !== 'all'}
            <button
              onclick={() => {
                searchKeyword = "";
                selectedCategoryFilter = "all";
                selectedContingentFilter = "all";
                selectedStatusFilter = "all";
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
                    aria-label="Pilih semua atlet di halaman ini"
                    checked={paginatedAthletes.length > 0 && paginatedAthletes.every((a) => selectedIds.has(a.id))}
                    onchange={toggleSelectAll}
                    class="rounded border-white/20 bg-black/40 text-white focus:ring-0 cursor-pointer"
                  />
                </th>
                <th class="px-3 py-3 w-12 text-center font-bold">NO</th>
                <th class="px-4 py-3 font-bold">NAMA ATLET & IDENTITAS</th>
                <th class="px-4 py-3 font-bold">KONTINGEN & PERGURUAN</th>
                <th class="px-4 py-3 font-bold">NOMOR TANDING / KELAS</th>
                <th class="px-3 py-3 text-center w-24 font-bold">SEED</th>
                <th class="px-4 py-3 text-right w-24 font-bold pr-5">AKSI</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-white/[0.04] font-medium">
              {#each paginatedAthletes as atlet, idx (atlet.id)}
                <tr class="hover:bg-white/[0.02] transition-colors {selectedIds.has(atlet.id) ? 'bg-white/[0.04]' : ''}">
                  <!-- Checkbox -->
                  <td class="px-4 py-2.5 text-center">
                    <input
                      type="checkbox"
                      aria-label={`Pilih atlet ${atlet.nama_atlet}`}
                      checked={selectedIds.has(atlet.id)}
                      onchange={() => toggleSelectAthlete(atlet.id)}
                      class="rounded border-white/20 bg-black/40 text-white focus:ring-0 cursor-pointer"
                    />
                  </td>

                  <!-- Number -->
                  <td class="px-3 py-2.5 text-center text-zinc-500 font-mono text-[11px] tabular-nums">
                    {(currentPage - 1) * pageSize + idx + 1}
                  </td>

                  <!-- Athlete Name & Identification -->
                  <td class="px-4 py-2.5">
                    <div class="flex items-center space-x-2">
                      <span class="font-bold text-white text-[12.5px] uppercase tracking-tight">
                        {atlet.nama_atlet}
                      </span>
                      {#if atlet.is_priority}
                        <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/20">
                          SEED
                        </span>
                      {/if}
                    </div>

                    <div class="flex items-center space-x-1.5 mt-1">
                      {#if atlet.nik}
                        <span class="font-mono text-[10px] text-zinc-400 bg-white/[0.04] px-1.5 py-0.5 rounded border border-white/[0.06] tabular-nums">
                          NIK {atlet.nik}
                        </span>
                      {:else}
                        <span class="font-mono text-[9.5px] text-rose-400 bg-rose-500/10 px-1.5 py-0.5 rounded border border-rose-500/20">
                          TANPA NIK
                        </span>
                      {/if}

                      {#if atlet.kode_atlet}
                        <span class="font-mono text-[10px] text-zinc-400 bg-white/[0.04] px-1.5 py-0.5 rounded border border-white/[0.06]">
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

                  <!-- Contingent & Dojo -->
                  <td class="px-4 py-2.5">
                    <div class="flex items-center space-x-2">
                      {#if atlet.utusan_logo && logoMap[atlet.utusan_logo]}
                        <img
                          src={logoMap[atlet.utusan_logo]}
                          alt="Logo"
                          class="w-5 h-5 object-contain shrink-0 rounded-xs"
                        />
                      {/if}
                      <div class="truncate leading-tight">
                        <div class="font-bold text-zinc-200 text-xs truncate uppercase">
                          {atlet.utusan_nama || "KONTINGEN #" + (atlet.utusan_id || "-")}
                        </div>
                        <div class="text-[10px] text-zinc-400 truncate mt-0.5">
                          {atlet.perguruan_nama || "—"}
                        </div>
                      </div>
                    </div>
                  </td>

                  <!-- Category -->
                  <td class="px-4 py-2.5">
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
                        <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/20">
                          FESTIVAL
                        </span>
                      {:else if atlet.is_bob}
                        <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-purple-500/10 text-purple-300 border border-purple-500/20">
                          BOB
                        </span>
                      {:else}
                        <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.04] text-zinc-400 border border-white/[0.06]">
                          PRESTASI
                        </span>
                      {/if}
                    </div>
                  </td>

                  <!-- Seed Toggle -->
                  <td class="px-3 py-2.5 text-center">
                    <button
                      onclick={() => handleTogglePriority(atlet)}
                      class="h-6 px-2.5 rounded text-[10px] font-mono font-bold transition-colors border {atlet.is_priority ? 'bg-amber-500/10 text-amber-300 border-amber-500/25 hover:bg-amber-500/20' : 'bg-white/[0.04] text-zinc-500 border-white/[0.08] hover:text-zinc-300'}"
                      title="Klik untuk mengubah status unggulan"
                    >
                      {atlet.is_priority ? "SEED" : "—"}
                    </button>
                  </td>

                  <!-- Actions -->
                  <td class="px-4 py-2.5 text-right pr-5">
                    <div class="flex items-center justify-end space-x-1">
                      <button
                        onclick={() => openEditModal(atlet)}
                        class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition-colors"
                        title="Edit data atlet"
                      >
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                        </svg>
                      </button>
                      <button
                        onclick={() => openDeleteSingleModal(atlet)}
                        class="p-1.5 text-zinc-400 hover:text-rose-400 rounded-lg hover:bg-white/[0.08] transition-colors"
                        title="Hapus atlet"
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
            <span>Menampilkan {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, filteredAthletes.length)} dari {filteredAthletes.length} atlet</span>
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
    {:else if viewType === 'contingent'}
      <!-- Contingents Grid View -->
      <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3 p-4">
        {#each contingents as contingent}
          <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-3 flex items-center space-x-3 hover:border-white/20 transition-colors">
            {#if contingent.logo && logoMap[contingent.logo]}
              <img src={logoMap[contingent.logo]} alt="Logo" class="w-9 h-9 object-contain shrink-0" />
            {:else}
              <div class="w-9 h-9 rounded-lg bg-white/[0.05] border border-white/[0.08] flex items-center justify-center font-mono font-bold text-xs text-zinc-300 shrink-0">
                {contingent.nama_utusan ? contingent.nama_utusan.charAt(0).toUpperCase() : 'U'}
              </div>
            {/if}
            <div class="truncate">
              <h4 class="text-xs font-bold text-white truncate uppercase">{contingent.nama_utusan}</h4>
              <p class="text-[10px] font-mono text-zinc-500 tabular-nums">ID #{contingent.id}</p>
            </div>
          </div>
        {/each}
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
        <button onclick={() => (athleteModalOpen = false)} class="text-zinc-500 hover:text-white transition-colors">
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
            placeholder="Contoh: Ahmad Fadillah"
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

        <!-- Kontingen & Perguruan Row -->
        <div class="grid grid-cols-2 gap-3">
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
          <div>
            <label for="perguruan" class="block text-xs font-semibold text-zinc-300 mb-1.5">Perguruan</label>
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
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleSaveAthlete}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
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
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={confirmDeleteSingle}
          disabled={isDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isDeleting ? "Menghapus..." : "Hapus Atlet"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Batch -->
{#if batchDeleteModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-sm shadow-2xl p-6 animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">Hapus {selectedIds.size} Atlet Terpilih</h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Apakah Anda yakin ingin menghapus <strong>{selectedIds.size} atlet</strong> yang dipilih dari database turnamen? Atlet yang sudah terikat pada partai bagan tidak akan terhapus demi integritas bracket.
      </p>

      <div class="mt-5 flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          onclick={() => (batchDeleteModalOpen = false)}
          disabled={isBatchDeleting}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={confirmBatchDelete}
          disabled={isBatchDeleting}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isBatchDeleting ? "Menghapus..." : `Hapus ${selectedIds.size} Atlet`}
        </button>
      </div>
    </div>
  </div>
{/if}


<!-- Modal: Tambah / Edit Kontingen -->
{#if contingentModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-md shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <!-- Header -->
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight">
          {isContingentEdit ? "Edit Kontingen / Utusan" : "Tambah Kontingen Baru"}
        </h3>
        <button onclick={() => (contingentModalOpen = false)} class="text-zinc-500 hover:text-white transition-colors">
          ✕
        </button>
      </div>

      <!-- Body -->
      <div class="p-6 space-y-4">
        {#if contingentFormError}
          <div class="bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs px-3 py-2 rounded-xl">
            {contingentFormError}
          </div>
        {/if}

        <!-- Nama Kontingen -->
        <div>
          <label for="nama_kontingen_input" class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Nama Kontingen / Utusan <span class="text-rose-400">*</span>
          </label>
          <input
            id="nama_kontingen_input"
            type="text"
            bind:value={contingentForm.nama_utusan}
            placeholder="Contoh: KONTINGEN KOTA SURABAYA"
            class="w-full bg-[#111215] text-xs font-semibold text-white uppercase px-3 py-2.5 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors"
          />
        </div>

        <!-- Logo Upload Section -->
        <div>
          <span class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Logo Kontingen
          </span>
          
          <div class="flex items-center space-x-3 p-3 bg-[#111215] rounded-xl border border-white/[0.08]">
            {#if contingentForm.new_logo_data}
              <div class="w-14 h-14 rounded-lg bg-white/[0.04] border border-white/[0.08] p-1 flex items-center justify-center shrink-0">
                <img src={contingentForm.new_logo_data} alt="Preview" class="w-full h-full object-contain" />
              </div>
            {:else if contingentForm.logo && logoMap[contingentForm.logo]}
              <div class="w-14 h-14 rounded-lg bg-white/[0.04] border border-white/[0.08] p-1 flex items-center justify-center shrink-0">
                <img src={logoMap[contingentForm.logo]} alt="Existing Logo" class="w-full h-full object-contain" />
              </div>
            {:else}
              <div class="w-14 h-14 rounded-lg bg-white/[0.03] border border-dashed border-white/20 flex flex-col items-center justify-center text-zinc-500 shrink-0">
                <svg class="w-5 h-5 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                </svg>
                <span class="text-[9px] mt-0.5">No Logo</span>
              </div>
            {/if}

            <div class="flex-1 space-y-1.5">
              <label for="logo_file_input" class="cursor-pointer inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-white/[0.06] hover:bg-white/[0.12] text-xs font-semibold text-zinc-200 hover:text-white border border-white/[0.1] transition-colors">
                <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                </svg>
                <span>Pilih File Gambar</span>
                <input
                  id="logo_file_input"
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/svg+xml"
                  onchange={handleLogoFileChange}
                  class="hidden"
                />
              </label>
              {#if contingentForm.new_logo_data || contingentForm.logo}
                <div>
                  <button
                    type="button"
                    onclick={clearContingentLogo}
                    class="text-[11px] text-rose-400 hover:text-rose-300 font-medium"
                  >
                    Hapus Logo
                  </button>
                </div>
              {/if}
              <p class="text-[10px] text-zinc-500">Format PNG, JPG, WebP atau SVG</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-end space-x-2">
        <button
          onclick={() => (contingentModalOpen = false)}
          disabled={isSavingContingent}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleSaveContingent}
          disabled={isSavingContingent}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isSavingContingent ? "Menyimpan..." : isContingentEdit ? "Simpan Perubahan" : "Simpan Kontingen"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Tambah / Edit Perguruan -->
{#if perguruanModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-md shadow-2xl overflow-hidden flex flex-col animate-in zoom-in-95 duration-150">
      <!-- Header -->
      <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between">
        <h3 class="text-sm font-bold text-white tracking-tight">
          {isPerguruanEdit ? "Edit Perguruan / Dojo" : "Tambah Perguruan Baru"}
        </h3>
        <button onclick={() => (perguruanModalOpen = false)} class="text-zinc-500 hover:text-white transition-colors">
          ✕
        </button>
      </div>

      <!-- Body -->
      <div class="p-6 space-y-4">
        {#if perguruanFormError}
          <div class="bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs px-3 py-2 rounded-xl">
            {perguruanFormError}
          </div>
        {/if}

        <div>
          <label for="nama_perguruan_input" class="block text-xs font-semibold text-zinc-300 mb-1.5">
            Nama Perguruan / Dojo <span class="text-rose-400">*</span>
          </label>
          <input
            id="nama_perguruan_input"
            type="text"
            bind:value={perguruanForm.nama_perguruan}
            placeholder="Contoh: INKAI / LEMKARI / KERA SAKTI"
            class="w-full bg-[#111215] text-xs font-semibold text-white uppercase px-3 py-2.5 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors"
          />
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-end space-x-2">
        <button
          onclick={() => (perguruanModalOpen = false)}
          disabled={isSavingPerguruan}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={handleSavePerguruan}
          disabled={isSavingPerguruan}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isSavingPerguruan ? "Menyimpan..." : isPerguruanEdit ? "Simpan Perubahan" : "Simpan Perguruan"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Konfirmasi Hapus Master (Kontingen / Perguruan) -->
{#if deleteMasterModalOpen}
  {@const targetName = masterTypeToDelete === 'contingent' ? contingentToDelete?.nama_utusan : perguruanToDelete?.nama_perguruan}
  {@const count = masterTypeToDelete === 'contingent' ? (contingentToDelete?.athlete_count || 0) : (perguruanToDelete?.athlete_count || 0)}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4 select-none">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl w-full max-w-sm shadow-2xl p-6 animate-in zoom-in-95 duration-150">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white tracking-tight">
            Hapus {masterTypeToDelete === 'contingent' ? 'Kontingen' : 'Perguruan'}
          </h3>
        </div>
      </div>

      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Apakah Anda yakin ingin menghapus <strong class="text-white uppercase">{targetName}</strong> dari database turnamen? Tindakan ini bersifat permanen.
      </p>

      {#if count > 0}
        <div class="mt-3 p-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-300 text-[11px] leading-snug">
          ⚠️ <strong>Perhatian:</strong> Terdapat <strong>{count} atlet</strong> yang terdaftar pada {masterTypeToDelete === 'contingent' ? 'kontingen' : 'perguruan'} ini. Menghapus data ini akan melepaskan afiliasi atlet tersebut.
        </div>
      {/if}

      <div class="mt-5 flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button
          onclick={() => { deleteMasterModalOpen = false; contingentToDelete = null; perguruanToDelete = null; }}
          disabled={isDeletingMaster}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors"
        >
          Batal
        </button>
        <button
          onclick={confirmDeleteMaster}
          disabled={isDeletingMaster}
          class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl transition-colors shadow-md active:scale-95 disabled:opacity-50"
        >
          {isDeletingMaster ? "Menghapus..." : "Hapus Permanen"}
        </button>
      </div>
    </div>
  </div>
{/if}