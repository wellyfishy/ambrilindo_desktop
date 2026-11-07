<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import FestivalSmartMixModal from "$lib/components/FestivalSmartMixModal.svelte";
  import FestivalAutoMixModal from "$lib/components/FestivalAutoMixModal.svelte";
  import type { FestivalMixGroupData } from "$lib/festival_mix_helper";
  import type { DrawingCategoryInfo, DrawingSummary } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    onClose: () => void;
    onComplete: () => void;
  }

  let { eventId, eventName = "Event", onClose, onComplete }: Props = $props();

  let categories = $state<DrawingCategoryInfo[]>([]);
  let isLoading = $state(true);
  let isExecuting = $state(false);
  let executionError = $state<string | null>(null);
  let executionSuccess = $state<DrawingSummary | null>(null);

  // Selections
  let selectedCategoryIds = $state<number[]>([]);
  let shuffleType = $state<string>("both"); // "both" (kontingen & perguruan) or "perguruan"
  let searchQuery = $state("");

  // Non-Destructive Bagan Mix State & Auto Mix
  let mixGroups = $state<FestivalMixGroupData[]>([]);
  let autoMixModalOpen = $state(false);

  let mixedCategoryIds = $derived.by(() => {
    const s = new Set<number>();
    for (const g of mixGroups) {
      s.add(g.primary_category_id);
      for (const sid of g.secondary_category_ids) {
        s.add(sid);
      }
    }
    return s;
  });

  // Solo Category Safeguard: Only categories with 1 athlete that are NOT yet part of any mix group
  let soloCategories = $derived(
    categories.filter((c) => c.athlete_count === 1 && !mixedCategoryIds.has(c.id))
  );
  let hasSoloCategories = $derived(soloCategories.length > 0);
  let showSoloWarningModal = $state(false);
  let bypassSoloWarning = $state(false);

  let smartMixModalOpen = $state(false);
  let activeMixSourceCategory = $state<DrawingCategoryInfo | null>(null);

  function openSmartMixForCategory(cat: DrawingCategoryInfo) {
    activeMixSourceCategory = cat;
    smartMixModalOpen = true;
  }

  function getCategoryMixInfo(catId: number): {
    isPrimary: boolean;
    baganName: string;
    primaryId?: number;
    combinedAthleteCount?: number;
    totalClasses?: number;
  } | null {
    for (const g of mixGroups) {
      if (g.primary_category_id === catId) {
        const secCats = categories.filter((c) => g.secondary_category_ids.includes(c.id));
        const primaryCat = categories.find((c) => c.id === catId);
        const combinedCount =
          (primaryCat?.athlete_count ?? 0) +
          secCats.reduce((sum, c) => sum + c.athlete_count, 0);
        return {
          isPrimary: true,
          baganName: g.bagan_name,
          combinedAthleteCount: combinedCount,
          totalClasses: 1 + g.secondary_category_ids.length,
        };
      }
      if (g.secondary_category_ids.includes(catId)) {
        return { isPrimary: false, baganName: g.bagan_name, primaryId: g.primary_category_id };
      }
    }
    return null;
  }

  async function handleRemoveMixGroup(primaryCatId: number) {
    mixGroups = mixGroups.filter((g) => g.primary_category_id !== primaryCatId);
    try {
      await invoke("save_festival_mix_groups", { eventId, mixGroups });
    } catch (e) {
      console.error("Gagal menghapus mix group:", e);
    }
  }

  async function handleConfirmAutoMix(newMixList: FestivalMixGroupData[]) {
    mixGroups = newMixList;
    try {
      await invoke("save_festival_mix_groups", { eventId, mixGroups });
    } catch (e) {
      console.error("Gagal menyimpan auto mix:", e);
    }
  }

  onMount(async () => {
    await loadCategories();
  });

  async function loadCategories() {
    isLoading = true;
    executionError = null;
    try {
      const [data, loadedMix] = await Promise.all([
        invoke<DrawingCategoryInfo[]>("get_drawing_eligible_festival_categories", { eventId }),
        invoke<FestivalMixGroupData[]>("get_festival_mix_groups", { eventId }).catch(() => []),
      ]);
      categories = data || [];
      mixGroups = loadedMix || [];
      // Auto-select categories that have athletes
      selectedCategoryIds = categories
        .filter((c) => c.athlete_count > 0)
        .map((c) => c.id);
    } catch (err: any) {
      console.error("Gagal memuat kategori festival:", err);
      // Fallback HTTP if invoke error
      try {
        const res = await fetch(`/api/festival/drawing-eligible-categories/${eventId}`);
        if (res.ok) {
          const json = await res.json();
          categories = json.data || [];
          selectedCategoryIds = categories.filter((c) => c.athlete_count > 0).map((c) => c.id);
        } else {
          executionError = String(err?.message || err);
        }
      } catch {
        executionError = String(err?.message || err);
      }
    } finally {
      isLoading = false;
    }
  }

  let filteredCategories = $derived(
    categories.filter((c) =>
      c.nama_nomor_tanding.toLowerCase().includes(searchQuery.toLowerCase())
    )
  );

  let secondaryCategoryIds = $derived.by(() => {
    const s = new Set<number>();
    for (const g of mixGroups) {
      for (const sid of g.secondary_category_ids) {
        s.add(sid);
      }
    }
    return s;
  });

  // Effective bagan count to be generated (secondary categories are merged into their primary bagan)
  let effectiveBaganCount = $derived(
    categories.filter(
      (c) => selectedCategoryIds.includes(c.id) && c.athlete_count > 0 && !secondaryCategoryIds.has(c.id)
    ).length
  );

  let selectedEligibleCount = $derived(
    categories.filter((c) => selectedCategoryIds.includes(c.id) && c.athlete_count > 0).length
  );

  let unmixedSelectedSoloCategories = $derived(
    categories.filter(
      (c) =>
        selectedCategoryIds.includes(c.id) &&
        c.athlete_count === 1 &&
        !mixedCategoryIds.has(c.id)
    )
  );

  let totalSelectedAthletes = $derived(
    categories
      .filter((c) => selectedCategoryIds.includes(c.id))
      .reduce((sum, c) => sum + c.athlete_count, 0)
  );

  let estimatedBaganSheets = $derived(
    categories
      .filter((c) => selectedCategoryIds.includes(c.id) && c.athlete_count > 0 && !secondaryCategoryIds.has(c.id))
      .reduce((sum, c) => {
        const mixInfo = getCategoryMixInfo(c.id);
        const count =
          mixInfo?.isPrimary && mixInfo.combinedAthleteCount !== undefined
            ? mixInfo.combinedAthleteCount
            : c.athlete_count;
        const groups = Math.ceil(count / 4);
        const sheets = Math.ceil(groups / 4);
        return sum + Math.max(1, sheets);
      }, 0)
  );

  function toggleCategory(catId: number) {
    if (selectedCategoryIds.includes(catId)) {
      selectedCategoryIds = selectedCategoryIds.filter((id) => id !== catId);
    } else {
      selectedCategoryIds = [...selectedCategoryIds, catId];
    }
  }

  function selectAll() {
    selectedCategoryIds = categories.filter((c) => c.athlete_count > 0).map((c) => c.id);
  }

  function deselectAll() {
    selectedCategoryIds = [];
  }

  function getGroupBreakdown(count: number): { groups: number; sheets: number; label: string } {
    if (count <= 0) return { groups: 0, sheets: 0, label: "0 Atlet (Kosong)" };
    const groups = Math.ceil(count / 4);
    const sheets = Math.ceil(groups / 4);
    if (sheets === 1) {
      return { groups, sheets, label: `${groups} Grup (1 Lembar A4)` };
    }
    return { groups, sheets, label: `${groups} Grup (${sheets} Lembar A4)` };
  }

  async function handleExecuteDrawing() {
    if (selectedCategoryIds.length === 0) return;

    // Safeguard Gate: check if any unmixed selected category has only 1 athlete
    if (unmixedSelectedSoloCategories.length > 0 && !bypassSoloWarning) {
      showSoloWarningModal = true;
      return;
    }

    isExecuting = true;
    executionError = null;
    executionSuccess = null;
    bypassSoloWarning = false;
    showSoloWarningModal = false;

    try {
      const summary = await invoke<DrawingSummary>("execute_drawing_festival_bagan", {
        eventId,
        categoryIds: selectedCategoryIds,
        shuffleType,
      });

      executionSuccess = summary;
      setTimeout(() => {
        onComplete();
      }, 1500);
    } catch (err: any) {
      console.error("Gagal melakukan drawing festival:", err);
      // Fallback HTTP
      try {
        const res = await fetch(`/api/festival/execute-drawing`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            event_id: eventId,
            category_ids: selectedCategoryIds,
            shuffle_type: shuffleType,
          }),
        });
        if (res.ok) {
          const json = await res.json();
          executionSuccess = json.data;
          setTimeout(() => {
            onComplete();
          }, 1500);
        } else {
          const errJson = await res.json().catch(() => ({}));
          executionError = errJson.message || String(err?.message || err);
        }
      } catch {
        executionError = String(err?.message || err);
      }
    } finally {
      isExecuting = false;
    }
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm animate-fade-in">
  <div class="bg-[#14161b] border border-white/10 rounded-2xl w-full max-w-3xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden font-sans text-zinc-200">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-white/[0.08] flex items-center justify-between bg-[#191c23]">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-emerald-500/20 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-lg">
          🎋
        </div>
        <div>
          <div class="flex items-center gap-2">
            <h2 class="text-sm font-black text-white uppercase tracking-wide">
              Drawing Bagan Festival
            </h2>
            <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
              4 ATLET / GRUP
            </span>
          </div>
          <p class="text-[11px] text-zinc-400 mt-0.5">
            {eventName} • Sistem Gugur Mini 4 Atlet Per Grup (Maksimal 4 Grup Per Lembar Cetak A4 Landscape)
          </p>
        </div>
      </div>
      <button
        onclick={onClose}
        disabled={isExecuting}
        class="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition"
      >
        ✕
      </button>
    </div>

    <!-- Content -->
    <div class="p-6 flex-1 overflow-y-auto space-y-5">
      {#if executionError}
        <div class="p-3.5 rounded-xl bg-rose-500/15 border border-rose-500/30 text-rose-300 text-xs flex items-center space-x-2">
          <span>⚠️</span>
          <span>{executionError}</span>
        </div>
      {/if}

      {#if executionSuccess}
        <div class="p-4 rounded-xl bg-emerald-500/20 border border-emerald-500/40 text-emerald-200 text-xs space-y-1">
          <div class="font-bold text-emerald-300 flex items-center gap-1.5">
            <span>✅</span>
            <span>Drawing Bagan Festival Berhasil Dibuat!</span>
          </div>
          <p class="text-zinc-300">
            {executionSuccess.categories_processed} Kategori diproses • {executionSuccess.bagans_created} Bagan dibuat • {executionSuccess.total_matches} Total Partai
          </p>
        </div>
      {/if}

      <!-- Solo Category Warning Banner -->
      {#if hasSoloCategories}
        <div class="p-4 rounded-xl bg-amber-500/15 border border-amber-500/30 text-amber-200 text-xs flex items-center justify-between gap-3">
          <div class="flex items-center space-x-3 min-w-0">
            <span class="text-xl shrink-0">⚠️</span>
            <div class="min-w-0">
              <div class="font-black text-amber-300 uppercase tracking-tight flex items-center gap-2">
                <span>Terdeteksi {soloCategories.length} Kategori Festival Solo (1 Atlet)</span>
                <span class="px-2 py-0.5 rounded text-[10px] font-mono bg-amber-500/20 border border-amber-500/40 animate-pulse">BELUM ADA LAWAN</span>
              </div>
              <p class="text-[11px] text-zinc-300 mt-1">
                Atlet dalam kategori solo belum memiliki lawan tanding. Disarankan me-mix (menggabungkan) ke kategori terdekat yang serumpun agar atlet dapat bertanding di matras.
              </p>
            </div>
          </div>
          <div class="flex items-center space-x-2 shrink-0">
            <button
              type="button"
              onclick={() => (autoMixModalOpen = true)}
              class="px-3.5 py-1.5 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-black font-black text-xs rounded-xl shadow-md transition active:scale-95 flex items-center space-x-1.5 cursor-pointer"
              title="Otomatis mencocokkan dan menyusun bagan mix untuk seluruh kategori solo"
            >
              <span>⚡ Auto Mix</span>
            </button>
          </div>
        </div>
      {/if}

      <!-- Options Row -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <!-- Anti-Bentrok Selection -->
        <div class="p-3.5 rounded-xl bg-white/[0.02] border border-white/[0.06] space-y-2">
          <span class="text-[11px] font-mono font-bold uppercase text-zinc-400 block tracking-wider">
            Aturan Pengacakan Anti-Bentrok
          </span>
          <div class="space-y-1.5 text-xs">
            <label class="flex items-center space-x-2 cursor-pointer p-1.5 rounded hover:bg-white/[0.04]">
              <input
                type="radio"
                bind:group={shuffleType}
                value="both"
                class="accent-emerald-500"
              />
              <span class="text-zinc-200">Pisahkan Kontingen & Perguruan (Default)</span>
            </label>
            <label class="flex items-center space-x-2 cursor-pointer p-1.5 rounded hover:bg-white/[0.04]">
              <input
                type="radio"
                bind:group={shuffleType}
                value="perguruan"
                class="accent-emerald-500"
              />
              <span class="text-zinc-200">Pisahkan Perguruan Saja</span>
            </label>
          </div>
        </div>

        <!-- Format Explanation Summary -->
        <div class="p-3.5 rounded-xl bg-emerald-950/20 border border-emerald-500/20 text-xs text-zinc-300 space-y-1.5">
          <div class="font-bold text-emerald-400 flex items-center gap-1">
            <span>💡</span>
            <span>Prinsip Lembar Bagan Festival:</span>
          </div>
          <ul class="text-[11px] text-zinc-400 list-disc list-inside space-y-1">
            <li>Setiap grup berisi <strong>4 atlet</strong> (Semi 1, Semi 2, Final Grup).</li>
            <li><strong>1 Lembar Bagan A4 Landscape</strong> menampung hingga <strong>4 grup</strong> (16 atlet).</li>
            <li>Dalam 1 bagan penuh terdapat <strong>4 Emas, 4 Perak, 8 Perunggu</strong>.</li>
            <li>Sangat hemat kertas saat dicetak (semua grup dalam 1 lembar A4).</li>
          </ul>
        </div>
      </div>

      <!-- Category Filter & Action Bar -->
      <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pt-2">
        <div class="flex items-center space-x-2">
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Cari kategori festival..."
            class="bg-black/50 border border-white/[0.08] focus:border-emerald-500/50 rounded-lg px-3 py-1.5 text-xs text-white placeholder-zinc-500 outline-none w-56"
          />
          <span class="text-xs text-zinc-400 font-mono">
            {filteredCategories.length} Kategori
          </span>
        </div>

        <div class="flex items-center space-x-2 text-xs">
          <button
            type="button"
            onclick={() => (autoMixModalOpen = true)}
            class="px-2.5 py-1 rounded bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 border border-amber-500/30 font-bold transition flex items-center space-x-1 cursor-pointer"
            title="Auto mix kategori festival solo"
          >
            <span>⚡ Auto Mix</span>
          </button>
          <button
            type="button"
            onclick={selectAll}
            class="px-2.5 py-1 rounded bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium transition cursor-pointer"
          >
            Pilih Semua
          </button>
          <button
            type="button"
            onclick={deselectAll}
            class="px-2.5 py-1 rounded bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium transition cursor-pointer"
          >
            Hapus Pilihan
          </button>
        </div>
      </div>

      <!-- Categories List Table -->
      <div class="border border-white/[0.08] rounded-xl overflow-hidden bg-black/30">
        {#if isLoading}
          <div class="py-12 text-center text-xs text-zinc-500 italic">
            Memuat daftar nomor tanding festival...
          </div>
        {:else if categories.length === 0}
          <div class="py-12 text-center text-xs text-zinc-500 italic">
            Tidak ada nomor tanding festival ditemukan untuk event ini.
          </div>
        {:else}
          <div class="max-h-72 overflow-y-auto divide-y divide-white/[0.04]">
            {#each filteredCategories as cat}
              {@const mixInfo = getCategoryMixInfo(cat.id)}
              {@const isSelected = selectedCategoryIds.includes(cat.id)}
              {@const hasAthletes = cat.athlete_count > 0}
              {@const effectiveAthleteCount = (mixInfo?.isPrimary && mixInfo.combinedAthleteCount !== undefined) ? mixInfo.combinedAthleteCount : cat.athlete_count}
              {@const breakdown = getGroupBreakdown(effectiveAthleteCount)}
              <div
                class="px-3.5 py-2.5 flex items-center justify-between hover:bg-white/[0.02] transition {isSelected ? 'bg-emerald-500/[0.04]' : ''} {!hasAthletes ? 'opacity-40' : ''}"
              >
                <div class="flex items-center space-x-3 min-w-0 flex-1">
                  <input
                    type="checkbox"
                    checked={isSelected}
                    disabled={!hasAthletes || isExecuting}
                    onchange={() => toggleCategory(cat.id)}
                    class="accent-emerald-500 w-4 h-4 rounded cursor-pointer disabled:cursor-not-allowed"
                  />
                  <div class="min-w-0 flex-1">
                    <div class="text-xs font-semibold text-zinc-200 truncate flex items-center space-x-1.5">
                      {#if mixInfo}
                        {#if mixInfo.isPrimary}
                          <span class="text-amber-300 font-bold truncate">[{mixInfo.baganName}]</span>
                          <span class="text-[10px] text-zinc-400 truncate">(Asal: {cat.nama_nomor_tanding})</span>
                        {:else}
                          <span class="text-zinc-400 line-through truncate">{cat.nama_nomor_tanding}</span>
                        {/if}
                      {:else}
                        <span class="truncate">{cat.nama_nomor_tanding}</span>
                      {/if}
                    </div>
                    <div class="text-[10px] text-zinc-400 font-mono mt-0.5 flex items-center gap-2">
                      {#if mixInfo?.isPrimary && mixInfo.combinedAthleteCount !== undefined}
                        <span class="text-amber-400 font-bold">
                          {mixInfo.combinedAthleteCount} Atlet ({mixInfo.totalClasses} Kelas Gabungan)
                        </span>
                        <span>•</span>
                        <span>{breakdown.label}</span>
                      {:else if mixInfo && !mixInfo.isPrimary}
                        <span class="text-zinc-400 italic">
                          {cat.athlete_count} Atlet • Dilebur ke Bagan #{mixInfo.primaryId}
                        </span>
                      {:else}
                        <span class="text-emerald-400 font-bold">{cat.athlete_count} Atlet</span>
                        <span>•</span>
                        <span>{breakdown.label}</span>
                      {/if}
                    </div>
                  </div>
                </div>

                <div class="flex items-center space-x-2 shrink-0 ml-3">
                  {#if mixInfo}
                    {#if mixInfo.isPrimary}
                      <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black bg-amber-500/20 text-amber-300 border border-amber-500/40">
                        ⚡ BAGAN MIX ({mixInfo.totalClasses} KELAS)
                      </span>
                      <button
                        type="button"
                        onclick={(e) => { e.stopPropagation(); handleRemoveMixGroup(cat.id); }}
                        class="text-zinc-400 hover:text-red-400 px-1 py-0.5 text-xs transition cursor-pointer font-bold"
                        title="Batalkan Mix untuk bagan ini"
                      >
                        ✕ Batal
                      </button>
                    {:else}
                      <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-white/[0.06] text-zinc-400 border border-white/[0.1]">
                        ⚡ Digabung #{mixInfo.primaryId}
                      </span>
                    {/if}
                  {:else if cat.athlete_count === 1}
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 animate-pulse">
                      1 Atlet (Solo)
                    </span>
                    <button
                      type="button"
                      onclick={(e) => { e.stopPropagation(); openSmartMixForCategory(cat); }}
                      class="h-7 px-2.5 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-black font-black text-[10px] rounded-lg shadow-xs flex items-center space-x-1 transition active:scale-95 cursor-pointer"
                      title="Buka rekomendasi cerdas untuk menggabungkan kategori ini"
                    >
                      <span>⚡ Mix</span>
                    </button>
                  {/if}
                  {#if cat.bagan_count > 0}
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/15 text-amber-400 border border-amber-500/20">
                      {cat.bagan_count} Bagan Ada
                    </span>
                  {:else}
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono bg-zinc-800 text-zinc-400">
                      Belum Diundi
                    </span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Bottom Metric Summary Bar -->
      <div class="p-3.5 rounded-xl bg-white/[0.02] border border-white/[0.06] flex items-center justify-between text-xs">
        <div class="flex items-center space-x-4">
          <div>
            <span class="text-zinc-500 block text-[10px] uppercase font-mono">Bagan yang Dibuat</span>
            <span class="font-bold text-white text-sm">{effectiveBaganCount} Bagan</span>
          </div>
          <div class="border-l border-white/[0.08] pl-4">
            <span class="text-zinc-500 block text-[10px] uppercase font-mono">Total Atlet</span>
            <span class="font-bold text-emerald-400 text-sm">{totalSelectedAthletes}</span>
          </div>
          <div class="border-l border-white/[0.08] pl-4">
            <span class="text-zinc-500 block text-[10px] uppercase font-mono">Estimasi Lembar Bagan</span>
            <span class="font-bold text-amber-400 text-sm">{estimatedBaganSheets} Lembar A4</span>
          </div>
        </div>

        <div class="text-[11px] text-zinc-400 text-right">
          Maksimal 4 Grup per Lembar A4
        </div>
      </div>
    </div>

    <!-- Footer Actions -->
    <div class="px-6 py-4 border-t border-white/[0.08] flex items-center justify-between bg-[#191c23]">
      <button
        type="button"
        onclick={onClose}
        disabled={isExecuting}
        class="px-4 py-2 text-xs font-semibold text-zinc-400 hover:text-white rounded-xl hover:bg-white/[0.06] transition cursor-pointer"
      >
        Tutup
      </button>

      <button
        type="button"
        onclick={handleExecuteDrawing}
        disabled={effectiveBaganCount === 0 || isExecuting}
        class="px-5 py-2 text-xs font-bold rounded-xl bg-emerald-500 hover:bg-emerald-400 text-black shadow-lg shadow-emerald-500/20 active:scale-95 transition disabled:opacity-50 disabled:cursor-not-allowed flex items-center space-x-2 cursor-pointer"
      >
        {#if isExecuting}
          <div class="w-3.5 h-3.5 border-2 border-black border-t-transparent rounded-full animate-spin"></div>
          <span>Membuat Bagan Festival...</span>
        {:else}
          <span>🎋 Generate Bagan Festival ({effectiveBaganCount} Bagan)</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Modal: Solo Safeguard Gate Confirmation -->
  {#if showSoloWarningModal}
    {@const targetSoloCats = unmixedSelectedSoloCategories}
    <div class="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/85 backdrop-blur-sm animate-fade-in">
      <div class="bg-[#181a20] border border-amber-500/40 rounded-2xl p-6 shadow-2xl max-w-lg w-full space-y-4">
        <div class="flex items-start space-x-3">
          <div class="w-10 h-10 rounded-xl bg-amber-500/20 border border-amber-500/30 flex items-center justify-center text-amber-400 font-bold text-xl shrink-0">
            ⚠️
          </div>
          <div>
            <h3 class="text-sm font-black text-white uppercase tracking-tight">
              Peringatan: Kategori Festival Solo ({targetSoloCats.length} Kategori)
            </h3>
            <p class="text-xs text-zinc-300 mt-1 leading-relaxed">
              Kategori di bawah ini hanya memiliki <strong>1 atlet</strong> (tanpa lawan bertanding). Jika langsung diundi, atlet akan otomatis juara 1 tanpa tanding.
            </p>
          </div>
        </div>

        <div class="max-h-44 overflow-y-auto rounded-xl border border-white/[0.08] bg-[#14161b] p-3 space-y-2 scrollbar-thin">
          {#each targetSoloCats as cat}
            <div class="flex items-center justify-between text-xs py-1.5 border-b border-white/[0.04] last:border-0">
              <div class="min-w-0 flex-1 mr-2">
                <span class="text-white font-medium block truncate">{cat.nama_nomor_tanding}</span>
                <span class="text-[10px] font-mono text-amber-400">1 Atlet • Sendirian di bagan</span>
              </div>
              <button
                type="button"
                onclick={() => { showSoloWarningModal = false; openSmartMixForCategory(cat); }}
                class="px-2.5 py-1 rounded-lg bg-amber-500 hover:bg-amber-400 text-black text-[10px] font-black shrink-0 transition active:scale-95 cursor-pointer"
              >
                ⚡ Mix Ini
              </button>
            </div>
          {/each}
        </div>

        <div class="p-3 rounded-xl bg-emerald-950/20 border border-emerald-500/20 text-[11px] text-zinc-300 space-y-1">
          <div class="font-bold text-emerald-400 flex items-center gap-1">
            <span>💡</span>
            <span>Rekomendasi Turnamen:</span>
          </div>
          <p class="text-zinc-400">
            Gunakan fitur <strong>⚡ Mix</strong> untuk menggabungkan atlet ke kategori terdekat (sesama umur / beda 1 tingkat umur atau bobot berdekatan) agar atlet mendapatkan pengalaman bertanding.
          </p>
        </div>

        <div class="flex items-center justify-between pt-2 border-t border-white/[0.08] gap-2">
          <button
            type="button"
            onclick={() => (showSoloWarningModal = false)}
            class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 text-xs font-semibold rounded-xl border border-white/10 transition cursor-pointer"
          >
            Batal
          </button>
          <div class="flex items-center space-x-2">
            <button
              type="button"
              onclick={() => {
                bypassSoloWarning = true;
                handleExecuteDrawing();
              }}
              class="h-9 px-3.5 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-xs font-semibold rounded-xl transition cursor-pointer"
              title="Tetap undi solo sesuai kesepakatan Technical Meeting"
            >
              Tetap Undi Solo (Otomatis Juara)
            </button>
            {#if targetSoloCats.length > 1}
              <button
                type="button"
                onclick={() => {
                  showSoloWarningModal = false;
                  autoMixModalOpen = true;
                }}
                class="h-9 px-4 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-black font-black text-xs rounded-xl shadow-md transition active:scale-95 flex items-center space-x-1.5 cursor-pointer"
              >
                <span>⚡ Auto Mix Semua ({targetSoloCats.length} Kategori)</span>
              </button>
            {:else if targetSoloCats.length === 1}
              <button
                type="button"
                onclick={() => {
                  showSoloWarningModal = false;
                  openSmartMixForCategory(targetSoloCats[0]);
                }}
                class="h-9 px-4 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-black font-black text-xs rounded-xl shadow-md transition active:scale-95 flex items-center space-x-1.5 cursor-pointer"
              >
                <span>⚡ Buka Rekomendasi Mix</span>
              </button>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: Festival Auto Mix Assistant -->
  {#if autoMixModalOpen}
    <FestivalAutoMixModal
      {eventId}
      categories={categories.map((c) => ({
        id: c.id,
        nama: c.nama_nomor_tanding,
        athlete_count: c.athlete_count,
      }))}
      existingMixGroups={mixGroups}
      onClose={() => (autoMixModalOpen = false)}
      onConfirmMix={handleConfirmAutoMix}
    />
  {/if}

  <!-- Modal: Festival Smart Mix Assistant -->
  {#if smartMixModalOpen && activeMixSourceCategory}
    <FestivalSmartMixModal
      {eventId}
      sourceCategory={{
        id: activeMixSourceCategory.id,
        nama: activeMixSourceCategory.nama_nomor_tanding,
        athlete_count: activeMixSourceCategory.athlete_count,
      }}
      allCategories={categories.map((c) => ({
        id: c.id,
        nama: c.nama_nomor_tanding,
        athlete_count: c.athlete_count,
      }))}
      onClose={() => {
        smartMixModalOpen = false;
        activeMixSourceCategory = null;
      }}
      onMixed={async () => {
        await loadCategories();
      }}
    />
  {/if}
</div>
