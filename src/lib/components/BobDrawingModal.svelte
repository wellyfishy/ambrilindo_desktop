<script lang="ts">
  import { onMount } from "svelte";
  import { apiCall } from "$lib/api_client";
  import type { BobSourceCategory, BobBaganResult, CreateBobBaganPayload } from "$lib/types";

  interface Props {
    eventId: number;
    isOpen: boolean;
    onClose: () => void;
    onSuccess: (result: BobBaganResult) => void;
  }

  let { eventId, isOpen, onClose, onSuccess }: Props = $props();

  let candidates = $state<BobSourceCategory[]>([]);
  let isLoading = $state(true);
  let isSubmitting = $state(false);
  let errorMessage = $state<string | null>(null);

  let namaBob = $state("BEST OF THE BEST KUMITE SENIOR PUTRA");
  let selectedBaganIds = $state<number[]>([]);
  let shuffleType = $state<"kontingen" | "perguruan">("kontingen");
  let formatType = $state<"elimination" | "round_robin">("elimination");
  let searchQuery = $state("");

  let filteredCandidates = $derived(
    candidates.filter((c) => {
      const q = searchQuery.toLowerCase().trim();
      if (!q) return true;
      return (
        c.nama_bagan.toLowerCase().includes(q) ||
        c.kode_bagan.toLowerCase().includes(q) ||
        (c.juara_1_nama && c.juara_1_nama.toLowerCase().includes(q)) ||
        (c.juara_1_kontingen && c.juara_1_kontingen.toLowerCase().includes(q))
      );
    })
  );

  let eligibleCount = $derived(
    candidates.filter((c) => c.is_completed).length
  );

  let selectedEligibleCount = $derived(
    candidates.filter((c) => selectedBaganIds.includes(c.bagan_id) && c.is_completed).length
  );

  async function loadCandidates() {
    isLoading = true;
    errorMessage = null;
    try {
      const res = await apiCall<BobSourceCategory[]>("get_bob_candidates", {
        eventId,
        event_id: eventId,
      });
      candidates = res;
      // Pre-select all completed bagans
      selectedBaganIds = res.filter((c) => c.is_completed).map((c) => c.bagan_id);
    } catch (err: any) {
      console.error("Gagal memuat kandidat BOB:", err);
      errorMessage = err?.message || String(err);
    } finally {
      isLoading = false;
    }
  }

  function toggleBagan(id: number, isCompleted: boolean) {
    if (!isCompleted) return;
    if (selectedBaganIds.includes(id)) {
      selectedBaganIds = selectedBaganIds.filter((x) => x !== id);
    } else {
      selectedBaganIds = [...selectedBaganIds, id];
    }
  }

  function selectAllCompleted() {
    selectedBaganIds = candidates.filter((c) => c.is_completed).map((c) => c.bagan_id);
  }

  function clearSelection() {
    selectedBaganIds = [];
  }

  async function handleSubmit() {
    if (!namaBob.trim()) {
      alert("Nama bagan BOB tidak boleh kosong.");
      return;
    }
    if (selectedEligibleCount < 2) {
      alert("Pilih minimal 2 kategori dengan Juara 1 yang telah selesai.");
      return;
    }

    isSubmitting = true;
    errorMessage = null;

    try {
      const payload: CreateBobBaganPayload = {
        event_id: eventId,
        nama_bob: namaBob.trim(),
        source_bagan_ids: selectedBaganIds,
        shuffle_type: shuffleType,
        format_type: formatType,
      };

      const result = await apiCall<BobBaganResult>("create_bob_bagan", {
        input: payload,
        ...payload,
      });

      onSuccess(result);
      onClose();
    } catch (err: any) {
      console.error("Gagal membuat bagan BOB:", err);
      errorMessage = err?.message || String(err);
    } finally {
      isSubmitting = false;
    }
  }

  onMount(() => {
    if (isOpen) {
      loadCandidates();
    }
  });

  $effect(() => {
    if (isOpen) {
      loadCandidates();
    }
  });
</script>

