<script lang="ts">
  import { invoke } from "$lib/api_client";
  import {
    parseFestivalCategory,
    getFestivalMixRecommendations,
    type MixCandidateRecommendation,
    type FestivalCategoryItemLike,
    type FestivalMixGroupData,
  } from "$lib/festival_mix_helper";

  interface Props {
    eventId: number;
    sourceCategory: FestivalCategoryItemLike;
    allCategories: FestivalCategoryItemLike[];
    onClose: () => void;
    onMixed: (newCategoryName: string) => void;
    onShowToast?: (message: string, type?: "success" | "error" | "info") => void;
  }

  let {
    eventId,
    sourceCategory,
    allCategories,
    onClose,
    onMixed,
    onShowToast = () => {},
  }: Props = $props();

  let isSubmitting = $state(false);
  let errorMessage = $state<string | null>(null);

  // Parse source category info
  let sourceParsed = $derived(parseFestivalCategory(sourceCategory.nama));

  // Compute recommendations
  let recommendations = $derived(
    getFestivalMixRecommendations(sourceCategory, allCategories)
  );

  // Best matches vs Warnings
  let bestMatches = $derived(recommendations.filter((r) => r.quality === "best"));
  let warningMatches = $derived(recommendations.filter((r) => r.quality === "warning"));

  // Track editable names per candidate id
  let customNames = $state<Record<number, string>>({});

  function getCustomOrSuggestedName(rec: MixCandidateRecommendation): string {
    if (customNames[rec.categoryId] !== undefined) {
      return customNames[rec.categoryId];
    }
    return rec.suggestedName;
  }

  function handleNameChange(categoryId: number, value: string) {
    customNames[categoryId] = value;
  }

  async function handleExecuteMix(rec: MixCandidateRecommendation) {
    const finalName = getCustomOrSuggestedName(rec).trim().toUpperCase();
    if (!finalName) {
      errorMessage = "Nama kategori gabungan baru tidak boleh kosong.";
      return;
    }

    isSubmitting = true;
    errorMessage = null;

    try {
      // Non-destructive: Persist into FestivalMixGroup without mutating or deleting master categories!
      let currentMixGroups: FestivalMixGroupData[] = [];
      try {
        currentMixGroups = (await invoke<FestivalMixGroupData[]>("get_festival_mix_groups", { eventId })) || [];
      } catch {
        currentMixGroups = [];
      }

      // Remove any existing mix group involving these categories
      const updated = currentMixGroups.filter(
        (g) =>
          g.primary_category_id !== sourceCategory.id &&
          !g.secondary_category_ids.includes(sourceCategory.id) &&
          g.primary_category_id !== rec.categoryId &&
          !g.secondary_category_ids.includes(rec.categoryId)
      );

      updated.push({
        primary_category_id: sourceCategory.id,
        secondary_category_ids: [rec.categoryId],
        bagan_name: finalName,
      });

      await invoke("save_festival_mix_groups", {
        eventId,
        mixGroups: updated,
      });

      onShowToast(`Bagan berhasil diset menjadi mix: ${finalName}`, "success");
      onMixed(finalName);
      onClose();
    } catch (err: any) {
      console.error("Gagal menyimpan mix festival:", err);
      errorMessage = String(err?.message || err);
    } finally {
      isSubmitting = false;
    }
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm animate-fade-in no-print">
  <div class="bg-[#14161b] border border-white/10 rounded-2xl w-full max-w-3xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden font-sans text-zinc-200">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-white/[0.08] flex items-center justify-between bg-[#191c23] shrink-0">
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-amber-500/20 border border-amber-500/30 flex items-center justify-center text-amber-400 font-bold text-lg">
          ⚡
        </div>
        <div>
          <div class="flex items-center gap-2">
            <h2 class="text-sm font-black text-white uppercase tracking-wide">
              Rekomendasi Mix Kategori Festival
            </h2>
            <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30">
              ANTI-BENTROK OTOMATIS
            </span>
          </div>
          <p class="text-[11px] text-zinc-400 mt-0.5">
            Pilih kategori target terdekat untuk digabungkan agar atlet memiliki lawan tanding yang adil & proporsional
          </p>
        </div>
      </div>
      <button
        onclick={onClose}
        disabled={isSubmitting}
        class="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition"
        title="Tutup dialog"
      >
        ✕
      </button>
    </div>

    <!-- Content -->
    <div class="p-6 flex-1 overflow-y-auto space-y-5">
      {#if errorMessage}
        <div class="p-3.5 rounded-xl bg-rose-500/15 border border-rose-500/30 text-rose-300 text-xs flex items-center space-x-2">
          <span>⚠️</span>
          <span>{errorMessage}</span>
        </div>
      {/if}

      <!-- Source Category Summary Card -->
      <div class="p-4 rounded-xl bg-amber-500/[0.07] border border-amber-500/30 space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-mono font-bold uppercase text-amber-400 tracking-wider">
            Kategori Asal (Ingin Digabung):
          </span>
          <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 animate-pulse">
            {sourceCategory.athlete_count ?? sourceCategory.atlet_count ?? 1} ATLET (KATEGORI SEPI)
          </span>
        </div>
        <div class="text-sm font-bold text-white leading-snug">
          {sourceCategory.nama}
        </div>
        <div class="flex items-center gap-2 flex-wrap text-[10px] font-mono text-zinc-300 pt-1">
          <span class="px-2 py-0.5 rounded bg-white/[0.08] text-white font-bold uppercase">
            {sourceParsed.discipline.toUpperCase()}
          </span>
          <span class="px-2 py-0.5 rounded bg-white/[0.08] text-white font-bold uppercase">
            {sourceParsed.gender.toUpperCase()}
          </span>
          {#if sourceParsed.ageGroup}
            <span class="px-2 py-0.5 rounded bg-white/[0.08] text-zinc-200">
              {sourceParsed.ageGroup}
            </span>
          {/if}
          {#if sourceParsed.weightVal !== null}
            <span class="px-2 py-0.5 rounded bg-white/[0.08] text-amber-300 font-bold">
              {sourceParsed.weightSign || ""}{sourceParsed.weightVal} KG
            </span>
          {/if}
        </div>
      </div>

      <!-- Recommendations List -->
      {#if recommendations.length === 0}
        <div class="p-8 rounded-xl border border-white/[0.08] bg-black/30 text-center space-y-2">
          <div class="text-3xl">🥋</div>
          <div class="text-xs font-bold text-zinc-300">
            Tidak Ditemukan Kategori Rekomendasi Terdekat
          </div>
          <p class="text-[11px] text-zinc-400 max-w-md mx-auto">
            Seluruh kategori festival lainnya memiliki perbedaan disiplin (Kata vs Kumite), beda gender (Putra vs Putri), atau selisih kelompok umur terlalu jauh (&ge; 2 tingkat).
          </p>
        </div>
      {:else}
        <!-- Group A: Best Matches (Same Age Group) -->
        {#if bestMatches.length > 0}
          <div class="space-y-3">
            <div class="flex items-center space-x-2">
              <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
              <h3 class="text-xs font-mono font-black text-emerald-400 uppercase tracking-wide">
                Sangat Direkomendasikan (Satu Kelompok Umur & Bobot Terdekat)
              </h3>
            </div>

            <div class="space-y-3">
              {#each bestMatches as rec}
                <div class="p-4 rounded-xl border border-emerald-500/30 bg-emerald-950/10 hover:border-emerald-500/50 transition space-y-3">
                  <div class="flex items-start justify-between gap-3">
                    <div class="min-w-0 flex-1">
                      <div class="text-xs font-bold text-white">
                        {rec.categoryName}
                      </div>
                      <div class="flex items-center gap-1.5 flex-wrap text-[10px] mt-1.5 font-mono">
                        <span class="px-2 py-0.5 rounded bg-emerald-500/20 text-emerald-300 font-bold">
                          {rec.athleteCount} Atlet Saat Ini
                        </span>
                        {#each rec.reasons as r}
                          <span class="px-2 py-0.5 rounded bg-white/[0.06] text-zinc-300">
                            ✓ {r}
                          </span>
                        {/each}
                      </div>
                    </div>
                  </div>

                  <!-- Suggested Merged Name Input -->
                  <div class="space-y-1">
                    <label for="name-{rec.categoryId}" class="block text-[10px] font-mono text-zinc-400 uppercase">
                      Saran Nama Kategori Baru (Bisa Diedit):
                    </label>
                    <input
                      id="name-{rec.categoryId}"
                      type="text"
                      value={getCustomOrSuggestedName(rec)}
                      oninput={(e) => handleNameChange(rec.categoryId, e.currentTarget.value)}
                      class="w-full bg-[#181a20] border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white uppercase font-bold focus:border-emerald-500 outline-none"
                    />
                  </div>

                  <div class="flex justify-end pt-1">
                    <button
                      onclick={() => handleExecuteMix(rec)}
                      disabled={isSubmitting}
                      class="px-4 py-2 bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-400 hover:to-teal-500 text-black font-black text-xs rounded-xl shadow-md transition active:scale-95 disabled:opacity-50 flex items-center space-x-1.5 cursor-pointer"
                    >
                      <span>⚡ Gabungkan ke Kategori Ini (1-Klik)</span>
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Group B: Warning Matches (Adjacent Age Group / Delta 1) -->
        {#if warningMatches.length > 0}
          <div class="space-y-3 pt-2">
            <div class="flex items-center space-x-2">
              <span class="w-2 h-2 rounded-full bg-amber-400"></span>
              <h3 class="text-xs font-mono font-black text-amber-400 uppercase tracking-wide">
                Boleh dengan Catatan (Beda 1 Kelompok Umur / Toleransi Bobot)
              </h3>
            </div>

            <div class="space-y-3">
              {#each warningMatches as rec}
                <div class="p-4 rounded-xl border border-amber-500/30 bg-amber-950/10 hover:border-amber-500/50 transition space-y-3">
                  <div class="flex items-start justify-between gap-3">
                    <div class="min-w-0 flex-1">
                      <div class="text-xs font-bold text-white">
                        {rec.categoryName}
                      </div>

                      <!-- Warnings -->
                      {#if rec.warnings.length > 0}
                        <div class="mt-1.5 space-y-1">
                          {#each rec.warnings as w}
                            <div class="text-[11px] font-bold text-amber-300 bg-amber-500/20 border border-amber-500/30 px-2.5 py-1 rounded-md flex items-center gap-1.5">
                              <span>⚠️</span>
                              <span>{w}</span>
                            </div>
                          {/each}
                        </div>
                      {/if}

                      <div class="flex items-center gap-1.5 flex-wrap text-[10px] mt-2 font-mono">
                        <span class="px-2 py-0.5 rounded bg-amber-500/20 text-amber-300 font-bold">
                          {rec.athleteCount} Atlet Saat Ini
                        </span>
                        {#each rec.reasons as r}
                          <span class="px-2 py-0.5 rounded bg-white/[0.06] text-zinc-300">
                            ✓ {r}
                          </span>
                        {/each}
                      </div>
                    </div>
                  </div>

                  <!-- Suggested Merged Name Input -->
                  <div class="space-y-1">
                    <label for="name-{rec.categoryId}" class="block text-[10px] font-mono text-zinc-400 uppercase">
                      Saran Nama Kategori Baru (Bisa Diedit):
                    </label>
                    <input
                      id="name-{rec.categoryId}"
                      type="text"
                      value={getCustomOrSuggestedName(rec)}
                      oninput={(e) => handleNameChange(rec.categoryId, e.currentTarget.value)}
                      class="w-full bg-[#181a20] border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white uppercase font-bold focus:border-amber-500 outline-none"
                    />
                  </div>

                  <div class="flex justify-end pt-1">
                    <button
                      onclick={() => handleExecuteMix(rec)}
                      disabled={isSubmitting}
                      class="px-4 py-2 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-black font-black text-xs rounded-xl shadow-md transition active:scale-95 disabled:opacity-50 flex items-center space-x-1.5 cursor-pointer"
                    >
                      <span>⚡ Gabungkan ke Kategori Ini (1-Klik)</span>
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-6 py-3 border-t border-white/[0.08] flex items-center justify-between bg-[#191c23] shrink-0 text-xs">
      <span class="text-zinc-500 text-[11px] font-mono">
        Anti-Bentrok kontingen akan otomatis diperbarui setelah penggabungan.
      </span>
      <button
        onclick={onClose}
        disabled={isSubmitting}
        class="h-9 px-4 bg-white/[0.08] hover:bg-white/[0.15] text-zinc-300 font-semibold text-xs rounded-xl transition cursor-pointer"
      >
        Tutup
      </button>
    </div>
  </div>
</div>
