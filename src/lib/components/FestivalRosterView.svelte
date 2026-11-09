<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import { printWithOrientation } from "$lib/print_helper";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import FestivalSmartMixModal from "$lib/components/FestivalSmartMixModal.svelte";
  import FestivalAutoMixModal from "$lib/components/FestivalAutoMixModal.svelte";
  import type { FestivalMixGroupData } from "$lib/festival_mix_helper";
  import type {
    FestivalRosterData,
    FestivalRosterState,
    FestivalTatamiSection,
    FestivalCategory,
    FestivalMatch,
  } from "$lib/types";

  interface Props {
    eventId: number;
    onShowToast?: (message: string, type?: "success" | "error" | "info") => void;
  }

  let { eventId, onShowToast = () => {} }: Props = $props();

  // --- Core State ---
  let data = $state<FestivalRosterData | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let isDirty = $state(false);

  // --- Filter State ---
  let selectedTatamiId = $state<number | "all">("all");
  let selectedDiscipline = $state<"all" | "kata" | "kumite">("all");
  let searchQuery = $state("");

  // --- Mix Modal State ---
  let mixModalOpen = $state(false);
  let mixSourceIds = $state<number[]>([]);
  let mixNewName = $state("");
  let mixDeleteSources = $state(true);
  let isMixing = $state(false);

  // Non-Destructive Mix Groups & Auto Mix Modal State
  let mixGroups = $state<FestivalMixGroupData[]>([]);
  let autoMixModalOpen = $state(false);

  // Smart Mix Assistant State
  let smartMixModalOpen = $state(false);
  let smartMixSourceCategory = $state<FestivalCategory | null>(null);

  function openSmartMixForCategory(cat: FestivalCategory) {
    smartMixSourceCategory = cat;
    smartMixModalOpen = true;
  }

  // --- Print View State (Table Roster) ---
  let printModalOpen = $state(false);
  let printSelectedTatamiId = $state<number | "all">("all");

  $effect(() => {
    if (eventId) {
      loadFestivalData();
    }
  });

  onMount(() => {
    loadFestivalData();
  });

  async function loadFestivalData() {
    isLoading = true;
    try {
      const [res, mg] = await Promise.all([
        invoke<FestivalRosterData>("get_festival_roster_data", { eventId }),
        invoke<FestivalMixGroupData[]>("get_festival_mix_groups", { eventId }).catch(() => []),
      ]);
      data = res;
      mixGroups = mg || [];
      isDirty = false;
    } catch (e: any) {
      console.error("Gagal memuat data festival roster:", e);
      onShowToast(`Gagal memuat data: ${e}`, "error");
    } finally {
      isLoading = false;
    }
  }

  // --- Computed Sections based on filters ---
  let filteredSections = $derived.by(() => {
    if (!data) return [];

    return data.sections
      .filter((sec) => selectedTatamiId === "all" || sec.tatami_id === selectedTatamiId)
      .map((sec) => {
        const filteredCategories = sec.categories.filter((cat) => {
          const matchDisc =
            selectedDiscipline === "all" || cat.discipline === selectedDiscipline;
          const matchQuery =
            !searchQuery.trim() ||
            cat.nama.toLowerCase().includes(searchQuery.toLowerCase()) ||
            cat.display_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
            cat.matches.some(
              (m) =>
                m.aka.nama.toLowerCase().includes(searchQuery.toLowerCase()) ||
                (m.ao && m.ao.nama.toLowerCase().includes(searchQuery.toLowerCase())) ||
                m.aka.utusan.toLowerCase().includes(searchQuery.toLowerCase()) ||
                (m.ao && m.ao.utusan.toLowerCase().includes(searchQuery.toLowerCase()))
            );
          return matchDisc && matchQuery;
        });

        return {
          ...sec,
          categories: filteredCategories,
        };
      })
      .filter((sec) => sec.categories.length > 0 || selectedTatamiId !== "all");
  });

  // Flat list of all categories for mix modal
  let allCategoriesList = $derived.by(() => {
    if (!data) return [];
    const list: FestivalCategory[] = [];
    for (const sec of data.sections) {
      for (const cat of sec.categories) {
        list.push(cat);
      }
    }
    return list;
  });

  // Categories as stored in the DB: a mixed category is split back into its member classes,
  // so the mix dialogs work with real ids and athlete counts.
  let rawCategoriesList = $derived.by(() =>
    allCategoriesList.flatMap((c) =>
      c.mixed_from.length > 0
        ? c.mixed_from.map((m) => ({ id: m.id, nama: m.nama, athlete_count: m.atlet_count }))
        : [{ id: c.id, nama: c.nama, athlete_count: c.atlet_count }]
    )
  );

  async function releaseMix(cat: FestivalCategory) {
    const remaining = mixGroups.filter((g) => g.primary_category_id !== cat.id);
    try {
      await invoke("save_festival_mix_groups", { eventId, mixGroups: remaining });
      onShowToast(`Mix ${cat.display_name} dilepas.`, "success");
      await loadFestivalData();
    } catch (e: any) {
      onShowToast(`Gagal melepas mix: ${e}`, "error");
    }
  }

  // --- Print rows: one tatami = category title rows + athlete rows numbered continuously ---
  type PrintRow =
    | { kind: "category"; key: string; name: string }
    | { kind: "athlete"; key: string; no: number; nama: string; perguruan: string; utusan: string; sabuk: "M" | "B" };

  function buildPrintRows(sec: FestivalTatamiSection): PrintRow[] {
    const rows: PrintRow[] = [];
    let no = 0;
    for (const cat of sec.categories) {
      rows.push({ kind: "category", key: `c${cat.id}`, name: cat.display_name });
      for (const m of cat.matches) {
        for (const [ath, sabuk] of [[m.aka, "M"], [m.ao, "B"]] as const) {
          if (!ath) continue;
          no += 1;
          rows.push({
            kind: "athlete",
            key: `a${cat.id}-${ath.id}-${sabuk}`,
            no,
            nama: ath.nama,
            perguruan: ath.perguruan,
            utusan: ath.utusan,
            sabuk,
          });
        }
      }
    }
    return rows;
  }

  let printOrgName = $derived(data?.kop_surat?.nama_organisasi?.trim() || data?.event_name || "");
  let printKopMeta = $derived.by(() => {
    const kop = data?.kop_surat;
    if (!kop) return "";
    const alamat = (kop.alamat || "").split("\n").map((l) => l.trim()).filter(Boolean).join(", ");
    return [alamat, (kop.kontak || "").trim()].filter(Boolean).join("  •  ");
  });
  let printEventLine = $derived(
    data?.kop_surat?.nama_organisasi?.trim() && data?.event_name ? data.event_name : ""
  );

  // --- Actions ---

  function markDirty() {
    isDirty = true;
  }

  // Swap Aka and Ao within a match
  function swapMatchCorners(catId: number, matchNum: number) {
    if (!data) return;
    for (const sec of data.sections) {
      for (const cat of sec.categories) {
        if (cat.id === catId) {
          const m = cat.matches.find((match) => match.match_num === matchNum);
          if (m && m.ao) {
            const temp = m.aka;
            m.aka = m.ao;
            m.ao = temp;
            markDirty();
            return;
          }
        }
      }
    }
  }

  // Move Category to Another Tatami
  function moveCategoryTatami(catId: number, targetTatamiId: number) {
    if (!data) return;
    let targetCat: FestivalCategory | null = null;

    // Find and remove from current section
    for (const sec of data.sections) {
      const idx = sec.categories.findIndex((c) => c.id === catId);
      if (idx !== -1) {
        targetCat = sec.categories.splice(idx, 1)[0];
        sec.total_athletes -= targetCat.atlet_count;
        sec.total_matches -= targetCat.match_count;
        break;
      }
    }

    if (targetCat) {
      targetCat.tatami_id = targetTatamiId;
      for (const sec of data.sections) {
        if (sec.tatami_id === targetTatamiId) {
          sec.categories.push(targetCat);
          sec.total_athletes += targetCat.atlet_count;
          sec.total_matches += targetCat.match_count;
          break;
        }
      }
      markDirty();
    }
  }

  // Move Category Up or Down inside its Tatami
  function moveCategoryOrder(secIdx: number, catIdx: number, dir: "up" | "down") {
    if (!data) return;
    const sec = data.sections[secIdx];
    if (!sec) return;

    if (dir === "up" && catIdx > 0) {
      const temp = sec.categories[catIdx];
      sec.categories[catIdx] = sec.categories[catIdx - 1];
      sec.categories[catIdx - 1] = temp;
      markDirty();
    } else if (dir === "down" && catIdx < sec.categories.length - 1) {
      const temp = sec.categories[catIdx];
      sec.categories[catIdx] = sec.categories[catIdx + 1];
      sec.categories[catIdx + 1] = temp;
      markDirty();
    }
  }

  // Build FestivalRosterState for saving
  function buildCurrentRosterState(): FestivalRosterState {
    const tatami_assignments: Record<string, number[]> = {};
    const athlete_pairings: Record<string, number[]> = {};

    if (data) {
      for (const sec of data.sections) {
        const catIds = sec.categories.map((c) => c.id);
        tatami_assignments[sec.tatami_id.toString()] = catIds;

        for (const cat of sec.categories) {
          const athIds: number[] = [];
          for (const m of cat.matches) {
            athIds.push(m.aka.id);
            if (m.ao) {
              athIds.push(m.ao.id);
            }
          }
          athlete_pairings[cat.id.toString()] = athIds;
        }
      }
    }

    return { tatami_assignments, athlete_pairings };
  }

  async function handleSaveState() {
    if (!data) return;
    isSaving = true;
    try {
      const statePayload = buildCurrentRosterState();
      await invoke("save_festival_roster_state", {
        eventId,
        rosterState: statePayload,
      });
      isDirty = false;
      onShowToast("Roster festival berhasil disimpan ke database.", "success");
    } catch (e: any) {
      console.error("Gagal menyimpan roster festival:", e);
      onShowToast(`Gagal menyimpan: ${e}`, "error");
    } finally {
      isSaving = false;
    }
  }

  async function handleAutoPairAll() {
    isLoading = true;
    try {
      const res: FestivalRosterData = await invoke("auto_pair_festival_category", {
        eventId,
        nomorTandingId: null,
      });
      data = res;
      isDirty = false;
      onShowToast("Auto-pairing anti-bentrok berhasil diterapkan ke seluruh kategori.", "success");
    } catch (e: any) {
      console.error("Gagal menjalankan auto-pairing:", e);
      onShowToast(`Gagal auto-pairing: ${e}`, "error");
    } finally {
      isLoading = false;
    }
  }

  async function handleAutoPairSingle(catId: number) {
    try {
      const res: FestivalRosterData = await invoke("auto_pair_festival_category", {
        eventId,
        nomorTandingId: catId,
      });
      data = res;
      isDirty = false;
      onShowToast("Kategori berhasil di-pairing anti-bentrok.", "success");
    } catch (e: any) {
      console.error("Gagal auto-pairing single category:", e);
      onShowToast(`Gagal auto-pairing: ${e}`, "error");
    }
  }

  // Open Mix Modal
  function openMixModal() {
    const sparseCat = allCategoriesList.find((c) => c.atlet_count <= 2 && c.mixed_from.length === 0);
    if (sparseCat) {
      openSmartMixForCategory(sparseCat);
      return;
    }
    mixSourceIds = [];
    mixNewName = "";
    mixDeleteSources = true;
    mixModalOpen = true;
  }

  function toggleMixSource(id: number) {
    if (mixSourceIds.includes(id)) {
      mixSourceIds = mixSourceIds.filter((x) => x !== id);
    } else {
      mixSourceIds = [...mixSourceIds, id];
    }
  }

  async function executeMixCategories() {
    if (mixSourceIds.length === 0) {
      onShowToast("Pilih minimal 1 kategori festival untuk digabung.", "error");
      return;
    }
    if (!mixNewName.trim()) {
      onShowToast("Masukkan nama kategori gabungan baru.", "error");
      return;
    }

    isMixing = true;
    try {
      const res: FestivalRosterData = await invoke("mix_festival_categories", {
        eventId,
        sourceIds: mixSourceIds,
        targetId: null,
        newName: mixNewName.trim().toUpperCase(),
        deleteSources: mixDeleteSources,
      });
      data = res;
      isDirty = false;
      mixModalOpen = false;
      onShowToast("Penggabungan kelas festival berhasil diterapkan.", "success");
    } catch (e: any) {
      console.error("Gagal menggabungkan kelas:", e);
      onShowToast(`Gagal menggabungkan kelas: ${e}`, "error");
    } finally {
      isMixing = false;
    }
  }

  // Direct Launch to Control Panel for a Festival Match
  async function launchControlPanelForMatch(
    m: FestivalMatch,
    tatamiNum: number,
    catName: string,
    discipline: string
  ) {
    try {
      await invoke("open_festival_control_panel", {
        tatami: tatamiNum,
        akaNama: m.aka.nama,
        akaUtusan: m.aka.utusan,
        aoNama: m.ao ? m.ao.nama : null,
        aoUtusan: m.ao ? m.ao.utusan : null,
        catName: catName,
        discipline: discipline,
      });
      onShowToast(
        `Control Panel dibuka di Tatami ${tatamiNum} (${m.aka.nama} vs ${m.ao ? m.ao.nama : "BYE"}).`,
        "info"
      );
    } catch (e: any) {
      console.error("Gagal membuka control panel:", e);
      onShowToast(`Gagal membuka Control Panel: ${e}`, "error");
    }
  }

  function triggerPrint() {
    printWithOrientation("portrait", "8mm");
  }
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative festival-editor-main {printModalOpen ? 'hidden print:hidden' : 'print:hidden'}">
  <!-- Header Bar -->
  <div class="flex items-center justify-between pb-4 border-b border-white/[0.08] shrink-0">
    <div class="flex items-center space-x-2.5">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">
        Roster Maker Festival
      </h2>
      <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
        {data?.total_categories || 0} KATEGORI • {data?.total_athletes || 0} ATLET
      </span>
      {#if isDirty}
        <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 animate-pulse">
          BELUM DISIMPAN
        </span>
      {/if}
    </div>

    <!-- Actions Toolbar -->
    <div class="flex items-center space-x-2">
      <!-- Auto-Pairing Anti-Bentrok -->
      <button
        onclick={handleAutoPairAll}
        class="h-9 px-3.5 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-600 hover:to-amber-700 text-black font-black text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5"
        title="Jalankan algoritma anti-bentrok kontingen di seluruh kelas festival"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span>Auto-Pairing Anti-Bentrok</span>
      </button>

      <!-- Auto Mix Kategori Solo -->
      <button
        onclick={() => (autoMixModalOpen = true)}
        class="h-9 px-3 bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 hover:text-amber-200 font-bold text-xs rounded-xl border border-amber-500/30 transition-all flex items-center space-x-1.5 cursor-pointer"
        title="Otomatis mencocokkan dan menyusun bagan mix untuk seluruh kategori solo"
      >
        <span>⚡ Auto Mix Solo</span>
      </button>

      <!-- Mix / Gabung Kelas -->
      <button
        onclick={openMixModal}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-all flex items-center space-x-1.5 cursor-pointer"
        title="Gabungkan kategori festival yang sepi / solo"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
        <span>Mix Manual</span>
      </button>

      <!-- Cetak Roster -->
      <button
        onclick={() => (printModalOpen = true)}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-sky-300 hover:text-white font-semibold text-xs rounded-xl border border-sky-500/20 transition-all flex items-center space-x-1.5"
        title="Buka Pratinjau Dokumen Siap Cetak Roster Festival (Tabel Run Sheet)"
      >
        <svg class="w-3.5 h-3.5 text-sky-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak Roster</span>
      </button>

      <!-- Simpan Perubahan -->
      <button
        onclick={handleSaveState}
        disabled={isSaving}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5 disabled:opacity-50"
      >
        {#if isSaving}
          <span class="w-3.5 h-3.5 border-2 border-black border-t-transparent rounded-full animate-spin"></span>
          <span>Menyimpan...</span>
        {:else}
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
          </svg>
          <span>Simpan Perubahan</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- KPI Stats Cards (5-column responsive strip) -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Total Kategori</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{data?.total_categories || 0}</span>
        <span class="text-[10px] font-mono text-zinc-500">kelas</span>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-sky-400 uppercase">Total Atlet Festival</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{data?.total_athletes || 0}</span>
        <span class="text-[10px] font-mono text-zinc-500">atlet</span>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-amber-400 uppercase">Total Match 1-on-1</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{data?.total_matches || 0}</span>
        <span class="text-[10px] font-mono text-zinc-500">partai</span>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-purple-400 uppercase">Arena Tatami</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{data?.tatamis.length || 0}</span>
        <span class="text-[10px] font-mono text-zinc-500">tatami</span>
      </div>
    </div>

    <div class="bg-[#181a20] border {(data?.total_clashes || 0) > 0 ? 'border-rose-500/50 bg-rose-500/[0.05]' : 'border-white/[0.08]'} rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider {(data?.total_clashes || 0) > 0 ? 'text-rose-400' : 'text-emerald-400'} uppercase">Bentrok Kontingen</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono {(data?.total_clashes || 0) > 0 ? 'text-rose-400 animate-pulse' : 'text-emerald-400'}">{data?.total_clashes || 0}</span>
        <span class="text-[10px] font-mono {(data?.total_clashes || 0) > 0 ? 'text-rose-400' : 'text-emerald-400'}">
          {(data?.total_clashes || 0) > 0 ? 'konflik 1-dojo' : 'aman'}
        </span>
      </div>
    </div>
  </div>

  <!-- Toolbar: Segmented Tatami Filter Pills, Discipline Filter & Search -->
  <div class="flex flex-wrap items-center justify-between gap-3 pb-3 shrink-0">
    <!-- Tatami Filter Pills -->
    <div class="flex items-center space-x-1.5 overflow-x-auto scrollbar-none">
      <button
        onclick={() => (selectedTatamiId = "all")}
        class="px-3.5 py-1.5 rounded-xl text-xs font-semibold transition border flex items-center space-x-1.5 {selectedTatamiId === 'all' ? 'bg-white text-black border-white shadow-sm' : 'bg-white/[0.04] text-zinc-300 border-white/[0.08] hover:bg-white/[0.08]'}"
      >
        <span>Semua Tatami</span>
        <span class="text-[10px] font-mono opacity-60">({data?.total_categories || 0})</span>
      </button>

      {#if data}
        {#each data.tatamis as t}
          {@const sec = data.sections.find((s) => s.tatami_id === t.id)}
          <button
            onclick={() => (selectedTatamiId = t.id)}
            class="px-3 py-1.5 rounded-xl text-xs font-semibold transition border flex items-center space-x-1.5 {selectedTatamiId === t.id ? 'bg-white text-black border-white shadow-sm' : 'bg-white/[0.04] text-zinc-300 border-white/[0.08] hover:bg-white/[0.08]'}"
          >
            <span>Tatami {t.tatami_number}</span>
            <span class="text-[10px] font-mono opacity-60">({sec?.categories.length || 0})</span>
          </button>
        {/each}
      {/if}
    </div>

    <!-- Quick Filter & Search -->
    <div class="flex items-center space-x-2">
      <!-- Discipline Selector -->
      <select
        bind:value={selectedDiscipline}
        class="bg-[#181a20] text-xs text-zinc-300 px-3 py-1.5 rounded-xl border border-white/[0.08] focus:border-white/30 outline-none"
      >
        <option value="all">Semua Disiplin</option>
        <option value="kata">KATA Saja</option>
        <option value="kumite">KUMITE Saja</option>
      </select>

      <!-- Search Input -->
      <div class="relative w-56">
        <svg class="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari kelas / atlet / kontingen..."
          class="w-full bg-[#181a20] text-xs text-white pl-8 pr-7 py-1.5 rounded-xl border border-white/[0.1] focus:border-white/30 outline-none"
        />
        {#if searchQuery}
          <button
            onclick={() => (searchQuery = "")}
            class="absolute right-2 top-2 text-zinc-400 hover:text-white"
            title="Bersihkan pencarian"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
          </button>
        {/if}
      </div>
    </div>
  </div>

  <!-- Main Scrollable Content Area -->
  <div class="flex-1 overflow-y-auto min-h-0 space-y-4 pr-1 scrollbar-thin">
    {#if isLoading}
      <div class="flex items-center justify-center py-24 text-zinc-500">
        <div class="w-5 h-5 border-2 border-white/20 border-t-white rounded-full animate-spin mr-3"></div>
        <span class="text-xs font-mono">Memuat roster festival turnamen...</span>
      </div>
    {:else if !data || data.total_categories === 0}
      <div class="flex flex-col items-center justify-center py-24 text-zinc-500 border border-dashed border-white/10 rounded-2xl bg-[#181a20]/40">
        <svg class="w-10 h-10 text-zinc-600 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
        </svg>
        <span class="text-sm font-semibold text-zinc-400">Tidak Ada Kategori Festival</span>
        <p class="text-xs text-zinc-500 mt-1 max-w-sm text-center">
          Event ini belum memiliki nomor tanding berlabel festival. Pastikan nama nomor tanding mengandung kata "FESTIVAL".
        </p>
      </div>
    {:else}
      <!-- Sections per Tatami -->
      {#each filteredSections as sec, sIdx}
        <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-4 shadow-lg space-y-3">
          <!-- Tatami Section Header -->
          <div class="flex items-center justify-between pb-2 border-b border-white/[0.06]">
            <div class="flex items-center space-x-2.5">
              <span class="px-2.5 py-1 rounded-lg bg-white/[0.08] text-white font-black font-mono text-xs uppercase tracking-wide border border-white/10">
                TATAMI {sec.tatami_number}
              </span>
              <span class="text-xs font-mono text-zinc-400">
                {sec.categories.length} Kategori • {sec.total_athletes} Atlet • {sec.total_matches} Partai
              </span>
            </div>

            <div class="flex items-center space-x-1.5">
              <button
                onclick={() => {
                  printSelectedTatamiId = sec.tatami_id;
                  printModalOpen = true;
                }}
                class="px-2.5 py-1 bg-white/[0.04] hover:bg-white/[0.08] text-sky-300 text-xs font-semibold rounded-lg border border-sky-500/20 transition flex items-center space-x-1"
                title="Cetak Roster Meja Tatami {sec.tatami_number}"
              >
                <svg class="w-3 h-3 text-sky-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
                </svg>
                <span>Roster T{sec.tatami_number}</span>
              </button>
            </div>
          </div>

          <!-- Category Cards Container -->
          <div class="space-y-3">
            {#each sec.categories as cat, cIdx}
              <div class="bg-[#14161b] border border-white/[0.08] hover:border-white/20 transition rounded-xl p-3 shadow-sm">
                <!-- Category Top Bar -->
                <div class="flex items-center justify-between pb-2 border-b border-white/[0.04] mb-2 gap-2">
                  <div class="flex items-center space-x-2 min-w-0">
                    <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold uppercase shrink-0 {cat.discipline === 'kata' ? 'bg-purple-500/20 text-purple-300' : 'bg-sky-500/20 text-sky-300'}">
                      {cat.discipline}
                    </span>
                    <h4 class="text-xs font-bold text-white truncate" title={cat.nama}>
                      {cat.display_name}
                    </h4>
                    {#if cat.mixed_from.length > 0}
                      <span
                        class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold uppercase shrink-0 bg-amber-500/15 text-amber-300 border border-amber-500/30"
                        title={cat.mixed_from.map((m) => `${m.nama} (${m.atlet_count})`).join("\n")}
                      >
                        MIX {cat.mixed_from.length} KELAS
                      </span>
                    {/if}
                    <span class="text-[10px] font-mono text-zinc-400 shrink-0">
                      ({cat.atlet_count} Atlet • {cat.match_count} Match)
                    </span>
                  </div>

                  <!-- Category Management Buttons -->
                  <div class="flex items-center space-x-1.5 shrink-0">
                    <!-- Move Tatami Selector -->
                    <div class="flex items-center space-x-1 text-[10px] font-mono text-zinc-400">
                      <span>Pindah:</span>
                      <select
                        value={cat.tatami_id}
                        onchange={(e) => moveCategoryTatami(cat.id, Number((e.target as HTMLSelectElement).value))}
                        class="bg-[#181a20] text-zinc-300 text-[10px] px-2 py-0.5 rounded border border-white/10 outline-none"
                      >
                        {#each data.tatamis as t}
                          <option value={t.id}>Tatami {t.tatami_number}</option>
                        {/each}
                      </select>
                    </div>

                    <!-- Move Order Buttons -->
                    <button
                      onclick={() => moveCategoryOrder(sIdx, cIdx, "up")}
                      disabled={cIdx === 0}
                      class="p-1 text-zinc-400 hover:text-white disabled:opacity-20 transition"
                      title="Pindahkan urutan naik"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"/></svg>
                    </button>
                    <button
                      onclick={() => moveCategoryOrder(sIdx, cIdx, "down")}
                      disabled={cIdx === sec.categories.length - 1}
                      class="p-1 text-zinc-400 hover:text-white disabled:opacity-20 transition"
                      title="Pindahkan urutan turun"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                    </button>

                    {#if cat.mixed_from.length > 0}
                      <button
                        onclick={() => releaseMix(cat)}
                        class="px-2 py-0.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.1] text-[10px] font-semibold rounded transition cursor-pointer"
                        title="Kembalikan kelas-kelas ini menjadi kategori terpisah"
                      >
                        Lepas Mix
                      </button>
                    {:else if cat.atlet_count <= 2}
                      <button
                        onclick={() => openSmartMixForCategory(cat)}
                        class="px-2 py-0.5 bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 border border-amber-500/30 text-[10px] font-bold rounded flex items-center space-x-1 transition active:scale-95 cursor-pointer"
                        title="Buka rekomendasi cerdas untuk menggabungkan kelas sepi ini"
                      >
                        <span>⚡ Mix</span>
                      </button>
                    {/if}

                    <!-- Auto-Pair Single -->
                    <button
                      onclick={() => handleAutoPairSingle(cat.id)}
                      class="px-2 py-0.5 rounded bg-white/[0.04] hover:bg-white/[0.08] text-amber-300 text-[10px] font-semibold border border-amber-500/20 transition flex items-center space-x-1"
                      title="Auto-pair anti-bentrok kategori ini saja"
                    >
                      <svg class="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      <span>Pairing</span>
                    </button>
                  </div>
                </div>

                <!-- Match Pairs Table -->
                <div class="overflow-x-auto rounded-lg border border-white/[0.06]">
                  <table class="w-full text-left border-collapse text-xs">
                    <thead class="bg-[#111215] text-zinc-400 font-mono text-[10px] uppercase tracking-wider border-b border-white/[0.06]">
                      <tr>
                        <th class="py-1.5 px-3 w-16 text-center">Partai</th>
                        <th class="py-1.5 px-3 w-5/12">Sudut Merah (AKA)</th>
                        <th class="py-1.5 px-3 w-5/12 border-l border-white/[0.04]">Sudut Biru (AO)</th>
                        <th class="py-1.5 px-3 w-28 text-center border-l border-white/[0.04]">Status</th>
                        <th class="py-1.5 px-2 w-20 text-center border-l border-white/[0.04]">Aksi</th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-white/[0.04]">
                      {#each cat.matches as m}
                        <tr class="hover:bg-white/[0.02] transition {m.is_clash ? 'bg-rose-500/[0.04]' : ''}">
                          <!-- Match Number -->
                          <td class="py-2 px-3 text-center font-bold font-mono text-zinc-400">
                            #{m.match_num}
                          </td>

                          <!-- Sudut Merah (AKA) -->
                          <td class="py-2 px-3">
                            <div class="flex items-start space-x-2">
                              <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-rose-500/20 text-rose-300 border border-rose-500/30 mt-0.5 shrink-0">
                                AKA
                              </span>
                              <div class="min-w-0">
                                <div class="font-bold text-white text-xs truncate">{m.aka.nama}</div>
                                <div class="text-[10px] text-zinc-400 truncate flex items-center space-x-1.5 mt-0.5 font-mono">
                                  <span>{m.aka.perguruan}</span>
                                  <span>•</span>
                                  <span class="text-zinc-300 font-semibold">{m.aka.utusan}</span>
                                </div>
                              </div>
                            </div>
                          </td>

                          <!-- Sudut Biru (AO) -->
                          <td class="py-2 px-3 border-l border-white/[0.04]">
                            {#if m.ao}
                              <div class="flex items-start space-x-2">
                                <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-sky-500/20 text-sky-300 border border-sky-500/30 mt-0.5 shrink-0">
                                  AO
                                </span>
                                <div class="min-w-0">
                                  <div class="font-bold text-white text-xs truncate">{m.ao.nama}</div>
                                  <div class="text-[10px] text-zinc-400 truncate flex items-center space-x-1.5 mt-0.5 font-mono">
                                    <span>{m.ao.perguruan}</span>
                                    <span>•</span>
                                    <span class="text-zinc-300 font-semibold">{m.ao.utusan}</span>
                                  </div>
                                </div>
                              </div>
                            {:else}
                              <div class="text-[11px] font-mono text-zinc-500 italic">
                                TANPA LAWAN
                              </div>
                            {/if}
                          </td>

                          <!-- Status Clash Indicator -->
                          <td class="py-2 px-3 text-center border-l border-white/[0.04]">
                            {#if m.is_clash}
                              <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[9px] font-mono font-bold bg-rose-500/20 text-rose-300 border border-rose-500/40" title="Kedua atlet berasal dari kontingen yang sama">
                                <svg class="w-3 h-3 text-rose-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
                                </svg>
                                <span>1 DOJO</span>
                              </span>
                            {:else if m.ao}
                              <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[9px] font-mono font-bold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                                <svg class="w-3 h-3 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7"/>
                                </svg>
                                <span>SIAP</span>
                              </span>
                            {:else}
                              <span class="px-2 py-0.5 rounded text-[9px] font-mono font-bold bg-white/[0.06] text-zinc-400 border border-white/10">
                                SOLO
                              </span>
                            {/if}
                          </td>

                          <!-- Aksi (Swap M/B & Launch CP) -->
                          <td class="py-2 px-2 text-center border-l border-white/[0.04]">
                            <div class="flex items-center justify-center space-x-1">
                              {#if m.ao}
                                <button
                                  onclick={() => swapMatchCorners(cat.id, m.match_num)}
                                  class="p-1 rounded text-zinc-400 hover:text-white hover:bg-white/[0.08] transition"
                                  title="Tukar sudut Merah ⇄ Biru"
                                >
                                  <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
                                  </svg>
                                </button>
                              {/if}

                              <button
                                onclick={() => launchControlPanelForMatch(m, sec.tatami_number, cat.display_name, cat.discipline)}
                                class="p-1 rounded text-amber-400 hover:text-white hover:bg-amber-500/20 transition"
                                title="Buka pertandingan di Control Panel"
                              >
                                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                </svg>
                              </button>
                            </div>
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>

<!-- ========================================================================= -->
<!-- MODAL: MIX / GABUNG KELAS FESTIVAL                                       -->
<!-- ========================================================================= -->
{#if mixModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100 no-print">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-xl w-full flex flex-col max-h-[85vh]">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08] shrink-0">
        <div class="flex items-center space-x-2">
          <svg class="w-4 h-4 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          <h3 class="text-base font-black text-white uppercase tracking-tight">Mix / Gabung Kategori Festival</h3>
        </div>
        <button onclick={() => (mixModalOpen = false)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="py-4 space-y-4 overflow-y-auto pr-1 scrollbar-thin">
        <p class="text-xs text-zinc-400">
          Pilih 2 atau lebih kategori festival yang ingin digabung. Seluruh atlet akan dipindahkan ke kategori baru dan sistem otomatis melakukan anti-bentrok kontingen.
        </p>

        <!-- 1. Source Categories Multi-Select -->
        <div>
          <span class="block text-[11px] font-mono text-zinc-400 uppercase mb-1.5 font-semibold">
            1. Pilih Kategori Asal yang Ingin Digabung:
          </span>
          <div class="max-h-48 overflow-y-auto rounded-xl border border-white/[0.08] bg-[#14161b] p-2 space-y-1 scrollbar-thin">
            {#each allCategoriesList as cat}
              <label class="flex items-center justify-between p-2 rounded-lg hover:bg-white/[0.04] transition cursor-pointer text-xs {mixSourceIds.includes(cat.id) ? 'bg-white/[0.06]' : ''}">
                <div class="flex items-center space-x-2 min-w-0">
                  <input
                    type="checkbox"
                    checked={mixSourceIds.includes(cat.id)}
                    onchange={() => toggleMixSource(cat.id)}
                    class="rounded accent-amber-500"
                  />
                  <span class="truncate text-white font-medium">{cat.display_name}</span>
                </div>
                <span class="text-[10px] font-mono text-zinc-400 shrink-0 ml-2">
                  {cat.atlet_count} Atlet
                </span>
              </label>
            {/each}
          </div>
          <span class="text-[10px] font-mono text-amber-400 mt-1 block">
            {mixSourceIds.length} kategori dipilih
          </span>
        </div>

        <!-- 2. New Category Name -->
        <div>
          <label for="mix-new-name-input" class="block text-[11px] font-mono text-zinc-400 uppercase mb-1">
            2. Nama Kategori Gabungan Baru:
          </label>
          <div class="flex items-center rounded-xl border border-white/10 bg-[#14161b] overflow-hidden focus-within:border-white/30">
            <span class="px-3 py-2 text-xs font-mono font-bold bg-white/[0.06] text-amber-300 border-r border-white/10 shrink-0">
              FESTIVAL
            </span>
            <input
              id="mix-new-name-input"
              type="text"
              bind:value={mixNewName}
              placeholder="PRA USIA DINI (MIX USIA DINI) - KATA PUTRI"
              class="w-full bg-transparent text-white text-xs px-3 py-2 outline-none uppercase"
            />
          </div>
        </div>

        <!-- 3. Delete Empty Sources Option -->
        <label class="flex items-center space-x-2 cursor-pointer p-3 rounded-xl bg-[#14161b] border border-white/[0.06]">
          <input type="checkbox" bind:checked={mixDeleteSources} class="rounded accent-amber-500" />
          <span class="text-xs font-semibold text-white">Hapus kategori asal yang kosong setelah penggabungan</span>
        </label>
      </div>

      <div class="flex items-center justify-end space-x-2 pt-3 border-t border-white/[0.08] shrink-0">
        <button
          onclick={() => (mixModalOpen = false)}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 hover:text-white text-xs font-semibold rounded-xl border border-white/10"
        >
          Batal
        </button>
        <button
          onclick={executeMixCategories}
          disabled={isMixing || mixSourceIds.length === 0}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition disabled:opacity-50"
        >
          {isMixing ? 'Menggabungkan...' : 'Gabungkan Sekarang'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ========================================================================= -->
<!-- MODAL: PRATINJAU DOKUMEN SIAP CETAK (ROSTER FESTIVAL PRINT)              -->
<!-- ========================================================================= -->
{#if printModalOpen && data}
  {@const printSections = data.sections.filter(
    (s) => (printSelectedTatamiId === "all" || s.tatami_id === printSelectedTatamiId) && s.categories.length > 0
  )}
  <div class="fixed inset-0 z-50 bg-[#111215] flex flex-col overflow-y-auto p-6 select-text text-black table-roster-modal-wrapper">
    <!-- Screen-only toolbar -->
    <div class="no-print mb-4 pb-3 border-b border-white/10 flex items-center justify-between text-zinc-200 w-full max-w-[210mm] mx-auto">
      <div class="flex items-center space-x-3">
        <h3 class="text-sm font-black text-white uppercase tracking-tight">Cetak Roster Festival</h3>
        <select
          bind:value={printSelectedTatamiId}
          class="bg-[#181a20] text-xs text-white px-2.5 py-1.5 rounded-xl border border-white/[0.1] outline-none"
        >
          <option value="all">Semua Tatami</option>
          {#each data.tatamis as t}
            <option value={t.id}>Tatami {t.tatami_number}</option>
          {/each}
        </select>
      </div>
      <div class="flex items-center space-x-2">
        <button
          onclick={() => (printModalOpen = false)}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
        >
          Tutup
        </button>
        <button
          onclick={triggerPrint}
          disabled={printSections.length === 0}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-50"
        >
          Cetak
        </button>
      </div>
    </div>

    {#if printSections.length === 0}
      <div class="no-print text-center text-xs text-zinc-500 py-16">Tidak ada kategori festival di tatami ini.</div>
    {/if}

    <div class="roster-paper-stack">
      {#each printSections as sec (sec.tatami_id)}
        {@const rows = buildPrintRows(sec)}
        <div class="roster-sheet">
          <!-- div, not <header>: app.css hides every <header> in print -->
          <div class="roster-kop">
            <div class="roster-kop-logos">
              <img src={FORKI_LOGO_DATA_URL} alt="FORKI" class="roster-logo-forki" />
              <span class="roster-logo-divider"></span>
              <img src={AMBRILINDO_LOGO_DATA_URL} alt="Ambrilindo" class="roster-logo-ambrilindo" />
            </div>
            <div class="roster-kop-text">
              <div class="roster-kop-org">{printOrgName}</div>
              {#if printKopMeta}
                <div class="roster-kop-meta">{printKopMeta}</div>
              {/if}
              <div class="roster-kop-doc">
                ROSTER FESTIVAL{#if printEventLine}<span class="roster-kop-event">  •  {printEventLine}</span>{/if}
              </div>
            </div>
          </div>

          <div class="roster-tatami-banner">
            <span class="roster-tatami-title">TATAMI {sec.tatami_number}</span>
            <span class="roster-tatami-meta">{sec.categories.length} KATEGORI  •  {sec.total_athletes} ATLET</span>
          </div>

          <table class="roster-table">
            <colgroup>
              <col class="col-no" />
              <col class="col-nama" />
              <col class="col-perguruan" />
              <col class="col-utusan" />
              <col class="col-sabuk" />
              <col class="col-check" />
            </colgroup>
            <thead>
              <tr>
                <th>No</th>
                <th class="text-left">Nama</th>
                <th class="text-left">Perguruan</th>
                <th class="text-left">Utusan</th>
                <th>Sabuk</th>
                <th>Checklist</th>
              </tr>
            </thead>
            <tbody>
              {#each rows as r (r.key)}
                {#if r.kind === "category"}
                  <tr class="roster-cat-row">
                    <td colspan="6">{r.name}</td>
                  </tr>
                {:else}
                  <tr class={r.sabuk === "M" ? "roster-row-m" : "roster-row-b"}>
                    <td class="cell-center cell-bold">{r.no}</td>
                    <td class="cell-truncate cell-bold" title={r.nama}>{r.nama}</td>
                    <td class="cell-truncate" title={r.perguruan}>{r.perguruan}</td>
                    <td class="cell-truncate" title={r.utusan}>{r.utusan}</td>
                    <td class="cell-center cell-bold">
                      <span class="sabuk-pill {r.sabuk === 'M' ? 'sabuk-m' : 'sabuk-b'}">{r.sabuk}</span>
                    </td>
                    <td class="cell-center"><span class="check-box"></span></td>
                  </tr>
                {/if}
              {/each}
            </tbody>
          </table>
        </div>
      {/each}
    </div>
  </div>
{/if}

<!-- ========================================================================= -->
<!-- MODAL: ASISTEN REKOMENDASI MIX CERDAS FESTIVAL                           -->
<!-- ========================================================================= -->
{#if autoMixModalOpen && data}
  <FestivalAutoMixModal
    {eventId}
    categories={rawCategoriesList}
    existingMixGroups={mixGroups}
    onClose={() => (autoMixModalOpen = false)}
    onConfirmMix={async (newMixList) => {
      mixGroups = newMixList;
      try {
        await invoke("save_festival_mix_groups", { eventId, mixGroups });
      } catch (e) {
        console.error("Gagal menyimpan auto mix:", e);
      }
      await loadFestivalData();
    }}
  />
{/if}

{#if smartMixModalOpen && smartMixSourceCategory && data}
  <FestivalSmartMixModal
    {eventId}
    sourceCategory={{
      id: smartMixSourceCategory.id,
      nama: smartMixSourceCategory.nama,
      athlete_count: smartMixSourceCategory.atlet_count,
    }}
    allCategories={rawCategoriesList}
    onClose={() => {
      smartMixModalOpen = false;
      smartMixSourceCategory = null;
    }}
    onMixed={async () => {
      await loadFestivalData();
    }}
    {onShowToast}
  />
{/if}

<style>
  /* ===================== ROSTER FESTIVAL PRINT (A4 portrait) ===================== */
  .roster-paper-stack {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }
  .roster-sheet {
    width: 210mm;
    min-height: 297mm;
    padding: 8mm 10mm;
    box-sizing: border-box;
    background: #ffffff;
    color: #000000;
    font-family: Arial, Helvetica, sans-serif;
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.4);
    -webkit-print-color-adjust: exact;
    print-color-adjust: exact;
  }

  .roster-kop {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 52px;
    padding-bottom: 6px;
    border-bottom: 2px solid #0f172a;
  }
  .roster-kop-logos {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .roster-logo-forki {
    height: 40px;
    width: auto;
    max-width: 44px;
    object-fit: contain;
  }
  .roster-logo-divider {
    width: 1.2px;
    height: 28px;
    background: #cbd5e1;
  }
  .roster-logo-ambrilindo {
    height: 22px;
    width: auto;
    max-width: 110px;
    object-fit: contain;
  }
  .roster-kop-text {
    width: 100%;
    padding: 0 170px;
    box-sizing: border-box;
    text-align: center;
  }
  .roster-kop-org {
    font-size: 14px;
    font-weight: 900;
    line-height: 1.15;
    text-transform: uppercase;
    color: #020617;
  }
  .roster-kop-meta {
    margin-top: 2px;
    font-size: 8.5px;
    color: #334155;
    line-height: 1.25;
  }
  .roster-kop-doc {
    margin-top: 4px;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.12em;
    color: #1e293b;
    white-space: pre;
  }
  .roster-kop-event {
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .roster-tatami-banner {
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    margin-top: 6px;
    padding: 4px 8px;
    background: #e4dac4;
    border: 1.5px solid #000000;
    border-radius: 3px 3px 0 0;
  }
  .roster-tatami-title {
    font-size: 13.5px;
    font-weight: 900;
    letter-spacing: 0.1em;
  }
  .roster-tatami-meta {
    position: absolute;
    right: 8px;
    font-size: 8.5px;
    font-weight: 700;
    color: #3f3a2e;
    white-space: pre;
  }

  .roster-table {
    width: 100%;
    table-layout: fixed;
    border-collapse: collapse;
    border: 1.5px solid #000000;
    border-top: none;
    font-size: 9.5px;
  }
  .roster-table .col-no { width: 32px; }
  .roster-table .col-nama { width: 35%; }
  .roster-table .col-perguruan { width: 20%; }
  .roster-table .col-utusan { width: 30%; }
  .roster-table .col-sabuk { width: 46px; }
  .roster-table .col-check { width: 70px; }
  .roster-table thead {
    display: table-header-group;
  }
  .roster-table th {
    background: #e4dac4;
    border: 1px solid #000000;
    padding: 3px 4px;
    font-size: 9px;
    font-weight: 800;
    text-transform: uppercase;
    text-align: center;
  }
  .roster-table th.text-left {
    text-align: left;
  }
  .roster-table td {
    border: 1px solid #000000;
    padding: 2.2px 4px;
    height: 18px;
    line-height: 1.15;
    vertical-align: middle;
  }
  .roster-table tr {
    break-inside: avoid;
    page-break-inside: avoid;
  }
  .roster-cat-row td {
    background: #efe8d8;
    font-size: 10.5px;
    font-weight: 800;
    text-align: center;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .roster-cat-row {
    break-after: avoid;
    page-break-after: avoid;
  }
  .cell-center { text-align: center; }
  .cell-bold { font-weight: 700; }
  .cell-truncate {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sabuk-pill {
    display: inline-block;
    min-width: 16px;
    padding: 0 3px;
    border-radius: 2px;
    font-size: 9px;
    font-weight: 900;
    line-height: 13px;
  }
  .sabuk-m { background: #fde2e2; color: #991b1b; }
  .sabuk-b { background: #dbeafe; color: #1e3a8a; }
  .check-box {
    display: inline-block;
    width: 10px;
    height: 10px;
    border: 1px solid #000000;
    border-radius: 1.5px;
    background: #ffffff;
    vertical-align: middle;
  }

  @media print {
    :global(body) {
      background: #ffffff !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }
    .festival-editor-main,
    .no-print {
      display: none !important;
    }
    /* Un-fix the preview wrapper so the sheets paginate naturally */
    .table-roster-modal-wrapper {
      position: static !important;
      inset: auto !important;
      z-index: auto !important;
      background: transparent !important;
      padding: 0 !important;
      margin: 0 !important;
      width: 100% !important;
      height: auto !important;
      max-height: none !important;
      overflow: visible !important;
      display: block !important;
    }
    .roster-paper-stack {
      display: block;
      gap: 0;
    }
    .roster-sheet {
      width: 100%;
      min-height: 0;
      padding: 0;
      box-shadow: none;
      break-after: page;
      page-break-after: always;
    }
    .roster-sheet:last-child {
      break-after: auto;
      page-break-after: auto;
    }
  }
</style>
