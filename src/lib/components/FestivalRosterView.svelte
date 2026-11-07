<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import { printWithOrientation } from "$lib/print_helper";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL, BUSHIDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import FestivalSmartMixModal from "$lib/components/FestivalSmartMixModal.svelte";
  import FestivalAutoMixModal from "$lib/components/FestivalAutoMixModal.svelte";
  import FestivalBracketSheet from "$lib/components/FestivalBracketSheet.svelte";
  import type { FestivalMixGroupData } from "$lib/festival_mix_helper";
  import type {
    FestivalRosterData,
    FestivalRosterState,
    FestivalTatamiSection,
    FestivalCategory,
    FestivalMatch,
    TatamiItem,
    FestivalBookletData,
    FestivalBookletCategory,
    DetailBaganItem,
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

  // --- Bagan Booklet Print Modal State (Sampul Resmi + Bagan Festival) ---
  let baganBookletModalOpen = $state(false);
  let baganBookletLoading = $state(false);
  let baganBookletData = $state<FestivalBookletData | null>(null);
  let baganBookletSelectedTatamiId = $state<number | "all">("all");
  let sampulDayLabel = $state("DAY 1");
  let sampulVenue = $state("FESTIVAL BUSHIDO I");
  let sampulDate = $state("8 OKTOBER 2026");
  let sampulOrg = $state("FEDERASI OLAHRAGA KARATE-DO INDONESIA PROVINSI KALIMANTAN TIMUR");

  async function openBaganBooklet(tatamiId: number | "all" = "all") {
    baganBookletSelectedTatamiId = tatamiId;
    baganBookletLoading = true;
    baganBookletModalOpen = true;
    try {
      const res: FestivalBookletData = await invoke("get_festival_booklet_data", {
        eventId,
        tatamiId: tatamiId === "all" ? null : tatamiId,
      });
      baganBookletData = res;
      sampulVenue = "FESTIVAL BUSHIDO I";
      sampulDate = "8 OKTOBER 2026";
      if (res.kop_surat?.nama_organisasi) sampulOrg = res.kop_surat.nama_organisasi;
    } catch (e: any) {
      console.error("Gagal memuat booklet bagan festival:", e);
      onShowToast(`Gagal memuat bagan festival: ${e}`, "error");
    } finally {
      baganBookletLoading = false;
    }
  }

  async function handleFilterBookletTatami(newTatamiId: number | "all") {
    baganBookletSelectedTatamiId = newTatamiId;
    await openBaganBooklet(newTatamiId);
  }

  let coverTatamiTitle = $derived.by(() => {
    if (baganBookletSelectedTatamiId === "all" || !baganBookletData) {
      return "BAGAN FESTIVAL";
    }
    const t = baganBookletData.tatamis.find((x) => x.id === baganBookletSelectedTatamiId);
    const num = t?.tatami_number || 1;
    return `BAGAN FESTIVAL - TATAMI ${num}`;
  });



  function triggerBookletPrint() {
    printWithOrientation("landscape", "5mm");
  }

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
    const sparseCat = allCategoriesList.find((c) => c.atlet_count <= 2);
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
    window.print();
  }
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative festival-editor-main {baganBookletModalOpen || printModalOpen ? 'hidden print:hidden' : 'print:hidden'}">
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

      <!-- Cetak Bagan Festival (Booklet Lengkap Sampul) -->
      <button
        onclick={() => openBaganBooklet("all")}
        class="h-9 px-3.5 bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-400 hover:to-teal-500 text-black font-black text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5"
        title="Buka Dokumen Siap Cetak Seluruh Bagan Festival Lengkap Sampul Resmi Landscape"
      >
        <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
          <path d="M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z"/>
        </svg>
        <span>Cetak Bagan Festival</span>
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
                onclick={() => openBaganBooklet(sec.tatami_id)}
                class="px-2.5 py-1 bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-300 text-xs font-semibold rounded-lg border border-emerald-500/30 transition flex items-center space-x-1"
                title="Cetak Seluruh Bagan Tatami {sec.tatami_number} Lengkap Sampul Resmi"
              >
                <svg class="w-3 h-3 text-emerald-400 fill-current" viewBox="0 0 24 24">
                  <path d="M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z"/>
                </svg>
                <span>Cetak Bagan T{sec.tatami_number}</span>
              </button>

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

                    {#if cat.atlet_count <= 2}
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
                                (BYE / TUNGGAL - PERAIH EMAS LANGSUNG)
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
    (s) => printSelectedTatamiId === "all" || s.tatami_id === printSelectedTatamiId
  )}
  <div class="fixed inset-0 z-50 bg-[#111215] flex flex-col overflow-y-auto p-6 select-text text-black table-roster-modal-wrapper">
    <!-- Screen-Only Toolbar -->
    <div class="no-print mb-4 pb-3 border-b border-white/10 flex items-center justify-between text-zinc-200 max-w-5xl mx-auto w-full">
      <div class="flex items-center space-x-3">
        <h3 class="text-sm font-bold text-white uppercase">Pratinjau Dokumen Siap Cetak Roster Festival</h3>
        <select
          bind:value={printSelectedTatamiId}
          class="bg-[#181a20] text-xs text-white px-2.5 py-1 rounded-lg border border-white/10 outline-none"
        >
          <option value="all">Semua Tatami</option>
          {#each data.tatamis as t}
            <option value={t.id}>Tatami {t.tatami_number} Saja</option>
          {/each}
        </select>
      </div>
      <div class="flex items-center space-x-2">
        <button
          onclick={triggerPrint}
          class="h-9 px-4 bg-sky-500 hover:bg-sky-600 text-white font-bold text-xs rounded-xl shadow-md flex items-center space-x-1.5 transition"
        >
          <svg class="w-3.5 h-3.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
          </svg>
          <span>Cetak Sekarang (Print)</span>
        </button>
        <button
          onclick={() => (printModalOpen = false)}
          class="h-9 px-3.5 bg-white/[0.08] hover:bg-white/[0.15] text-zinc-300 font-semibold text-xs rounded-xl transition"
        >
          Tutup
        </button>
      </div>
    </div>

    <!-- Printable Paper Page (A4 Ink-Friendly) -->
    <div class="bg-white p-6 max-w-5xl mx-auto shadow-2xl rounded-lg text-black font-sans w-full print-page-box">
      <!-- Kop Surat -->
      <div class="simple-roster-header">
        <div class="simple-roster-kop-left">
          <div class="simple-roster-org-info">
            <h3 class="simple-roster-org-name">{data.kop_surat?.nama_organisasi || data.event_name}</h3>
            {#if data.kop_surat?.alamat || data.kop_surat?.kontak}
              <p class="simple-roster-org-meta">{data.kop_surat.alamat} {data.kop_surat.kontak ? `• ${data.kop_surat.kontak}` : ''}</p>
            {/if}
            <div class="simple-roster-doc-title">ROSTER PERTANDINGAN KATEGORI FESTIVAL</div>
          </div>
        </div>
        <div class="simple-roster-day-badge-box">
          <div class="simple-roster-day-label">FESTIVAL</div>
          <div class="simple-roster-day-date">{data.total_matches} Partai</div>
        </div>
      </div>

      <!-- Sections & Tables -->
      {#each printSections as sec}
        <div class="mb-6 break-inside-avoid">
          <div class="simple-roster-day-bar">
            <span class="day-bar-title">TATAMI {sec.tatami_number}</span>
            <span class="day-bar-date">{sec.categories.length} Kategori • {sec.total_athletes} Atlet • {sec.total_matches} Partai</span>
          </div>

          {#each sec.categories as cat}
            <div class="mb-4 break-inside-avoid">
              <div class="bg-zinc-100 border border-zinc-300 px-3 py-1 font-bold text-xs uppercase flex justify-between items-center my-1">
                <span>[{cat.discipline.toUpperCase()}] {cat.display_name}</span>
                <span class="text-[10px] font-mono text-zinc-600 font-semibold">{cat.atlet_count} Atlet • {cat.match_count} Match</span>
              </div>

              <table class="w-full text-left border-collapse border border-zinc-400 text-xs">
                <thead>
                  <tr class="bg-zinc-200 text-zinc-800 text-[10px] font-bold">
                    <th class="border border-zinc-400 p-1.5 w-12 text-center">Partai</th>
                    <th class="border border-zinc-400 p-1.5 w-5/12">Sudut Merah (AKA)</th>
                    <th class="border border-zinc-400 p-1.5 w-5/12">Sudut Biru (AO)</th>
                    <th class="border border-zinc-400 p-1.5 w-16 text-center">Paraf Meja</th>
                  </tr>
                </thead>
                <tbody>
                  {#each cat.matches as m}
                    <tr>
                      <td class="border border-zinc-400 p-1.5 text-center font-bold font-mono">#{m.match_num}</td>
                      <td class="border border-zinc-400 p-1.5">
                        <div class="font-bold text-[11px] leading-tight">[M] {m.aka.nama}</div>
                        <div class="text-[9.5px] text-zinc-600 font-mono mt-0.5">{m.aka.perguruan} • {m.aka.utusan}</div>
                      </td>
                      <td class="border border-zinc-400 p-1.5">
                        {#if m.ao}
                          <div class="font-bold text-[11px] leading-tight">[B] {m.ao.nama}</div>
                          <div class="text-[9.5px] text-zinc-600 font-mono mt-0.5">{m.ao.perguruan} • {m.ao.utusan}</div>
                        {:else}
                          <div class="text-[10px] text-zinc-500 italic">(BYE / TUNGGAL)</div>
                        {/if}
                      </td>
                      <td class="border border-zinc-400 p-1.5 text-center">
                        <div class="border border-zinc-400 w-4 h-4 mx-auto rounded"></div>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/each}
        </div>
      {/each}

      <!-- Signatures Footer -->
      <div class="flex justify-between mt-12 pt-4 text-center text-xs break-inside-avoid">
        <div class="w-56">
          <p class="text-zinc-500 mb-14">Technical Delegate,</p>
          <div class="border-t border-black font-bold pt-1">( ........................................ )</div>
        </div>
        <div class="w-56">
          <p class="text-zinc-500 mb-14">Ketua Panitia Pelaksana,</p>
          <div class="border-t border-black font-bold pt-1">( ........................................ )</div>
        </div>
      </div>

      <div class="simple-roster-footer-line">
        <span>Dicetak: {new Date().toLocaleDateString('id-ID', { day: 'numeric', month: 'short', year: 'numeric' })} • Ambrilindo</span>
      </div>
    </div>
  </div>
{/if}


<!-- ========================================================================= -->
<!-- MODAL: ASISTEN REKOMENDASI MIX CERDAS FESTIVAL                           -->
<!-- ========================================================================= -->
{#if autoMixModalOpen && data}
  <FestivalAutoMixModal
    {eventId}
    categories={data.sections.flatMap((s) => s.categories).map((c) => ({
      id: c.id,
      nama: c.nama,
      athlete_count: c.atlet_count,
    }))}
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
    allCategories={allCategoriesList.map((c) => ({
      id: c.id,
      nama: c.nama,
      athlete_count: c.atlet_count,
    }))}
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

<!-- ========================================================================= -->
<!-- MODAL: PRATINJAU DOKUMEN CETAK BAGAN FESTIVAL (LENGKAP SAMPUL RESMI)     -->
<!-- ========================================================================= -->
{#if baganBookletModalOpen}
  <div class="fixed inset-0 z-50 bg-[#111215] flex flex-col overflow-y-auto p-4 md:p-6 select-text text-black booklet-modal-wrapper">
    <!-- Screen-Only Toolbar -->
    <div class="no-print mb-4 pb-3 border-b border-white/10 flex flex-wrap items-center justify-between text-zinc-200 max-w-6xl mx-auto w-full gap-3">
      <div class="flex items-center space-x-3 flex-wrap gap-2">
        <div class="flex items-center space-x-2">
          <span class="w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse"></span>
          <h3 class="text-sm font-black text-white uppercase tracking-tight">Cetak Bagan Festival (Lengkap Sampul)</h3>
          {#if baganBookletData}
            <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30" title="Bagan membaca langsung dari database SQLite (Single Source of Truth)">
              ⚡ SSoT • {baganBookletData.categories.length} BAGAN
            </span>
          {/if}
        </div>

        <!-- Filter Tatami -->
        <select
          value={baganBookletSelectedTatamiId}
          onchange={(e) => handleFilterBookletTatami(e.currentTarget.value === "all" ? "all" : Number(e.currentTarget.value))}
          class="bg-[#181a20] text-xs text-white px-2.5 py-1 rounded-lg border border-white/20 outline-none font-bold cursor-pointer"
        >
          <option value="all">Semua Tatami</option>
          {#if baganBookletData}
            {#each baganBookletData.tatamis as t}
              <option value={t.id}>Tatami {t.tatami_number} Saja</option>
            {/each}
          {/if}
        </select>
      </div>

      <!-- Editable Cover Fields in Toolbar -->
      <div class="flex items-center space-x-2 flex-wrap gap-2">
        <div class="flex items-center space-x-1 text-[11px] font-mono text-zinc-400">
          <span>Hari:</span>
          <input
            type="text"
            bind:value={sampulDayLabel}
            placeholder="DAY 1"
            class="bg-[#181a20] text-white px-2 py-0.5 rounded border border-white/10 text-xs w-20 outline-none uppercase font-bold"
          />
        </div>

        <div class="flex items-center space-x-1 text-[11px] font-mono text-zinc-400">
          <span>Event:</span>
          <input
            type="text"
            bind:value={sampulVenue}
            placeholder="FESTIVAL BUSHIDO I"
            class="bg-[#181a20] text-white px-2 py-0.5 rounded border border-white/10 text-xs w-52 outline-none uppercase font-bold"
          />
        </div>

        <div class="flex items-center space-x-1 text-[11px] font-mono text-zinc-400">
          <span>Tanggal:</span>
          <input
            type="text"
            bind:value={sampulDate}
            placeholder="8 OKTOBER 2026"
            class="bg-[#181a20] text-white px-2 py-0.5 rounded border border-white/10 text-xs w-36 outline-none uppercase font-bold"
          />
        </div>

        <!-- Print Action -->
        <button
          onclick={triggerBookletPrint}
          disabled={baganBookletLoading || !baganBookletData}
          class="h-9 px-4 bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-400 hover:to-teal-500 text-black font-black text-xs rounded-xl shadow-md flex items-center space-x-1.5 transition active:scale-95 disabled:opacity-50 cursor-pointer"
        >
          <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
            <path d="M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z"/>
          </svg>
          <span>Cetak Sekarang (Print / PDF)</span>
        </button>

        <button
          onclick={() => (baganBookletModalOpen = false)}
          class="h-9 px-3.5 bg-white/[0.08] hover:bg-white/[0.15] text-zinc-300 font-semibold text-xs rounded-xl transition cursor-pointer"
        >
          Tutup
        </button>
      </div>
    </div>

    <!-- Document Preview Content -->
    {#if baganBookletLoading}
      <div class="flex items-center justify-center py-32 text-zinc-400">
        <div class="w-6 h-6 border-2 border-white/30 border-t-white rounded-full animate-spin mr-3"></div>
        <span class="text-sm font-mono">Menyiapkan seluruh bagan festival & sampul resmi...</span>
      </div>
    {:else if baganBookletData}
      {@const targetCategories = baganBookletData.categories}
      <div class="booklet-preview-container max-w-6xl mx-auto w-full space-y-8 select-text">
        <!-- ========================================================================= -->
        <!-- HALAMAN 1: SAMPUL RESMI TURNAMEN (A4 LANDSCAPE)                          -->
        <!-- ========================================================================= -->
        <div class="booklet-page booklet-cover-page">
          <div class="booklet-page-label-bar no-print">
            <span>Halaman 1 • Sampul Resmi Bagan Festival ({coverTatamiTitle})</span>
          </div>
          <div class="sampul-page-container">
            <img src="/img/cover/cover_clean_template.png" class="absolute inset-0 w-full h-full object-fill pointer-events-none select-none z-0" alt="Cover" />
            <!-- Top Ribbon Organization Text -->
            <div class="sampul-org-bar">
              {sampulOrg}
            </div>

            <!-- Event Logo Container -->
            <div class="sampul-logo-container">
              <img
                src={BUSHIDO_LOGO_DATA_URL}
                class="sampul-event-logo"
                alt="Logo Bushido"
              />
            </div>

            <!-- Main Title Group inside Cover -->
            <div class="sampul-main-group">
              <div class="sampul-bagan-label">BAGAN:</div>
              <div class="sampul-tatami-title">{coverTatamiTitle}</div>
              <div class="sampul-event-title">{sampulVenue}</div>
              <div class="sampul-event-dates">{sampulDate}</div>
            </div>

            <!-- Supported by branding -->
            <div class="sampul-supported-by">
              SUPPORTED BY:
            </div>
          </div>
        </div>

        <!-- ========================================================================= -->
        <!-- HALAMAN 2+: BAGAN PER KATEGORI (A4 LANDSCAPE PRESISI 1 LEMBAR PER KELAS) -->
        <!-- ========================================================================= -->
        {#if targetCategories.length === 0}
          <div class="bg-white p-12 text-center text-zinc-600 rounded-2xl shadow-sm border border-slate-200 max-w-lg mx-auto space-y-3">
            <div class="w-12 h-12 rounded-full bg-amber-500/10 border border-amber-500/20 text-amber-500 text-2xl flex items-center justify-center mx-auto">
              🎋
            </div>
            <h4 class="text-sm font-black text-slate-900 uppercase">Belum Ada Bagan Festival di Database</h4>
            <p class="text-xs text-slate-500 leading-relaxed">
              Bagan festival turnamen ini belum dibuat/diundi di database. Silakan buka menu <strong>Drawing Bagan Festival</strong> untuk mengundi bagan terlebih dahulu.
            </p>
          </div>
        {:else}
          {#each targetCategories as cat, cIdx}
            <div class="booklet-page booklet-bracket-page">
              <div class="booklet-page-label-bar no-print">
                <span>Halaman {cIdx + 2} • Bagan {cIdx + 1}/{targetCategories.length}: [{cat.bagan_kode || 'F'}] {cat.category_name} (Tatami {cat.tatami_number})</span>
              </div>

              <div class="bracket-page-container print-sheet">
                <FestivalBracketSheet
                  baganId={cat.bagan_id || undefined}
                  baganKode={cat.bagan_kode}
                  categoryName={cat.category_name}
                  discipline={cat.discipline}
                  poolCount={cat.pool}
                  athletesCount={cat.athletes_count}
                  tatamiNumber={cat.tatami_number}
                  venue={sampulVenue}
                  matches={cat.matches}
                  layoutTemplate={cat.layout_type || "quadrant"}
                  showHeader={true}
                  interactive={false}
                  isDarkMode={false}
                />
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .simple-roster-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1.5px solid #cbd5e1;
    padding-bottom: 4px;
    margin-bottom: 4px;
    gap: 8px;
  }
  .simple-roster-kop-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .simple-roster-org-name {
    margin: 0;
    font-size: 13px;
    font-weight: 800;
    color: #0f172a;
    line-height: 1.15;
    text-transform: uppercase;
  }
  .simple-roster-org-meta {
    margin: 1px 0 0;
    font-size: 8.5px;
    color: #475569;
    line-height: 1.15;
  }
  .simple-roster-doc-title {
    margin-top: 2px;
    font-size: 10px;
    font-weight: 800;
    color: #1e293b;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
  .simple-roster-day-badge-box {
    text-align: right;
    padding: 2px 6px;
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-radius: 4px;
    min-width: 90px;
  }
  .simple-roster-day-label {
    font-size: 11.5px;
    font-weight: 800;
    color: #b91c1c;
    text-transform: uppercase;
  }
  .simple-roster-day-date {
    font-size: 8.5px;
    font-weight: 600;
    color: #475569;
    margin-top: 1px;
  }
  .simple-roster-day-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-left: 3px solid #b91c1c;
    padding: 2px 6px;
    margin: 4px 0 3px 0;
  }
  .day-bar-title {
    text-transform: uppercase;
    letter-spacing: 0.02em;
    font-size: 9px;
    font-weight: 800;
    color: #0f172a;
  }
  .day-bar-date {
    font-size: 8px;
    font-weight: 600;
    color: #475569;
  }
  .simple-roster-footer-line {
    font-size: 7.5px;
    color: #94a3b8;
    text-align: right;
    margin-top: 4px;
  }
  .print-page-box {
    box-shadow: none;
    max-width: 100%;
    padding: 0;
    border: none;
  }

  /* ========================================================================= */
  /* BAGAN BOOKLET PRINT ENGINE STYLES (A4 LANDSCAPE + SAMPUL RESMI)           */
  /* ========================================================================= */
  .booklet-preview-container {
    width: 100%;
    margin: 0 auto;
  }
  .booklet-page {
    position: relative;
    width: 100%;
    background: #ffffff;
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.3);
    border-radius: 4px;
    overflow: hidden;
  }
  .booklet-page-label-bar {
    background: #1e293b;
    color: #94a3b8;
    padding: 4px 12px;
    font-size: 11px;
    font-family: monospace;
    font-weight: bold;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  /* Page 1: Sampul */
  .booklet-cover-page {
    height: 196mm;
    max-height: 196mm;
    min-height: 196mm;
    background: #111827;
    box-sizing: border-box;
  }
  .sampul-page-container {
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
    background: #111827 url("/img/cover/cover_clean_template.png") no-repeat center center;
    background-size: 100% 100%;
    box-sizing: border-box;
  }
  .sampul-org-bar {
    position: absolute;
    top: 3.2%;
    height: 9.8%;
    left: 38%;
    right: 8%;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    text-align: right;
    font-family: "Arial Narrow", "Helvetica Neue Condensed", Arial, sans-serif;
    font-size: 11.5pt;
    font-weight: 800;
    color: #ffffff;
    line-height: 1;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    z-index: 10;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sampul-logo-container {
    position: absolute;
    top: 15.5%;
    right: 3.5%;
    width: 28%;
    height: 30%;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }
  .sampul-event-logo {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.45));
  }
  .sampul-main-group {
    position: absolute;
    top: 66%;
    left: 57.5%;
    width: 39%;
    transform: translateY(-50%);
    z-index: 10;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
  }
  .sampul-bagan-label {
    font-family: 'Roboto', 'Oswald', sans-serif;
    font-size: 26pt;
    font-weight: 900;
    color: #ffffff;
    letter-spacing: 0.04em;
    line-height: 1.1;
    text-transform: uppercase;
    margin: 0;
  }
  .sampul-tatami-title {
    font-family: 'Roboto', 'Oswald', sans-serif;
    font-size: 26pt;
    font-weight: 900;
    color: #ffffff;
    letter-spacing: 0.03em;
    line-height: 1.15;
    text-transform: uppercase;
    margin: 0 0 6px 0;
  }
  .sampul-event-title {
    font-family: 'Inter', Arial, sans-serif;
    font-size: 13.5pt;
    font-weight: 800;
    color: #facc15;
    line-height: 1.25;
    text-transform: uppercase;
    margin: 0;
  }
  .sampul-event-dates {
    font-family: 'Inter', Arial, sans-serif;
    font-size: 11.5pt;
    font-weight: 700;
    color: #facc15;
    line-height: 1.2;
    margin: 0;
  }
  .sampul-supported-by {
    position: absolute;
    top: 84.0%;
    left: 81.0%;
    font-family: 'Calibri', 'Inter', sans-serif;
    font-size: 11pt;
    font-weight: 800;
    color: #ffffff;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    z-index: 10;
  }

  /* Page 2+: Bagan Sheets */
  .booklet-bracket-page {
    height: 196mm;
    max-height: 196mm;
    min-height: 196mm;
    overflow: hidden;
    box-sizing: border-box;
  }
  .print-sheet {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    padding: 2.5mm 3.5mm;
    background: #ffffff;
    color: #000000;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  @media print {
    @page {
      size: landscape;
      margin: 5mm;
    }

    :global(body) {
      background: #ffffff !important;
      color: #000000 !important;
      margin: 0 !important;
      padding: 0 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    /* 1. Strictly hide editor and screen-only controls */
    .festival-editor-main,
    .no-print {
      display: none !important;
    }

    /* 2. De-fix the modal wrappers so they paginate naturally without repeating fixed headers */
    .booklet-modal-wrapper,
    .table-roster-modal-wrapper {
      position: static !important;
      inset: auto !important;
      z-index: auto !important;
      background: transparent !important;
      background-color: transparent !important;
      padding: 0 !important;
      margin: 0 !important;
      width: 100% !important;
      max-width: 100% !important;
      height: auto !important;
      min-height: auto !important;
      max-height: none !important;
      overflow: visible !important;
      display: block !important;
      box-shadow: none !important;
    }

    .print-page-box {
      box-shadow: none !important;
      max-width: 100% !important;
      padding: 0 !important;
      border: none !important;
    }

    /* 3. Booklet preview container reset */
    .booklet-preview-container {
      max-width: 100% !important;
      width: 100% !important;
      margin: 0 !important;
      padding: 0 !important;
      gap: 0 !important;
      display: block !important;
    }
    .booklet-preview-container > * + * {
      margin-top: 0 !important;
    }

    /* 4. Individual Booklet Pages (A4 Landscape) */
    .booklet-page {
      box-shadow: none !important;
      border: none !important;
      border-radius: 0 !important;
      margin: 0 !important;
      padding: 0 !important;
      width: 100% !important;
      height: 196mm !important;
      max-height: 196mm !important;
      min-height: 196mm !important;
      overflow: hidden !important;
      box-sizing: border-box !important;
      page-break-after: always !important;
      break-after: page !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
    }
    .booklet-page:last-child {
      page-break-after: auto !important;
      break-after: auto !important;
    }

    /* 5. Cover Page (Page 1) */
    .booklet-cover-page {
      height: 196mm !important;
      max-height: 196mm !important;
      min-height: 196mm !important;
      page-break-after: always !important;
      break-after: page !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
      overflow: hidden !important;
    }

    /* 6. Bracket Pages (Page 2+) */
    .booklet-bracket-page {
      height: 196mm !important;
      max-height: 196mm !important;
      min-height: 196mm !important;
      page-break-after: always !important;
      break-after: page !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
      overflow: hidden !important;
    }

    .print-sheet {
      box-sizing: border-box !important;
      width: 100% !important;
      height: 100% !important;
      padding: 2.5mm 3.5mm !important;
      background: #ffffff !important;
      color: #000000 !important;
      display: flex !important;
      flex-direction: column !important;
      overflow: hidden !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .sampul-page-container {
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }
  }

</style>