{#if isOpen}
  <!-- Backdrop -->
  <div
    class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 overflow-y-auto"
    role="dialog"
    aria-modal="true"
  >
    <div
      class="bg-[#14161b] border border-amber-500/40 rounded-2xl w-full max-w-2xl overflow-hidden shadow-2xl flex flex-col max-h-[90vh] text-zinc-200"
    >
      <!-- Modal Header -->
      <div class="px-5 py-4 border-b border-white/[0.08] bg-[#181a20] flex items-center justify-between shrink-0">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-gradient-to-br from-amber-500 to-amber-600 text-slate-950 flex items-center justify-center text-lg font-black shadow-md">
            🏆
          </div>
          <div>
            <h3 class="text-sm font-black text-white uppercase tracking-tight">
              Drawing Bagan Best of the Best (BOB)
            </h3>
            <p class="text-[11px] text-zinc-400 font-medium">
              Himpun seluruh Juara 1 (Peraih Emas) ke dalam bagan kejuaraan bergengsi
            </p>
          </div>
        </div>

        <button
          type="button"
          onclick={onClose}
          class="w-7 h-7 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-zinc-400 hover:text-white flex items-center justify-center text-xs font-bold transition"
        >
          ✕
        </button>
      </div>

      <!-- Modal Body -->
      <div class="p-5 overflow-y-auto space-y-4 flex-1 scrollbar-thin">
        {#if errorMessage}
          <div class="p-3 rounded-lg bg-red-950/60 border border-red-500/50 text-red-300 text-xs flex items-center space-x-2">
            <span>⚠️</span>
            <span>{errorMessage}</span>
          </div>
        {/if}

        <!-- Judul Bagan BOB -->
        <div>
          <label for="bob-title-input" class="block text-xs font-bold text-zinc-300 mb-1.5 uppercase tracking-wide">
            Judul / Nama Bagan BOB
          </label>
          <input
            id="bob-title-input"
            type="text"
            bind:value={namaBob}
            placeholder="Contoh: BEST OF THE BEST KUMITE SENIOR PUTRA"
            class="w-full bg-[#181a20] text-white text-xs font-semibold px-3 py-2 rounded-xl border border-white/[0.1] focus:outline-none focus:border-amber-500 transition"
          />
        </div>

        <!-- Format & Shuffle Options -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div>
            <label for="format-select" class="block text-[11px] font-bold text-zinc-400 mb-1 uppercase tracking-wide">
              Format Pertandingan
            </label>
            <select
              id="format-select"
              bind:value={formatType}
              class="w-full bg-[#181a20] text-white text-xs font-medium px-3 py-2 rounded-xl border border-white/[0.1] focus:outline-none focus:border-amber-500"
            >
              <option value="elimination">Sistem Gugur (Single Elimination WKF)</option>
              <option value="round_robin">Setengah Kompetisi (Round-Robin Berger)</option>
            </select>
          </div>

          <div>
            <label for="shuffle-select" class="block text-[11px] font-bold text-zinc-400 mb-1 uppercase tracking-wide">
              Pemisahan Lawan (Shuffle)
            </label>
            <select
              id="shuffle-select"
              bind:value={shuffleType}
              class="w-full bg-[#181a20] text-white text-xs font-medium px-3 py-2 rounded-xl border border-white/[0.1] focus:outline-none focus:border-amber-500"
            >
              <option value="kontingen">Berdasarkan Kontingen / Utusan</option>
              <option value="perguruan">Berdasarkan Perguruan / Aliran</option>
            </select>
          </div>
        </div>

        <!-- Source Category Picker -->
        <div>
          <div class="flex items-center justify-between mb-2">
            <div>
              <span class="text-xs font-bold text-zinc-300 uppercase tracking-wide">Pilih Bagan Sumber</span>
              <span class="text-[11px] text-zinc-400 ml-1">
                ({selectedEligibleCount} dipilih dari {eligibleCount} juara tersedia)
              </span>
            </div>

            <div class="flex items-center space-x-1.5 text-[10px]">
              <button
                type="button"
                onclick={selectAllCompleted}
                class="px-2 py-0.5 rounded bg-white/[0.06] hover:bg-white/[0.1] text-amber-300 font-semibold transition"
              >
                Pilih Semua Juara 1
              </button>
              <button
                type="button"
                onclick={clearSelection}
                class="px-2 py-0.5 rounded bg-white/[0.06] hover:bg-white/[0.1] text-zinc-400 font-semibold transition"
              >
                Reset
              </button>
            </div>
          </div>

          <!-- Search Filter -->
          <div class="relative mb-2">
            <input
              type="text"
              bind:value={searchQuery}
              placeholder="Cari kelas tanding atau nama peraih emas..."
              class="w-full bg-[#181a20] text-white text-xs px-3 py-1.5 pl-8 rounded-lg border border-white/[0.08] focus:outline-none focus:border-amber-500 placeholder-zinc-500"
            />
            <span class="absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-500 text-xs">🔍</span>
          </div>

          <!-- Candidates List -->
          <div class="border border-white/[0.08] rounded-xl overflow-hidden bg-[#101216] max-h-56 overflow-y-auto divide-y divide-white/[0.04] scrollbar-thin">
            {#if isLoading}
              <div class="p-6 text-center text-xs text-zinc-500">
                Memuat kategori dan hasil final...
              </div>
            {:else if filteredCandidates.length === 0}
              <div class="p-6 text-center text-xs text-zinc-500">
                Tidak ada bagan yang cocok dengan pencarian.
              </div>
            {:else}
              {#each filteredCandidates as cat (cat.bagan_id)}
                {@const isSelected = selectedBaganIds.includes(cat.bagan_id)}
                {@const isCompleted = cat.is_completed}
                <div
                  role="button"
                  tabindex="0"
                  onclick={() => toggleBagan(cat.bagan_id, isCompleted)}
                  onkeydown={(e) => { if (e.key === 'Enter') toggleBagan(cat.bagan_id, isCompleted); }}
                  class="p-2.5 flex items-center justify-between gap-3 text-xs cursor-pointer select-none transition {isSelected && isCompleted ? 'bg-amber-500/10' : (!isCompleted ? 'opacity-40 cursor-not-allowed bg-black/20' : 'hover:bg-white/[0.02]')}"
                >
                  <div class="flex items-center space-x-2.5 min-w-0">
                    <input
                      type="checkbox"
                      checked={isSelected && isCompleted}
                      disabled={!isCompleted}
                      class="rounded border-zinc-700 text-amber-500 focus:ring-amber-500/30"
                    />

                    <span class="font-mono text-[10px] font-bold px-1.5 py-0.2 rounded bg-white/[0.06] text-zinc-300 shrink-0">
                      {cat.kode_bagan}
                    </span>

                    <div class="min-w-0">
                      <div class="font-bold text-white truncate text-xs">
                        {cat.nama_bagan}
                      </div>
                      {#if isCompleted}
                        <div class="text-[11px] text-amber-300 font-semibold truncate flex items-center space-x-1.5 mt-0.5">
                          <span>🥇 {cat.juara_1_nama}</span>
                          <span class="text-zinc-500">•</span>
                          <span class="text-zinc-400 font-normal">{cat.juara_1_kontingen || cat.juara_1_perguruan || '-'}</span>
                        </div>
                      {:else}
                        <div class="text-[10px] text-zinc-500 italic mt-0.5">
                          Pertandingan final belum selesai / belum ada Juara 1
                        </div>
                      {/if}
                    </div>
                  </div>

                  <div class="shrink-0">
                    {#if isCompleted}
                      <span class="px-2 py-0.5 rounded text-[9.5px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40">
                        Emas Ada
                      </span>
                    {:else}
                      <span class="px-2 py-0.5 rounded text-[9.5px] font-medium bg-zinc-800 text-zinc-500">
                        Pending
                      </span>
                    {/if}
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="px-5 py-3 border-t border-white/[0.08] bg-[#181a20] flex items-center justify-between shrink-0">
        <div class="text-xs text-zinc-400">
          <span class="font-bold text-white">{selectedEligibleCount}</span> atlet juara terpilih
        </div>

        <div class="flex items-center space-x-2">
          <button
            type="button"
            onclick={onClose}
            disabled={isSubmitting}
            class="px-3 py-1.5 text-xs font-semibold rounded-lg text-zinc-300 hover:text-white hover:bg-white/[0.05] transition"
          >
            Batal
          </button>

          <button
            type="button"
            onclick={handleSubmit}
            disabled={isSubmitting || selectedEligibleCount < 2 || !namaBob.trim()}
            class="px-4 py-1.5 text-xs font-bold rounded-lg bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-slate-950 shadow-md active:scale-95 disabled:opacity-40 transition flex items-center space-x-1.5"
          >
            <span>🏆</span>
            <span>{isSubmitting ? 'Membuat Bagan...' : 'Buat Bagan BOB'}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
