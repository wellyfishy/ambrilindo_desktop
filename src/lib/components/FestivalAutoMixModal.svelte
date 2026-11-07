<script lang="ts">
  import {
    computeAutoMixProposals,
    addCategoryToProposal,
    removeTargetCategoryFromProposal,
    parseFestivalCategory,
    type AutoMixProposal,
    type FestivalCategoryItemLike,
    type FestivalMixGroupData,
  } from "$lib/festival_mix_helper";

  interface Props {
    eventId: number;
    categories: FestivalCategoryItemLike[];
    existingMixGroups?: FestivalMixGroupData[];
    onClose: () => void;
    onConfirmMix: (newMixList: FestivalMixGroupData[]) => void;
    onShowToast?: (message: string, type?: "success" | "error" | "info") => void;
  }

  let {
    eventId,
    categories,
    existingMixGroups = [],
    onClose,
    onConfirmMix,
    onShowToast = () => {},
  }: Props = $props();

  // Reactive auto mix proposal computation
  let computedResult = $derived(computeAutoMixProposals(categories, existingMixGroups));
  let proposals = $state<AutoMixProposal[]>([]);
  let unmatchedSolos = $state<FestivalCategoryItemLike[]>([]);

  // Synchronize state when computedResult changes
  $effect(() => {
    proposals = computedResult.proposals.map((p) => ({
      ...p,
      targetCategories: [...p.targetCategories],
      reasons: [...p.reasons],
      warnings: [...p.warnings],
    }));
    unmatchedSolos = [...computedResult.unmatchedSolos];
  });

  let selectedCount = $derived(proposals.filter((p) => p.selected).length);

  // Active dropdown picker state for adding another class to a proposal
  let activeAddPickerProposalId = $state<string | null>(null);

  function toggleProposal(id: string) {
    const p = proposals.find((x) => x.id === id);
    if (p) {
      p.selected = !p.selected;
    }
  }

  function toggleSelectAll() {
    const allSelected = proposals.every((p) => p.selected);
    for (const p of proposals) {
      p.selected = !allSelected;
    }
  }

  function handleNameChange(id: string, value: string) {
    const p = proposals.find((x) => x.id === id);
    if (p) {
      p.proposedBaganName = value.toUpperCase();
    }
  }

  /**
   * Adds another category to a proposal (enabling Mix 3 Kelas).
   */
  function handleAddCategory(proposalId: string, categoryToAdd: FestivalCategoryItemLike) {
    const idx = proposals.findIndex((x) => x.id === proposalId);
    if (idx === -1) return;

    const updated = addCategoryToProposal(proposals[idx], categoryToAdd);
    proposals[idx] = updated;

    // Remove from unmatched list if it was there
    unmatchedSolos = unmatchedSolos.filter((c) => c.id !== categoryToAdd.id);
    activeAddPickerProposalId = null;

    onShowToast(`Berhasil menambahkan kelas ${categoryToAdd.nama} ke bagan gabungan.`, "success");
  }

  /**
   * Removes a secondary category from a proposal.
   */
  function handleRemoveTargetCategory(proposalId: string, targetCatId: number) {
    const idx = proposals.findIndex((x) => x.id === proposalId);
    if (idx === -1) return;

    const prop = proposals[idx];
    const removedCat = prop.targetCategories.find((c) => c.id === targetCatId);

    if (prop.targetCategories.length > 1) {
      // Still has other target categories
      proposals[idx] = removeTargetCategoryFromProposal(prop, targetCatId);
      if (removedCat && (removedCat.athlete_count ?? removedCat.atlet_count ?? 1) <= 1) {
        if (!unmatchedSolos.some((c) => c.id === removedCat.id)) {
          unmatchedSolos = [...unmatchedSolos, removedCat];
        }
      }
    } else {
      // Discard proposal completely
      const sourceCat = prop.sourceCategory;
      proposals = proposals.filter((p) => p.id !== proposalId);

      // Return solo categories to unmatched list
      const toReturn = [sourceCat, ...(removedCat ? [removedCat] : [])].filter(
        (c) => (c.athlete_count ?? c.atlet_count ?? 1) <= 1
      );
      for (const ret of toReturn) {
        if (!unmatchedSolos.some((c) => c.id === ret.id)) {
          unmatchedSolos = [...unmatchedSolos, ret];
        }
      }
    }
    onShowToast("Kategori dikeluarkan dari bagan gabungan.", "info");
  }

  /**
   * Quick action from unmatched list to merge directly into an existing proposal.
   */
  function handleQuickMerge(unmatchedCat: FestivalCategoryItemLike, targetProposalId: string) {
    if (!targetProposalId) return;
    handleAddCategory(targetProposalId, unmatchedCat);
  }

  /**
   * Computes available candidates that can be added to a specific proposal.
   */
  function getEligibleAddCandidates(proposal: AutoMixProposal): FestivalCategoryItemLike[] {
    const sourceParsed = parseFestivalCategory(proposal.sourceCategory.nama);
    const existingIds = new Set([
      proposal.sourceCategory.id,
      ...proposal.targetCategories.map((c) => c.id),
    ]);

    return categories.filter((c) => {
      if (existingIds.has(c.id)) return false;
      const cnt = c.athlete_count ?? c.atlet_count ?? 0;
      if (cnt <= 0) return false;

      const candParsed = parseFestivalCategory(c.nama);
      // Same discipline & gender
      if (candParsed.discipline !== sourceParsed.discipline) return false;
      if (candParsed.gender !== sourceParsed.gender) return false;

      // Tier proximity <= 2
      if (sourceParsed.ageTier > 0 && candParsed.ageTier > 0) {
        if (Math.abs(sourceParsed.ageTier - candParsed.ageTier) > 2) return false;
      }
      return true;
    });
  }

  function handleApplyMix() {
    const selectedProposals = proposals.filter((p) => p.selected);
    if (selectedProposals.length === 0) {
      onShowToast("Pilih minimal satu usulan penggabungan bagan.", "info");
      return;
    }

    // Verify bagan names are not empty
    for (const p of selectedProposals) {
      if (!p.proposedBaganName.trim()) {
        onShowToast(`Nama bagan untuk penggabungan ID ${p.sourceCategory.id} tidak boleh kosong.`, "error");
        return;
      }
    }

    const newMixList: FestivalMixGroupData[] = [
      ...existingMixGroups,
      ...selectedProposals.map((p) => ({
        primary_category_id: p.sourceCategory.id,
        secondary_category_ids: p.targetCategories.map((c) => c.id),
        bagan_name: p.proposedBaganName.trim().toUpperCase(),
      })),
    ];

    onConfirmMix(newMixList);
    onShowToast(`Berhasil menerapkan mix untuk ${selectedProposals.length} bagan festival!`, "success");
    onClose();
  }
