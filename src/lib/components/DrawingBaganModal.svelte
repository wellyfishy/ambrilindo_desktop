<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { DrawingCategoryInfo, DrawingSummary } from "$lib/types";
  import BobDrawingModal from "$lib/components/BobDrawingModal.svelte";

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

  // BOB Modal State
  let showBobModal = $state(false);

  // Selections
  let selectedCategoryIds = $state<number[]>([]);
  let vrCategoryIds = $state<number[]>([]);
  let shuffleType = $state<"both" | "kontingen" | "perguruan">("both");
  let searchQuery = $state("");

  // Delete all modal state
  let showDeleteConfirm = $state(false);
  let confirmDeleteChecked = $state(false);
  let isDeleting = $state(false);
  let isCreatingRoundRobin = $state<number | null>(null);

  // Result dialog
  let summaryResult = $state<DrawingSummary | null>(null);

  async function handleCreateRoundRobin(catId: number) {
    isCreatingRoundRobin = catId;
    executionError = null;
    try {
      await invoke("create_round_robin_bagan", { eventId, nomorTandingId: catId });
      await loadCategories();
      alert("Bagan Round-Robin berhasil dibuat!");
      onComplete();
    } catch (err: any) {
      executionError = typeof err === "string" ? err : err.message || "Gagal membuat bagan round-robin.";
    } finally {
      isCreatingRoundRobin = null;
    }
  }

  async function loadCategories() {
    isLoading = true;
    executionError = null;
    try {
      const data = await invoke<DrawingCategoryInfo[]>("get_drawing_eligible_categories", { eventId });
      categories = data;

      // Default selection: select all categories that have athletes
      selectedCategoryIds = data.filter((c) => c.athlete_count > 0).map((c) => c.id);

      // Default VR: preserve existing has_vr state for Kumite
      vrCategoryIds = data.filter((c) => c.is_kumite && c.has_vr).map((c) => c.id);
    } catch (err: any) {
      executionError = typeof err === "string" ? err : err.message || "Gagal memuat kategori drawing.";
    } finally {
      isLoading = false;
    }
  }

  // Filtered categories
  let filteredCategories = $derived.by(() => {
    if (!searchQuery.trim()) return categories;
    const q = searchQuery.toLowerCase();
    return categories.filter((c) => c.nama_nomor_tanding.toLowerCase().includes(q));
  });

  // Kumite categories in current selection for VR setup
  let kumiteSelected = $derived.by(() => {
    return categories.filter((c) => c.is_kumite && selectedCategoryIds.includes(c.id));
  });

  function toggleCategory(id: number) {
    if (selectedCategoryIds.includes(id)) {
      selectedCategoryIds = selectedCategoryIds.filter((x) => x !== id);
    } else {
      selectedCategoryIds = [...selectedCategoryIds, id];
    }
  }

  function selectAll() {
    selectedCategoryIds = categories.filter((c) => c.athlete_count > 0).map((c) => c.id);
  }

  function selectUndrawnOnly() {
    selectedCategoryIds = categories.filter((c) => c.athlete_count > 0 && c.bagan_count === 0).map((c) => c.id);
  }

  function clearCategorySelection() {
    selectedCategoryIds = [];
  }

  function toggleVrCategory(id: number) {
    if (vrCategoryIds.includes(id)) {
      vrCategoryIds = vrCategoryIds.filter((x) => x !== id);
    } else {
      vrCategoryIds = [...vrCategoryIds, id];
    }
  }

  function selectAllKumiteVr() {
    vrCategoryIds = kumiteSelected.map((c) => c.id);
  }

  function selectOfficialKumiteVr() {
    // Official classes: Kadet, Cadet, Junior, U-21, Under 21, Senior
    vrCategoryIds = kumiteSelected
      .filter((c) => /kadet|cadet|junior|u-?21|under|senior/i.test(c.nama_nomor_tanding))
      .map((c) => c.id);
  }

  function clearVr() {
    vrCategoryIds = [];
  }

  async function handleExecuteDrawing() {
    if (selectedCategoryIds.length === 0) {
      executionError = "Pilih minimal satu nomor tanding untuk diundi.";
      return;
    }

    isExecuting = true;
    executionError = null;

    try {
      const summary = await invoke<DrawingSummary>("execute_drawing_bagan", {
        eventId,
        categoryIds: selectedCategoryIds,
        shuffleType,
        vrCategoryIds,
      });
      summaryResult = summary;
    } catch (err: any) {
      executionError = typeof err === "string" ? err : err.message || "Gagal melakukan drawing bagan.";
    } finally {
      isExecuting = false;
    }
  }

  async function handleDeleteAllBagans() {
    if (!confirmDeleteChecked) return;
    isDeleting = true;
    executionError = null;
    try {
      await invoke("delete_all_bagans", { eventId });
      showDeleteConfirm = false;
      confirmDeleteChecked = false;
      await loadCategories();
      onComplete();
    } catch (err: any) {
      executionError = typeof err === "string" ? err : err.message || "Gagal menghapus seluruh bagan.";
    } finally {
      isDeleting = false;
    }
  }

  onMount(() => {
    loadCategories();
  });
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm select-none font-sans">
  <div class="bg-[#14161b] border border-white/[0.08] w-full max-w-4xl max-h-[92vh] rounded-2xl shadow-2xl flex flex-col overflow-hidden text-zinc-200">
    
    <!-- Modal Header -->
    <div class="px-6 py-4 border-b border-white/[0.08] flex items-center justify-between shrink-0 bg-[#181a20]">
      <div>
        <div class="flex items-center space-x-2.5">
          <span class="w-2 h-2 rounded-full bg-amber-400"></span>
          <h2 class="text-sm font-black tracking-wider uppercase text-white font-mono">
            DRAWING BAGAN OTOMATIS
          </h2>
        </div>
        <p class="text-xs text-zinc-400 mt-1 font-normal">
          Pengundian nomor tanding turnamen karate dengan optimasi anti-tabrakan kontingen / perguruan.
        </p>
      </div>

      <button
        onclick={onClose}
        disabled={isExecuting || isDeleting}
        aria-label="Tutup modal drawing"
        class="text-zinc-400 hover:text-white p-1.5 rounded-lg hover:bg-white/[0.06] transition disabled:opacity-50"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    <!-- Error Alert -->
    {#if executionError}
      <div class="mx-6 mt-4 p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-xs text-red-400 flex items-start space-x-2">
        <svg class="w-4 h-4 shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <span>{executionError}</span>
      </div>
    {/if}

    <!-- Result Modal (if finished) -->
    {#if summaryResult}
      <div class="p-8 flex flex-col items-center justify-center text-center space-y-4">
        <div class="w-12 h-12 rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
        </div>

        <div>
          <h3 class="text-base font-bold text-white uppercase tracking-wider font-mono">
            DRAWING SELESAI
          </h3>
          <p class="text-xs text-zinc-400 mt-1 max-w-md">
            Struktur pohon eliminasi dan rincian partai telah berhasil dibentuk dan disimpan ke database lokal.
          </p>
        </div>

        <!-- Metric Grid -->
        <div class="grid grid-cols-4 gap-3 w-full max-w-lg pt-2 font-mono">
          <div class="bg-[#181a20] border border-white/[0.08] p-3 rounded-xl text-center">
            <span class="text-lg font-bold text-white">{summaryResult.categories_processed}</span>
            <span class="block text-[10px] text-zinc-500 uppercase mt-0.5">Kategori</span>
          </div>
          <div class="bg-[#181a20] border border-white/[0.08] p-3 rounded-xl text-center">
            <span class="text-lg font-bold text-white">{summaryResult.bagans_created}</span>
            <span class="block text-[10px] text-zinc-500 uppercase mt-0.5">Bagan Dibuat</span>
          </div>
          <div class="bg-[#181a20] border border-white/[0.08] p-3 rounded-xl text-center">
            <span class="text-lg font-bold text-white">{summaryResult.total_matches}</span>
            <span class="block text-[10px] text-zinc-500 uppercase mt-0.5">Total Partai</span>
          </div>
          <div class="bg-[#181a20] border {summaryResult.total_collisions > 0 ? 'border-amber-500/30 bg-amber-500/5' : 'border-white/[0.08]'} p-3 rounded-xl text-center">
            <span class="text-lg font-bold {summaryResult.total_collisions > 0 ? 'text-amber-400' : 'text-emerald-400'}">
              {summaryResult.total_collisions}
            </span>
            <span class="block text-[10px] text-zinc-500 uppercase mt-0.5">Tabrakan R1</span>
          </div>
        </div>

        {#if summaryResult.total_collisions > 0}
          <div class="max-w-md p-3 bg-amber-500/10 border border-amber-500/20 rounded-xl text-[11px] text-amber-300 text-left">
            Perhatian: {summaryResult.total_collisions} atlet satu kontingen/perguruan tidak dapat dipisahkan di Babak 1 karena keterbatasan jumlah lawan. Anda dapat mengundi ulang atau menyesuaikan secara manual jika diperlukan.
          </div>
        {/if}

        <div class="pt-4 flex items-center space-x-3">
          <button
            onclick={() => {
              summaryResult = null;
              onComplete();
              onClose();
            }}
            class="px-6 py-2 bg-white text-black font-semibold text-xs rounded-xl hover:bg-zinc-200 transition font-mono"
          >
            TUTUP & LIHAT BAGAN
          </button>
        </div>
      </div>
    {:else}
      <!-- Main Modal Content -->
      <div class="flex-1 overflow-y-auto p-6 space-y-6 scrollbar-thin">
        
        <!-- SECTION 1: Pilih Nomor Tanding -->
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center space-x-2">
              <span class="text-xs font-mono font-bold text-zinc-300 uppercase tracking-wider">
                1. PILIH NOMOR TANDING
              </span>
              <span class="text-[11px] text-zinc-500">
                ({selectedCategoryIds.length} dari {categories.length} dipilih)
              </span>
            </div>

            <!-- Quick Selection Filters -->
            <div class="flex items-center space-x-1.5 font-mono text-[11px]">
              <button
                type="button"
                onclick={selectAll}
                class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.06] transition"
              >
                Pilih Semua
              </button>
              <button
                type="button"
                onclick={selectUndrawnOnly}
                class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.06] transition"
              >
                Belum Diundi
              </button>
              <button
                type="button"
                onclick={clearCategorySelection}
                class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 border border-white/[0.06] transition"
              >
                Kosongkan
              </button>
            </div>
          </div>

          <!-- Search Filter -->
          <div class="relative">
            <input
              type="text"
              bind:value={searchQuery}
              placeholder="Cari nama kategori pertandingan..."
              class="w-full bg-[#181a20] border border-white/[0.08] rounded-xl px-3.5 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-white/20 transition font-mono"
            />
          </div>

          <!-- Category List Cards -->
          <div class="max-h-56 overflow-y-auto space-y-1.5 pr-1 scrollbar-thin">
            {#if isLoading}
              <div class="py-8 text-center text-xs text-zinc-500">Memuat kategori turnamen...</div>
            {:else if filteredCategories.length === 0}
              <div class="py-8 text-center text-xs text-zinc-500">Tidak ada nomor tanding ditemukan.</div>
            {:else}
              {#each filteredCategories as cat}
                {@const isSelected = selectedCategoryIds.includes(cat.id)}
                <div
                  role="button"
                  tabindex="0"
                  onclick={() => toggleCategory(cat.id)}
                  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleCategory(cat.id); } }}
                  class="w-full flex items-center justify-between p-2.5 rounded-xl border transition text-left cursor-pointer {isSelected ? 'bg-white/[0.04] border-white/20' : 'bg-[#181a20]/40 border-white/[0.04] hover:border-white/[0.08]'}"
                >
                  <div class="flex items-center space-x-3 truncate">
                    <input
                      type="checkbox"
                      checked={isSelected}
                      class="w-4 h-4 rounded bg-[#181a20] border-white/20 text-blue-500 focus:ring-0 cursor-pointer pointer-events-none"
                    />
                    <div class="truncate">
                      <span class="text-xs font-semibold text-white tracking-tight truncate block">
                        {cat.nama_nomor_tanding}
                      </span>
                    </div>
                  </div>

                  <div class="flex items-center space-x-2 shrink-0 font-mono text-[10px]">
                    {#if cat.is_kumite}
                      <span class="px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20">
                        KUMITE
                      </span>
                    {:else}
                      <span class="px-1.5 py-0.5 rounded bg-sky-500/10 text-sky-300 border border-sky-500/20">
                        KATA
                      </span>
                    {/if}

                    {#if cat.is_beregu}
                      <span class="px-1.5 py-0.5 rounded bg-purple-500/10 text-purple-300 border border-purple-500/20">
                        REGU
                      </span>
                    {/if}

                    <span class="px-2 py-0.5 rounded bg-white/[0.04] text-zinc-300 border border-white/[0.06]">
                      {cat.athlete_count} ATLET
                    </span>

                    {#if cat.athlete_count >= 2 && cat.athlete_count <= 5 && cat.bagan_count === 0}
                      <button
                        type="button"
                        onclick={(e) => {
                          e.stopPropagation();
                          handleCreateRoundRobin(cat.id);
                        }}
                        disabled={isCreatingRoundRobin === cat.id}
                        class="px-2 py-0.5 rounded bg-indigo-500/20 hover:bg-indigo-500/35 text-indigo-300 border border-indigo-500/40 text-[10px] font-bold font-mono transition flex items-center gap-1 active:scale-95 disabled:opacity-50"
                        title="Buat bagan sistem kompetisi penuh Round-Robin untuk kategori kecil ini"
                      >
                        <span>{isCreatingRoundRobin === cat.id ? "Membuat..." : "+ Round-Robin"}</span>
                      </button>
                    {/if}

                    {#if cat.bagan_count > 0}
                      <span class="px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                        ADA {cat.bagan_count} BAGAN
                      </span>
                    {:else}
                      <span class="text-zinc-500">
                        BELUM
                      </span>
                    {/if}
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </div>

        <!-- SECTION 2: Konfigurasi Video Review (VR) -->
        <div class="p-4 bg-[#181a20] border border-white/[0.08] rounded-xl space-y-3">
          <div class="flex items-center justify-between">
            <div>
              <div class="flex items-center space-x-2">
                <span class="text-xs font-mono font-bold text-white uppercase tracking-wider">
                  2. VIDEO REVIEW (VR) KUMITE
                </span>
                <span class="text-[10px] text-zinc-500 font-mono">
                  ({vrCategoryIds.length} Aktif)
                </span>
              </div>
              <p class="text-[11px] text-zinc-400 mt-0.5 font-normal">
                Centang kelas Kumite yang menggunakan Video Review. Regenerasi otomatis di Semi Final &amp; Final.
              </p>
            </div>

            <!-- Quick VR buttons -->
            <div class="flex items-center space-x-1.5 font-mono text-[10px]">
              <button
                type="button"
                onclick={selectAllKumiteVr}
                class="px-2 py-1 rounded bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.06] transition"
              >
                Semua Kumite
              </button>
              <button
                type="button"
                onclick={selectOfficialKumiteVr}
                class="px-2 py-1 rounded bg-blue-500/10 hover:bg-blue-500/20 text-blue-300 border border-blue-500/30 transition"
              >
                Kadet/Junior/Senior
              </button>
              <button
                type="button"
                onclick={clearVr}
                class="px-2 py-1 rounded bg-red-500/10 hover:bg-red-500/20 text-red-300 border border-red-500/20 transition"
              >
                Kosongkan
              </button>
            </div>
          </div>

          <!-- Kumite selection badges -->
          <div class="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto pr-1 scrollbar-thin">
            {#if kumiteSelected.length === 0}
              <div class="py-2 text-[11px] text-zinc-500">Tidak ada kategori Kumite dalam daftar nomor tanding yang dipilih.</div>
            {:else}
              {#each kumiteSelected as cat}
                {@const isVr = vrCategoryIds.includes(cat.id)}
                <button
                  type="button"
                  onclick={() => toggleVrCategory(cat.id)}
                  class="px-2.5 py-1 rounded-lg text-[11px] font-mono border transition flex items-center space-x-1.5 {isVr ? 'bg-amber-500/15 text-amber-200 border-amber-500/40 font-bold' : 'bg-black/20 text-zinc-400 border-white/[0.06] hover:text-white'}"
                >
                  <span class="w-1.5 h-1.5 rounded-full {isVr ? 'bg-amber-400' : 'bg-zinc-600'}"></span>
                  <span class="truncate max-w-[200px]">{cat.nama_nomor_tanding}</span>
                </button>
              {/each}
            {/if}
          </div>
        </div>

        <!-- SECTION 3: Tipe Pemisahan (Shuffle Type) -->
        <div class="p-4 bg-[#181a20] border border-white/[0.08] rounded-xl space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-xs font-mono font-bold text-white uppercase tracking-wider block">
              3. METODE PEMISAHAN ANTI-TABRAKAN
            </span>
            <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 font-bold">
              STANDAR WKF / FORKI
            </span>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
            <label
              class="flex items-start space-x-3 p-3 rounded-xl border transition cursor-pointer {shuffleType === 'both' ? 'bg-blue-500/[0.08] border-blue-500/40 ring-1 ring-blue-500/20' : 'bg-black/20 border-white/[0.06] hover:bg-white/[0.02]'}"
            >
              <input
                type="radio"
                name="shuffleType"
                value="both"
                bind:group={shuffleType}
                class="mt-0.5 text-blue-500 focus:ring-0 cursor-pointer"
              />
              <div>
                <div class="flex items-center gap-1.5 flex-wrap">
                  <span class="text-xs font-bold text-white block">Kontingen &amp; Perguruan</span>
                  <span class="px-1.5 py-0.2 rounded text-[9px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
                    REKOMENDASI
                  </span>
                </div>
                <span class="text-[11px] text-zinc-400 block mt-1 leading-relaxed">
                  Pemisahan dua lapis: atlet satu kontingen dan satu perguruan dipisah ke pool &amp; paruh eliminasi yang berbeda.
                </span>
              </div>
            </label>

            <label
              class="flex items-start space-x-3 p-3 rounded-xl border transition cursor-pointer {shuffleType === 'kontingen' ? 'bg-white/[0.04] border-white/20' : 'bg-black/20 border-white/[0.06] hover:bg-white/[0.02]'}"
            >
              <input
                type="radio"
                name="shuffleType"
                value="kontingen"
                bind:group={shuffleType}
                class="mt-0.5 text-blue-500 focus:ring-0 cursor-pointer"
              />
              <div>
                <span class="text-xs font-bold text-white block">Pemisahan Kontingen</span>
                <span class="text-[11px] text-zinc-400 block mt-1 leading-relaxed">
                  Atlet satu kontingen/daerah pendaftaran dipisah sejauh mungkin di bagan.
                </span>
              </div>
            </label>

            <label
              class="flex items-start space-x-3 p-3 rounded-xl border transition cursor-pointer {shuffleType === 'perguruan' ? 'bg-white/[0.04] border-white/20' : 'bg-black/20 border-white/[0.06] hover:bg-white/[0.02]'}"
            >
              <input
                type="radio"
                name="shuffleType"
                value="perguruan"
                bind:group={shuffleType}
                class="mt-0.5 text-blue-500 focus:ring-0 cursor-pointer"
              />
              <div>
                <span class="text-xs font-bold text-white block">Pemisahan Perguruan</span>
                <span class="text-[11px] text-zinc-400 block mt-1 leading-relaxed">
                  Pemisahan berdasarkan perguruan FORKI (Inkai, KKI, Lemkari, dll).
                </span>
              </div>
            </label>
          </div>
        </div>

      </div>

      <!-- Modal Footer -->
      <div class="px-6 py-4 border-t border-white/[0.08] flex items-center justify-between shrink-0 bg-[#181a20]">
        <div class="flex items-center space-x-2">
          <!-- Delete All Brackets Button -->
          <button
            type="button"
            onclick={() => showDeleteConfirm = true}
            disabled={isExecuting || isDeleting}
            class="px-3.5 py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-400 border border-red-500/20 rounded-xl text-xs font-medium font-mono transition disabled:opacity-50"
          >
            HAPUS SEMUA BAGAN
          </button>

          <!-- Create BOB Bracket Button -->
          <button
            type="button"
            onclick={() => (showBobModal = true)}
            disabled={isExecuting || isDeleting}
            class="px-3.5 py-1.5 bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 border border-amber-500/30 rounded-xl text-xs font-bold font-mono transition flex items-center space-x-1.5 active:scale-95"
            title="Himpun para juara 1 (peraih emas) ke dalam bagan Best of the Best (BOB)"
          >
            <span>🏆</span>
            <span>BAGAN BOB</span>
          </button>
        </div>

        <div class="flex items-center space-x-3">
          <button
            type="button"
            onclick={onClose}
            disabled={isExecuting || isDeleting}
            class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-xs font-medium text-zinc-300 border border-white/[0.08] rounded-xl transition disabled:opacity-50 font-mono"
          >
            Batal
          </button>

          <button
            type="button"
            onclick={handleExecuteDrawing}
            disabled={isExecuting || isDeleting || selectedCategoryIds.length === 0}
            class="px-5 py-2 bg-amber-500 hover:bg-amber-400 text-black font-bold text-xs rounded-xl transition disabled:opacity-40 flex items-center space-x-2 font-mono shadow-md"
          >
            {#if isExecuting}
              <svg class="animate-spin h-3.5 w-3.5 text-black" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
              </svg>
              <span>MENGUNDI ({selectedCategoryIds.length})...</span>
            {:else}
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              <span>MULAI DRAWING ({selectedCategoryIds.length})</span>
            {/if}
          </button>
        </div>
      </div>
    {/if}

  </div>
</div>

<!-- Confirmation Submodal for Delete All Brackets -->
{#if showDeleteConfirm}
  <div class="fixed inset-0 z-60 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md font-sans">
    <div class="bg-[#14161b] border border-red-500/30 w-full max-w-md rounded-2xl shadow-2xl p-6 text-zinc-200 space-y-4">
      <div class="flex items-center space-x-3 text-red-400">
        <svg class="w-6 h-6 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <h3 class="text-sm font-black uppercase tracking-wider font-mono text-white">
          HAPUS SELURUH BAGAN?
        </h3>
      </div>

      <p class="text-xs text-zinc-400 leading-relaxed">
        Apakah Anda yakin ingin menghapus <strong>seluruh bagan pertandingan</strong> pada event ini? Seluruh rincian partai, skor, dan penetapan juara lokal akan dihapus secara permanen. Tindakan ini tidak dapat dibatalkan.
      </p>

      <label class="flex items-center space-x-2.5 p-3 rounded-xl bg-red-500/10 border border-red-500/20 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmDeleteChecked}
          class="w-4 h-4 rounded text-red-500 focus:ring-0"
        />
        <span class="text-xs text-red-300 font-medium">
          Saya yakin ingin menghapus seluruh bagan turnamen.
        </span>
      </label>

      <div class="flex items-center justify-end space-x-2.5 pt-2 font-mono">
        <button
          type="button"
          onclick={() => {
            showDeleteConfirm = false;
            confirmDeleteChecked = false;
          }}
          disabled={isDeleting}
          class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-xs text-zinc-300 rounded-xl transition"
        >
          Batal
        </button>

        <button
          type="button"
          onclick={handleDeleteAllBagans}
          disabled={!confirmDeleteChecked || isDeleting}
          class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white font-bold text-xs rounded-xl transition disabled:opacity-40 flex items-center space-x-1.5"
        >
          {#if isDeleting}
            <span>MENGHAPUS...</span>
          {:else}
            <span>YA, HAPUS SEMUA</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Best of Best (BOB) Drawing Modal -->
<BobDrawingModal
  {eventId}
  isOpen={showBobModal}
  onClose={() => (showBobModal = false)}
  onSuccess={(result) => {
    showBobModal = false;
    alert(`Bagan Best of the Best (BOB) '${result.nama_bob}' berhasil dibuat dengan ${result.athlete_count} juara!`);
    onComplete();
    onClose();
  }}
/>
