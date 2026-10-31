<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    FestivalRosterData,
    FestivalRosterState,
    FestivalTatamiSection,
    FestivalCategory,
    FestivalMatch,
    TatamiItem,
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

  // --- Print View State ---
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
      const res: FestivalRosterData = await invoke("get_festival_roster_data", { eventId });
      data = res;
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

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
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

      <!-- Mix / Gabung Kelas -->
      <button
        onclick={openMixModal}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-all flex items-center space-x-1.5"
        title="Gabungkan kategori festival yang sepi / solo"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
        <span>Mix / Gabung Kelas</span>
      </button>

      <!-- Cetak Roster -->
      <button
        onclick={() => (printModalOpen = true)}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-sky-300 hover:text-white font-semibold text-xs rounded-xl border border-sky-500/20 transition-all flex items-center space-x-1.5"
        title="Buka Pratinjau Dokumen Siap Cetak Roster Festival"
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

            <button
              onclick={() => {
                printSelectedTatamiId = sec.tatami_id;
                printModalOpen = true;
              }}
              class="px-2.5 py-1 bg-white/[0.04] hover:bg-white/[0.08] text-sky-300 text-xs font-semibold rounded-lg border border-sky-500/20 transition flex items-center space-x-1"
            >
              <svg class="w-3 h-3 text-sky-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
              </svg>
              <span>Cetak Tatami {sec.tatami_number}</span>
            </button>
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
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100">
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
  <div class="fixed inset-0 z-50 bg-[#111215] flex flex-col overflow-y-auto p-6 select-text text-black">
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
    }
    .no-print {
      display: none !important;
    }
    .print-page-box {
      box-shadow: none !important;
      max-width: 100% !important;
      padding: 0 !important;
      border: none !important;
    }
  }
</style>