</script>

<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 overflow-y-auto animate-in fade-in duration-150 select-none"
  role="dialog"
  aria-modal="true"
  aria-labelledby="auto-mix-title"
>
  <div
    class="relative w-full max-w-4xl bg-[#14161d] border border-amber-500/40 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[92vh]"
  >
    <!-- Modal Header -->
    <div
      class="px-6 py-4 bg-gradient-to-r from-amber-950/60 via-slate-900 to-amber-950/40 border-b border-amber-500/30 flex items-center justify-between shrink-0"
    >
      <div class="flex items-center space-x-3 min-w-0">
        <div
          class="w-10 h-10 rounded-xl bg-amber-500/20 border border-amber-500/40 flex items-center justify-center text-amber-400 font-black text-xl shadow-inner shrink-0"
        >
          ⚡
        </div>
        <div class="min-w-0">
          <div class="flex items-center space-x-2">
            <h2 id="auto-mix-title" class="text-base font-black text-white tracking-wide truncate">
              Auto Mix Kategori Festival Solo & Multi-Kelas
            </h2>
            <span
              class="px-2 py-0.5 rounded-full text-[10px] font-mono font-black uppercase tracking-wider bg-amber-500/20 text-amber-300 border border-amber-500/40"
            >
              Non-Destructive • Mendukung Mix 3 Kelas
            </span>
          </div>
          <p class="text-xs text-zinc-400 mt-0.5 truncate">
            Sistem otomatis menganalisis nomor tanding solo & kelas kecil untuk digabungkan ke 1 bagan pertandingan resmi.
          </p>
        </div>
      </div>

      <button
        type="button"
        onclick={onClose}
        class="w-8 h-8 rounded-lg flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/10 transition shrink-0 ml-2 cursor-pointer"
        title="Tutup"
      >
        ✕
      </button>
    </div>

    <!-- Info Banner: Master Data Protection -->
    <div class="px-6 py-2.5 bg-amber-950/30 border-b border-amber-500/20 flex items-center justify-between text-xs text-amber-200/90 shrink-0">
      <div class="flex items-center space-x-2">
        <span class="text-base">🛡️</span>
        <span>
          <strong>Data Master Pendaftaran Aman:</strong> Nomor tanding awal tidak dihapus dan tidak diubah di database. Seluruh atlet dimasukkan ke dalam 1 bagan festival bersama.
        </span>
      </div>
      {#if proposals.length > 0}
        <button
          type="button"
          onclick={toggleSelectAll}
          class="text-xs font-mono font-bold text-amber-400 hover:text-amber-300 underline underline-offset-2 ml-4 shrink-0 cursor-pointer"
        >
          {proposals.every((p) => p.selected) ? "Batal Pilih Semua" : "Pilih Semua"}
        </button>
      {/if}
    </div>

    <!-- Modal Body (Scrollable Proposals List) -->
    <div class="p-6 overflow-y-auto flex-1 space-y-4">
      {#if proposals.length === 0}
        <div class="py-12 flex flex-col items-center justify-center text-center space-y-3">
          <div class="w-14 h-14 rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-2xl text-emerald-400">
            ✓
          </div>
          <div>
            <h3 class="text-sm font-bold text-white">Tidak Ada Kategori Solo yang Butuh Di-mix</h3>
            <p class="text-xs text-zinc-400 max-w-md mt-1">
              Seluruh kategori festival sudah memiliki atlet yang cukup, atau seluruh kategori solo telah digabung.
            </p>
          </div>
        </div>
      {:else}
        <div class="space-y-4">
          {#each proposals as prop, pIdx (prop.id)}
            {@const cntA = prop.sourceCategory.athlete_count ?? prop.sourceCategory.atlet_count ?? 1}
            {@const firstTarget = prop.targetCategories[0]}
            {@const cntFirstTarget = firstTarget ? (firstTarget.athlete_count ?? firstTarget.atlet_count ?? 1) : 0}
            {@const totalClasses = 1 + prop.targetCategories.length}
            {@const isAddingClass = activeAddPickerProposalId === prop.id}
            {@const eligibleCandidates = isAddingClass ? getEligibleAddCandidates(prop) : []}

            <div
              class="border rounded-xl p-4 transition {prop.selected ? 'bg-[#181b24] border-amber-500/50 shadow-md' : 'bg-[#121318] border-white/[0.08] opacity-60'}"
            >
              <!-- Card Header Bar -->
              <div class="flex items-center justify-between pb-3 border-b border-white/[0.08] mb-3">
                <label class="flex items-center space-x-2.5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={prop.selected}
                    onchange={() => toggleProposal(prop.id)}
                    class="w-4 h-4 rounded text-amber-500 focus:ring-amber-500 focus:ring-offset-0 bg-black/40 border-zinc-600 cursor-pointer"
                  />
                  <span class="text-xs font-black font-mono tracking-wide {prop.selected ? 'text-amber-400' : 'text-zinc-400'}">
                    USULAN #{pIdx + 1} • {totalClasses === 2 ? 'GABUNG 2 KELAS' : `MIX ${totalClasses} KELAS`}
                  </span>
                </label>

                <div class="flex items-center space-x-2">
                  {#if prop.quality === "best"}
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black uppercase bg-emerald-500/20 text-emerald-300 border border-emerald-500/40">
                      Best Match • Ideal
                    </span>
                  {:else}
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black uppercase bg-amber-500/20 text-amber-300 border border-amber-500/40">
                      Warning Usia • Diperbolehkan
                    </span>
                  {/if}

                  <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
                    Total {prop.combinedAthletes} Atlet
                  </span>

                  <!-- Button: Add Another Class (Mix 3 Kelas) -->
                  <button
                    type="button"
                    onclick={() => {
                      activeAddPickerProposalId = isAddingClass ? null : prop.id;
                    }}
                    class="px-2.5 py-1 rounded-lg text-[10px] font-mono font-bold transition flex items-center space-x-1 cursor-pointer {isAddingClass ? 'bg-amber-500 text-black font-black' : 'bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/30'}"
                    title="Tambahkan kelas lain ke bagan ini (Mix 3 Kelas)"
                  >
                    <span>{isAddingClass ? '✕ Batal' : '➕ Tambah Kelas'}</span>
                  </button>
                </div>
              </div>

              <!-- Match Candidates Flex/Grid -->
              <div class="space-y-2">
                <div class="grid grid-cols-1 md:grid-cols-12 gap-2 items-center">
                  <!-- Category 1 (Source Category) -->
                  <div class="md:col-span-5 p-2.5 rounded-lg bg-black/40 border border-white/[0.08] flex flex-col justify-between">
                    <div class="flex items-center justify-between text-[10px] font-mono text-zinc-400 mb-1">
                      <span class="font-bold text-amber-300">KELAS 1 (UTAMA)</span>
                      <span class="px-1.5 py-0.2 rounded font-black text-amber-400 bg-amber-400/10 border border-amber-400/20">
                        {cntA} ATLET
                      </span>
                    </div>
                    <div class="text-xs font-bold text-white leading-snug">
                      {prop.sourceCategory.nama}
                    </div>
                  </div>

                  <!-- Plus Symbol -->
                  <div class="md:col-span-1 flex flex-col items-center justify-center">
                    <div class="w-6 h-6 rounded-full bg-amber-500/20 border border-amber-500/40 flex items-center justify-center text-amber-400 font-black text-xs">
                      +
                    </div>
                  </div>

                  <!-- Category 2 (First Target) -->
                  <div class="md:col-span-6 p-2.5 rounded-lg bg-black/40 border border-white/[0.08] flex flex-col justify-between relative group">
                    <div class="flex items-center justify-between text-[10px] font-mono text-zinc-400 mb-1">
                      <span class="font-bold text-emerald-400">KELAS 2 (TARGET)</span>
                      <div class="flex items-center space-x-1.5">
                        <span class="px-1.5 py-0.2 rounded font-black text-emerald-400 bg-emerald-400/10 border border-emerald-400/20">
                          {cntFirstTarget} ATLET
                        </span>
                        {#if prop.targetCategories.length > 1}
                          <button
                            type="button"
                            onclick={() => handleRemoveTargetCategory(prop.id, firstTarget.id)}
                            class="text-zinc-500 hover:text-red-400 text-xs px-1 font-bold cursor-pointer transition"
                            title="Hapus kelas ini dari mix"
                          >
                            ✕
                          </button>
                        {/if}
                      </div>
                    </div>
                    <div class="text-xs font-bold text-white leading-snug">
                      {firstTarget ? firstTarget.nama : '-'}
                    </div>
                  </div>
                </div>

                <!-- Additional Target Categories (Classes 3, 4, etc.) -->
                {#if prop.targetCategories.length > 1}
                  {#each prop.targetCategories.slice(1) as extraTarget, eIdx (extraTarget.id)}
                    {@const cntExtra = extraTarget.athlete_count ?? extraTarget.atlet_count ?? 1}
                    <div class="grid grid-cols-1 md:grid-cols-12 gap-2 items-center pt-1 border-t border-white/[0.04]">
                      <div class="md:col-span-5 text-right text-[10px] font-mono text-zinc-500 font-bold pr-2">
                        + DITAMBAHKAN KE BAGAN INI
                      </div>
                      <div class="md:col-span-1 flex justify-center">
                        <div class="w-6 h-6 rounded-full bg-cyan-500/20 border border-cyan-500/40 flex items-center justify-center text-cyan-400 font-black text-xs">
                          +
                        </div>
                      </div>
                      <div class="md:col-span-6 p-2.5 rounded-lg bg-cyan-950/20 border border-cyan-500/30 flex flex-col justify-between">
                        <div class="flex items-center justify-between text-[10px] font-mono text-cyan-300 mb-1">
                          <span class="font-bold">KELAS {eIdx + 3} (MIX 3+ KELAS)</span>
                          <div class="flex items-center space-x-1.5">
                            <span class="px-1.5 py-0.2 rounded font-black text-cyan-300 bg-cyan-400/10 border border-cyan-400/20">
                              {cntExtra} ATLET
                            </span>
                            <button
                              type="button"
                              onclick={() => handleRemoveTargetCategory(prop.id, extraTarget.id)}
                              class="text-zinc-400 hover:text-red-400 text-xs px-1 font-bold cursor-pointer transition"
                              title="Hapus kelas ini dari mix"
                            >
                              ✕
                            </button>
                          </div>
                        </div>
                        <div class="text-xs font-bold text-cyan-100 leading-snug">
                          {extraTarget.nama}
                        </div>
                      </div>
                    </div>
                  {/each}
                {/if}
              </div>

              <!-- Inline Class Picker (When '+ Tambah Kelas' clicked) -->
              {#if isAddingClass}
                <div class="mt-3 p-3 rounded-lg bg-amber-950/20 border border-amber-500/30 space-y-2 animate-in fade-in duration-100">
                  <div class="flex items-center justify-between">
                    <span class="text-xs font-bold text-amber-300">
                      Pilih Kelas Tambahan untuk Digabungkan (Mix 3 Kelas):
                    </span>
                    <span class="text-[10px] font-mono text-zinc-400">
                      {eligibleCandidates.length} kategori yang kompatibel ditemukan
                    </span>
                  </div>

                  {#if eligibleCandidates.length === 0}
                    <p class="text-xs text-zinc-400 py-1">
                      Tidak ada kategori lain yang memiliki gender & disiplin yang sama.
                    </p>
                  {:else}
                    <div class="max-h-40 overflow-y-auto space-y-1.5 pr-1">
                      {#each eligibleCandidates as cand (cand.id)}
                        {@const candCnt = cand.athlete_count ?? cand.atlet_count ?? 1}
                        <button
                          type="button"
                          onclick={() => handleAddCategory(prop.id, cand)}
                          class="w-full text-left p-2 rounded-md bg-black/40 hover:bg-amber-500/20 border border-white/[0.08] hover:border-amber-500/40 transition flex items-center justify-between cursor-pointer group"
                        >
                          <div class="min-w-0 pr-2">
                            <div class="text-xs font-semibold text-zinc-200 group-hover:text-white truncate">
                              {cand.nama}
                            </div>
                          </div>
                          <div class="flex items-center space-x-2 shrink-0">
                            <span class="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold bg-white/[0.08] text-zinc-300">
                              {candCnt} Atlet
                            </span>
                            <span class="text-xs font-bold text-amber-400 group-hover:translate-x-0.5 transition">
                              ➕ Gabungkan
                            </span>
                          </div>
                        </button>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}

              <!-- Reasons & Warnings Chips -->
              <div class="flex flex-wrap items-center gap-1.5 mt-3 pt-2.5 border-t border-white/[0.04]">
                {#each prop.reasons as reason}
                  <span class="inline-flex items-center text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-950/40 text-emerald-300 border border-emerald-500/30">
                    ✓ {reason}
                  </span>
                {/each}
                {#each prop.warnings as warn}
                  <span class="inline-flex items-center text-[10px] font-mono px-2 py-0.5 rounded bg-amber-950/40 text-amber-300 border border-amber-500/30">
                    {warn}
                  </span>
                {/each}
              </div>

              <!-- Editable Proposed Bagan Name -->
              <div class="mt-3 pt-3 border-t border-white/[0.06]">
                <div class="flex items-center justify-between mb-1">
                  <label for="name-{prop.id}" class="text-[11px] font-bold font-mono text-zinc-300 flex items-center space-x-1.5">
                    <span>NAMA BAGAN RESMI (MIX):</span>
                    <span class="text-[9px] text-zinc-500 font-normal">(dapat disunting langsung)</span>
                  </label>
                </div>
                <input
                  id="name-{prop.id}"
                  type="text"
                  value={prop.proposedBaganName}
                  oninput={(e) => handleNameChange(prop.id, (e.target as HTMLInputElement).value)}
                  class="w-full bg-black/50 text-amber-300 font-mono font-bold text-xs px-3 py-2 rounded-lg border border-amber-500/40 focus:border-amber-400 focus:outline-none focus:ring-1 focus:ring-amber-400"
                  placeholder="FESTIVAL ... MIX"
                />
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Unmatched Solos Notice with Quick Action to Merge into existing proposal -->
      {#if unmatchedSolos.length > 0}
        <div class="p-4 rounded-xl bg-amber-950/20 border border-amber-500/30 space-y-3">
          <div class="flex items-center space-x-2 text-amber-300 text-xs font-bold">
            <span>⚠️</span>
            <span>Kategori Solo Tanpa Pasangan Otomatis ({unmatchedSolos.length} Kategori):</span>
          </div>
          <p class="text-[11px] text-zinc-400 leading-relaxed">
            Kategori di bawah ini belum terpasang otomatis. Anda dapat menggabungkannya ke salah satu usulan di atas sebagai <strong>kelas ke-3</strong> menggunakan tombol di samping, atau biarkan tetap bertanding sebagai nomor tanding mandiri (otomatis juara):
          </p>
          <div class="space-y-2">
            {#each unmatchedSolos as us (us.id)}
              {@const usCnt = us.athlete_count ?? us.atlet_count ?? 1}
              <div class="p-2.5 rounded-lg bg-black/40 border border-white/[0.08] flex items-center justify-between flex-wrap gap-2">
                <div class="min-w-0 flex-1">
                  <div class="text-xs font-bold text-white truncate">
                    {us.nama}
                  </div>
                  <div class="text-[10px] font-mono text-zinc-400">
                    {usCnt} Atlet
                  </div>
                </div>

                {#if proposals.length > 0}
                  <div class="flex items-center space-x-2">
                    <select
                      onchange={(e) => {
                        const targetId = (e.target as HTMLSelectElement).value;
                        if (targetId) {
                          handleQuickMerge(us, targetId);
                        }
                      }}
                      class="bg-black/60 text-amber-300 font-mono text-xs px-2.5 py-1.5 rounded border border-amber-500/40 focus:outline-none focus:border-amber-400 cursor-pointer"
                    >
                      <option value="">➕ Gabungkan ke Usulan...</option>
                      {#each proposals as p, idx}
                        <option value={p.id}>
                          Usulan #{idx + 1}: {p.proposedBaganName.slice(0, 45)}...
                        </option>
                      {/each}
                    </select>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <!-- Modal Footer Actions -->
    <div
      class="px-6 py-4 bg-[#101217] border-t border-white/[0.08] flex items-center justify-between shrink-0"
    >
      <div class="text-xs text-zinc-400 font-mono">
        {#if proposals.length > 0}
          <span>{selectedCount} dari {proposals.length} usulan terpilih</span>
        {/if}
      </div>

      <div class="flex items-center space-x-3">
        <button
          type="button"
          onclick={onClose}
          class="px-4 py-2 rounded-xl text-xs font-bold text-zinc-400 hover:text-white hover:bg-white/[0.06] border border-white/[0.1] transition cursor-pointer"
        >
          Batal
        </button>

        <button
          type="button"
          onclick={handleApplyMix}
          disabled={selectedCount === 0}
          class="px-5 py-2 rounded-xl text-xs font-black bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-black shadow-lg shadow-amber-500/20 transition active:scale-95 disabled:opacity-40 disabled:cursor-not-allowed flex items-center space-x-2 cursor-pointer"
        >
          <span>⚡ Konfirmasi Terapkan Mix ({selectedCount} Bagan)</span>
        </button>
      </div>
    </div>
  </div>
</div>
