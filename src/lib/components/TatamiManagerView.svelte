<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "$lib/api_client";
  import { listen } from "@tauri-apps/api/event";
  import { initLiveSync, subscribeLiveSync } from "$lib/liveSync";
  import type { TatamiItem } from "$lib/types";

  interface Props {
    eventId: number;
    tatamis: TatamiItem[];
    hostAddress: string;
    onRefreshTatamis: () => void;
    onShowToast?: (msg: string) => void;
  }

  let {
    eventId,
    tatamis,
    hostAddress,
    onRefreshTatamis,
    onShowToast,
  }: Props = $props();

  // Card menu (rename / delete) open for this tatami id
  let menuFor = $state<number | null>(null);

  // Filter & Search State
  let searchKeyword = $state("");
  let selectedStatusFilter = $state<"all" | "live" | "ready">("all");
  let selectedSort = $state<"tatami_asc" | "tatami_desc" | "live_first">("tatami_asc");

  // Pagination State
  let currentPage = $state(1);
  let pageSize = $state(50);

  // Modals State
  let isAddingModal = $state(false);
  let newTatamiNumber = $state<number>(1);
  let isSubmitting = $state(false);
  let addError = $state<string | null>(null);

  let editModalTatami = $state<TatamiItem | null>(null);
  let editTatamiNumber = $state<number>(1);
  let isEditing = $state(false);
  let editError = $state<string | null>(null);

  let tatamiToDelete = $state<TatamiItem | null>(null);
  let isDeleting = $state(false);

  // QR Code Modal State
  let qrModalTatami = $state<TatamiItem | null>(null);
  let qrRole = $state<"jury" | "lo-kata" | "operator" | "coach-supervisor" | "wasit-vr">("jury");
  let qrSvgContent = $state<string>("");
  let isLoadingQr = $state(false);
  let availableIps = $state<Array<{ name: string; ip: string; is_default: boolean }>>([]);
  let currentQrHost = $state<string>("");

  // Toast Notification State
  let toastMsg = $state<string | null>(null);
  let toastTimer: any = null;
  let syncInterval: any = null;

  function showToast(msg: string) {
    toastMsg = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMsg = null;
    }, 3500);
    onShowToast?.(msg);
  }

  let effectiveTatamis = $derived(tatamis ?? []);

  // KPI Metrics Calculation
  let totalTatamis = $derived(effectiveTatamis.length);
  let activeMatchesCount = $derived(effectiveTatamis.filter((t) => t.detail_bagan_id !== null).length);
  let readyCount = $derived(Math.max(0, totalTatamis - activeMatchesCount));

  // Filtered and Sorted Tatamis
  let filteredTatamis = $derived.by(() => {
    let result = [...effectiveTatamis];

    // Status Filter
    if (selectedStatusFilter === "live") {
      result = result.filter((t) => t.detail_bagan_id !== null);
    } else if (selectedStatusFilter === "ready") {
      result = result.filter((t) => t.detail_bagan_id === null);
    }

    // Search Keyword
    if (searchKeyword.trim()) {
      const q = searchKeyword.toLowerCase().trim();
      result = result.filter((t) => {
        const tNumStr = (t.tatami_number ?? "").toString();
        const baganStr = t.nama_bagan || (t.detail_bagan_id ? `partai ${t.detail_bagan_id} #${t.detail_bagan_id}` : "");
        return (
          tNumStr.includes(q) ||
          `tatami ${tNumStr}`.includes(q) ||
          baganStr.toLowerCase().includes(q)
        );
      });
    }

    // Sorting
    if (selectedSort === "tatami_asc") {
      result.sort((a, b) => (a.tatami_number ?? 0) - (b.tatami_number ?? 0));
    } else if (selectedSort === "tatami_desc") {
      result.sort((a, b) => (b.tatami_number ?? 0) - (a.tatami_number ?? 0));
    } else if (selectedSort === "live_first") {
      result.sort((a, b) => {
        const aLive = a.detail_bagan_id !== null ? 1 : 0;
        const bLive = b.detail_bagan_id !== null ? 1 : 0;
        if (bLive !== aLive) return bLive - aLive;
        return (a.tatami_number ?? 0) - (b.tatami_number ?? 0);
      });
    }

    return result;
  });

  // Pagination
  let totalPages = $derived(Math.max(1, Math.ceil(filteredTatamis.length / pageSize)));
  let paginatedTatamis = $derived(
    filteredTatamis.slice((currentPage - 1) * pageSize, currentPage * pageSize)
  );

  // Create Tatami Modal
  function openAddModal() {
    const maxNum = effectiveTatamis.length > 0 ? Math.max(...effectiveTatamis.map((t) => t.tatami_number || 0)) : 0;
    newTatamiNumber = maxNum + 1;
    addError = null;
    isAddingModal = true;
  }

  async function handleAddTatami() {
    if (!eventId || newTatamiNumber < 1) return;
    isSubmitting = true;
    addError = null;
    try {
      await invoke("create_tatami", {
        eventId,
        tatamiNumber: newTatamiNumber,
      });

      isAddingModal = false;
      onRefreshTatamis();
      showToast(`Tatami ${newTatamiNumber} berhasil ditambahkan!`);
    } catch (err: any) {
      addError = typeof err === "string" ? err : err?.message || "Gagal menambahkan tatami.";
    } finally {
      isSubmitting = false;
    }
  }

  // Edit Tatami Modal
  function openEditModal(tatami: TatamiItem) {
    editModalTatami = tatami;
    editTatamiNumber = tatami.tatami_number ?? 1;
    editError = null;
    isEditing = false;
  }

  async function handleSaveEditTatami() {
    if (!editModalTatami || editTatamiNumber < 1) return;
    isEditing = true;
    editError = null;
    try {
      await invoke("update_tatami_number", {
        tatamiId: editModalTatami.id,
        newNumber: editTatamiNumber,
      });

      const updatedNum = editTatamiNumber;
      editModalTatami = null;
      onRefreshTatamis();
      showToast(`Nomor tatami berhasil diperbarui ke Tatami ${updatedNum}!`);
    } catch (err: any) {
      editError = typeof err === "string" ? err : err?.message || "Gagal mengubah nomor tatami.";
    } finally {
      isEditing = false;
    }
  }

  // Single Delete Tatami
  function promptDeleteTatami(tatami: TatamiItem) {
    tatamiToDelete = tatami;
  }

  async function confirmDeleteTatami() {
    if (!tatamiToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_tatami", { tatamiId: tatamiToDelete.id });

      const deletedNum = tatamiToDelete.tatami_number;
      tatamiToDelete = null;
      onRefreshTatamis();
      showToast(`Tatami ${deletedNum} berhasil dihapus.`);
    } catch (err: any) {
      showToast(`Gagal menghapus tatami: ${err?.message || err}`);
    } finally {
      isDeleting = false;
    }
  }

  // QR Code Access Modal
  async function openQrModal(tatami: TatamiItem, defaultRole: "jury" | "lo-kata" | "operator" | "coach-supervisor" | "wasit-vr" = "jury") {
    qrModalTatami = tatami;
    qrRole = defaultRole;
    if (!currentQrHost) {
      currentQrHost = hostAddress;
    }
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        availableIps = await invoke("get_available_ips");
      }
    } catch (_) {}
    await updateQrCode();
  }

  async function updateQrCode() {
    if (!qrModalTatami) return;
    const tNum = qrModalTatami.tatami_number ?? 1;
    const baseHost = currentQrHost || hostAddress;
    let targetUrl = `${baseHost}/jury/${tNum}/`;
    if (qrRole === "lo-kata") {
      targetUrl = `${baseHost}/lo-kata/${tNum}/`;
    } else if (qrRole === "coach-supervisor") {
      targetUrl = `${baseHost}/coach-supervisor/${tNum}/`;
    } else if (qrRole === "wasit-vr") {
      targetUrl = `${baseHost}/wasit-vr/${tNum}/`;
    } else if (qrRole === "operator") {
      targetUrl = `${baseHost}/control-panel?tatami=${tNum}`;
    }

    isLoadingQr = true;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        qrSvgContent = await invoke<string>("generate_qr_svg", { url: targetUrl });
      } else {
        qrSvgContent = "";
      }
    } catch (_) {
      qrSvgContent = "";
    } finally {
      isLoadingQr = false;
    }
  }

  function copyTargetUrl() {
    if (!qrModalTatami) return;
    const tNum = qrModalTatami.tatami_number ?? 1;
    const baseHost = currentQrHost || hostAddress;
    let targetUrl = `${baseHost}/jury/${tNum}/`;
    let label = "Panel Juri";
    if (qrRole === "lo-kata") {
      targetUrl = `${baseHost}/lo-kata/${tNum}/`;
      label = "LO / Guide";
    } else if (qrRole === "coach-supervisor") {
      targetUrl = `${baseHost}/coach-supervisor/${tNum}/`;
      label = "Coach Supervisor (VR)";
    } else if (qrRole === "wasit-vr") {
      targetUrl = `${baseHost}/wasit-vr/${tNum}/`;
      label = "Meja Wasit VR";
    } else if (qrRole === "operator") {
      targetUrl = `${baseHost}/control-panel?tatami=${tNum}`;
      label = "Operator Meja";
    }
    navigator.clipboard.writeText(targetUrl);
    showToast(`URL ${label} Tatami ${tNum} disalin!`);
  }

  let unlistenLiveState: any = null;
  let liveBroadcastChannel: BroadcastChannel | null = null;

  onMount(() => {
    // Initial fetch
    onRefreshTatamis();

    initLiveSync(hostAddress);
    const unsubLive = subscribeLiveSync(() => {
      onRefreshTatamis();
    });

    // Instant sync on tatami state events
    listen("tatami_live_state", () => {
      onRefreshTatamis();
    }).then((u) => {
      unlistenLiveState = u;
    }).catch(() => {});

    try {
      if (typeof window !== "undefined" && window.BroadcastChannel) {
        liveBroadcastChannel = new BroadcastChannel("ambrilindo_live_sync");
        liveBroadcastChannel.onmessage = () => {
          onRefreshTatamis();
        };
      }
    } catch (_) {}

    // Real-time synchronization: Auto-refresh tatami state every 3 seconds
    syncInterval = setInterval(() => {
      onRefreshTatamis();
    }, 3000);

    return () => {
      unsubLive();
      if (unlistenLiveState) unlistenLiveState();
      if (liveBroadcastChannel) liveBroadcastChannel.close();
    };
  });

  onDestroy(() => {
    if (syncInterval) clearInterval(syncInterval);
    if (toastTimer) clearTimeout(toastTimer);
  });
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Toast Notification Overlay -->
  {#if toastMsg}
    <div class="absolute top-4 right-6 z-50 animate-in fade-in slide-in-from-top-2 duration-150">
      <div class="bg-white text-black font-semibold text-xs px-4 py-2.5 rounded-xl shadow-2xl flex items-center space-x-2 border border-zinc-200">
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
        <span>{toastMsg}</span>
      </div>
    </div>
  {/if}

  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-4 pb-4 border-b border-white/[0.08] shrink-0">
    <div>
      <div class="flex items-center gap-2.5">
        <h2 class="text-xl font-black text-white tracking-tight uppercase">Tatami</h2>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">{totalTatamis}</span>
      </div>
      <p class="text-sm text-zinc-400 mt-1" role="status">
        {#if totalTatamis === 0}
          Belum ada tatami
        {:else}
          <span class={activeMatchesCount > 0 ? "text-amber-300 font-semibold" : ""}>{activeMatchesCount} sedang tanding</span>
          <span class="text-zinc-600"> · </span>
          <span>{readyCount} siap</span>
        {/if}
      </p>
    </div>

    <button
      onclick={openAddModal}
      class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors active:scale-[0.98] flex items-center gap-2 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:ring-offset-2 focus-visible:ring-offset-[#14161b]"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" /></svg>
      <span>Tambah tatami</span>
    </button>
  </div>

  <!-- Filter pills only when the venue has many tatami -->
  {#if totalTatamis > 8}
    <div class="flex flex-wrap items-center gap-1.5 mt-4 shrink-0" role="radiogroup" aria-label="Filter status tatami">
      {#each [["all", `Semua (${totalTatamis})`], ["live", `Sedang tanding (${activeMatchesCount})`], ["ready", `Siap (${readyCount})`]] as [value, label]}
        <button
          type="button"
          role="radio"
          aria-checked={selectedStatusFilter === value}
          onclick={() => (selectedStatusFilter = value as "all" | "live" | "ready")}
          class="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors border cursor-pointer {selectedStatusFilter === value
            ? 'bg-white text-black border-white'
            : 'bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white'}"
        >
          {label}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Tatami cards -->
  <div class="flex-1 min-h-0 overflow-y-auto mt-4 -mx-1 px-1 pb-2">
    {#if filteredTatamis.length === 0}
      <div class="h-full min-h-[240px] flex flex-col items-center justify-center text-center gap-3 rounded-2xl border border-dashed border-white/[0.12] bg-[#181a20]/50 p-8">
        <svg class="w-10 h-10 text-zinc-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z" /></svg>
        {#if totalTatamis === 0}
          <p class="text-sm font-semibold text-zinc-300">Tambahkan tatami pertama</p>
          <button
            onclick={openAddModal}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md cursor-pointer"
          >
            Tambah tatami
          </button>
        {:else}
          <p class="text-sm text-zinc-400">Tidak ada tatami dengan status ini</p>
        {/if}
      </div>
    {:else}
      <div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));">
        {#each filteredTatamis as tatami (tatami.id)}
          {@const tNum = tatami.tatami_number ?? 1}
          {@const live = tatami.detail_bagan_id !== null && tatami.detail_bagan_id !== undefined}
          <article
            aria-label={`Tatami ${tNum}`}
            class="relative rounded-2xl bg-[#181a20] border p-4 flex flex-col gap-4 shadow-lg {live ? 'border-amber-500/35' : 'border-white/[0.08]'}"
          >
            <div class="flex items-start gap-3">
              <div class="w-12 h-12 rounded-xl bg-white/[0.05] border border-white/[0.1] flex items-center justify-center font-mono font-black text-2xl text-white tabular-nums shrink-0">
                {tNum}
              </div>
              <div class="flex-1 min-w-0">
                {#if live}
                  <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs font-semibold bg-amber-500/15 text-amber-300 border border-amber-500/30">
                    <svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>
                    Sedang tanding
                  </span>
                  <p class="mt-1.5 text-sm text-white font-semibold truncate" title={tatami.nama_bagan || ""}>
                    {tatami.nama_bagan || "Partai berjalan"}
                  </p>
                  <p class="text-xs text-zinc-400 font-mono tabular-nums">Partai #{tatami.detail_bagan_id}</p>
                {:else}
                  <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs font-semibold bg-emerald-500/10 text-emerald-300 border border-emerald-500/25">
                    <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                    Siap
                  </span>
                  <p class="mt-1.5 text-sm text-zinc-500">Belum ada partai berjalan</p>
                {/if}
              </div>

              <!-- Card menu (rename / delete) -->
              <div class="relative shrink-0">
                <button
                  type="button"
                  onclick={() => (menuFor = menuFor === tatami.id ? null : tatami.id)}
                  aria-haspopup="menu"
                  aria-expanded={menuFor === tatami.id}
                  aria-label={`Menu tatami ${tNum}`}
                  class="w-8 h-8 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                >
                  <svg class="w-4 h-4" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="12" cy="5" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="12" cy="19" r="1.8" /></svg>
                </button>
                {#if menuFor === tatami.id}
                  <button type="button" class="fixed inset-0 z-30 cursor-default" tabindex="-1" aria-label="Tutup menu" onclick={() => (menuFor = null)}></button>
                  <div role="menu" class="absolute right-0 top-9 z-40 w-44 bg-[#14161b] border border-white/[0.14] rounded-xl shadow-2xl p-1">
                    <button
                      type="button"
                      role="menuitem"
                      onclick={() => {
                        menuFor = null;
                        openEditModal(tatami);
                      }}
                      class="w-full text-left px-3 py-2 rounded-lg text-sm text-zinc-200 hover:bg-white/[0.06] hover:text-white transition cursor-pointer"
                    >
                      Ubah nomor
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onclick={() => {
                        menuFor = null;
                        promptDeleteTatami(tatami);
                      }}
                      class="w-full text-left px-3 py-2 rounded-lg text-sm text-rose-300 hover:bg-rose-500/10 transition cursor-pointer"
                    >
                      Hapus tatami
                    </button>
                  </div>
                {/if}
              </div>
            </div>

            <!-- Venue access -->
            <div class="flex flex-wrap items-center gap-2 pt-3 border-t border-white/[0.06]">
              <a
                href="{hostAddress}/scoreboard?tatami={tNum}"
                target="_blank"
                class="h-9 px-3 rounded-lg bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 text-xs font-semibold transition-colors flex items-center gap-1.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-300/50"
                title={`Buka scoreboard tatami ${tNum} di browser`}
              >
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M3 5h18v12H3zM8 21h8M12 17v4" /></svg>
                Scoreboard
              </a>
              <a
                href="{hostAddress}/tatami-{tNum}/"
                target="_blank"
                class="h-9 px-3 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white border border-white/[0.1] text-xs font-semibold transition-colors flex items-center focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                title={`Buka panel operator tatami ${tNum} di browser`}
              >
                Operator
              </a>
              <a
                href="{hostAddress}/jury/{tNum}/"
                target="_blank"
                class="h-9 px-3 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white border border-white/[0.1] text-xs font-semibold transition-colors flex items-center focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                title={`Buka panel juri tatami ${tNum} di browser`}
              >
                Juri
              </a>
              <button
                onclick={() => openQrModal(tatami, "jury")}
                class="h-9 w-9 ml-auto rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.1] transition flex items-center justify-center cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                title="QR untuk tablet juri dan operator"
                aria-label={`QR tablet tatami ${tNum}`}
              >
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h2v2h-2zM18 18h2v2h-2zM14 18h2v2h-2zM18 14h2v2h-2z" /></svg>
              </button>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Modal: Tambah Tatami Baru -->
  {#if isAddingModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-3">
          <h3 class="text-sm font-bold text-white tracking-tight">Tambah Tatami Baru</h3>
          <button
            onclick={() => (isAddingModal = false)}
            class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        {#if addError}
          <div class="p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs">
            {addError}
          </div>
        {/if}

        <div class="space-y-1.5">
          <label for="tatami-num-input" class="text-xs font-semibold text-zinc-300 block">Nomor Tatami</label>
          <input
            id="tatami-num-input"
            type="number"
            min="1"
            bind:value={newTatamiNumber}
            class="w-full bg-[#111215] border border-white/[0.1] text-white font-mono text-sm px-3 py-2 rounded-xl outline-none focus:border-white transition-colors"
          />
        </div>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={() => (isAddingModal = false)}
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
          >
            Batal
          </button>
          <button
            onclick={handleAddTatami}
            disabled={isSubmitting}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors disabled:opacity-50 cursor-pointer"
          >
            {isSubmitting ? "Menyimpan..." : "Simpan Tatami"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: Ubah Nomor Tatami -->
  {#if editModalTatami}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-3">
          <h3 class="text-sm font-bold text-white tracking-tight">Ubah Nomor Tatami</h3>
          <button
            onclick={() => (editModalTatami = null)}
            class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        {#if editError}
          <div class="p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs">
            {editError}
          </div>
        {/if}

        <div class="space-y-1.5">
          <label for="edit-tatami-num-input" class="text-xs font-semibold text-zinc-300 block">Nomor Tatami Baru</label>
          <input
            id="edit-tatami-num-input"
            type="number"
            min="1"
            bind:value={editTatamiNumber}
            class="w-full bg-[#111215] border border-white/[0.1] text-white font-mono text-sm px-3 py-2 rounded-xl outline-none focus:border-white transition-colors"
          />
        </div>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={() => (editModalTatami = null)}
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
          >
            Batal
          </button>
          <button
            onclick={handleSaveEditTatami}
            disabled={isEditing}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors disabled:opacity-50 cursor-pointer"
          >
            {isEditing ? "Menyimpan..." : "Simpan Perubahan"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: Konfirmasi Hapus Single Tatami -->
  {#if tatamiToDelete}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
            </svg>
          </div>
          <div>
            <h3 class="text-sm font-bold text-white tracking-tight">Hapus Tatami {tatamiToDelete.tatami_number}</h3>
          </div>
        </div>

        <p class="text-xs text-zinc-300 leading-relaxed">
          Apakah Anda yakin ingin menghapus <strong>Tatami {tatamiToDelete.tatami_number}</strong> dari turnamen ini?
        </p>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={() => (tatamiToDelete = null)}
            disabled={isDeleting}
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
          >
            Batal
          </button>
          <button
            onclick={confirmDeleteTatami}
            disabled={isDeleting}
            class="h-9 px-4 bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs rounded-xl shadow-md transition-colors disabled:opacity-50 flex items-center space-x-1.5 cursor-pointer"
          >
            <span>{isDeleting ? "Menghapus..." : "Hapus Tatami"}</span>
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: QR Code Akses Tatami Mobile -->
  {#if qrModalTatami}
    {@const tNum = qrModalTatami.tatami_number ?? 1}
    {@const baseHost = currentQrHost || hostAddress}
    {@const targetUrl = qrRole === "jury" ? `${baseHost}/jury/${tNum}/` : (qrRole === "lo-kata" ? `${baseHost}/lo-kata/${tNum}/` : `${baseHost}/control-panel?tatami=${tNum}`)}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs select-none">
      <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-sm w-full space-y-4 animate-in zoom-in-95 duration-150 text-center">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-2.5 text-left">
          <h3 class="text-sm font-bold text-white tracking-tight">Akses Mobile Tatami {tNum}</h3>
          <button
            onclick={() => (qrModalTatami = null)}
            class="text-zinc-500 hover:text-white transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        <!-- Role Toggle Tabs -->
        <div class="grid grid-cols-3 gap-1 p-1 bg-[#111215] rounded-xl border border-white/[0.08]">
          <button
            onclick={() => {
              qrRole = "jury";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'jury'
              ? 'bg-white text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Panel Juri
          </button>
          <button
            onclick={() => {
              qrRole = "lo-kata";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'lo-kata'
              ? 'bg-white text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            LO / Guide
          </button>
          <button
            onclick={() => {
              qrRole = "operator";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'operator'
              ? 'bg-white text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Meja Tatami
          </button>
          <button
            onclick={() => {
              qrRole = "coach-supervisor";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer {qrRole === 'coach-supervisor'
              ? 'bg-red-500 text-white shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Coach VR
          </button>
          <button
            onclick={() => {
              qrRole = "wasit-vr";
              updateQrCode();
            }}
            class="py-1.5 rounded-lg text-xs font-semibold transition-colors col-span-2 cursor-pointer {qrRole === 'wasit-vr'
              ? 'bg-amber-500 text-black shadow-xs font-bold'
              : 'text-zinc-400 hover:text-white'}"
          >
            Meja Wasit VR
          </button>
        </div>

        <!-- IP Venue Network Selector if multiple interfaces available -->
        {#if availableIps.length > 1}
          <div class="flex items-center justify-between gap-2 px-3 py-1.5 bg-[#111215] rounded-xl border border-white/[0.08] text-[11px] text-left">
            <span class="text-zinc-500 font-mono font-bold uppercase text-[9px] shrink-0">IP Venue:</span>
            <select
              value={currentQrHost.replace(/^https?:\/\//, "").replace(/:\d+.*$/, "")}
              onchange={(e) => {
                const targetIp = (e.target as HTMLSelectElement).value;
                currentQrHost = `http://${targetIp}:8000`;
                updateQrCode();
              }}
              class="bg-transparent text-white font-mono font-bold outline-none cursor-pointer text-right flex-1 truncate"
            >
              {#each availableIps as item}
                <option value={item.ip} class="bg-[#181a20] text-white">
                  {item.ip} ({item.name})
                </option>
              {/each}
            </select>
          </div>
        {/if}

        <!-- QR Code Canvas Display -->
        {#if isLoadingQr}
          <div class="w-52 h-52 mx-auto flex items-center justify-center bg-white/[0.02] border border-white/[0.08] rounded-xl text-xs text-zinc-500 font-mono">
            Menghasilkan QR Code...
          </div>
        {:else if qrSvgContent}
          <div class="bg-white p-3 rounded-xl w-52 h-52 mx-auto flex items-center justify-center shadow-lg">
            {@html qrSvgContent}
          </div>
        {:else}
          <div class="w-52 h-52 mx-auto flex flex-col items-center justify-center bg-white/[0.02] border border-white/[0.08] rounded-xl p-4 space-y-2">
            <svg class="w-8 h-8 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4v1m6 11h2m-6 0h-2v4m0-11v3m0 0h.01M12 12h4.01M16 20h4M4 12h4m12 0h.01M5 8h2a1 1 0 001-1V5a1 1 0 00-1-1H5a1 1 0 00-1 1v2a1 1 0 001 1zm12 0h2a1 1 0 001-1V5a1 1 0 00-1-1h-2a1 1 0 00-1 1v2a1 1 0 001 1zM5 20h2a1 1 0 001-1v-2a1 1 0 00-1-1H5a1 1 0 00-1 1v2a1 1 0 001 1z" />
            </svg>
            <span class="text-xs text-zinc-400 font-mono break-all">{targetUrl}</span>
          </div>
        {/if}

        <!-- Target URL Box -->
        <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-2.5 text-left">
          <span class="text-[9px] font-mono font-bold text-zinc-500 uppercase tracking-wider block">ALAMAT URL VENUE</span>
          <p class="text-xs font-mono font-semibold text-white truncate mt-0.5 select-all">{targetUrl}</p>
        </div>

        <div class="flex items-center justify-between pt-2 border-t border-white/[0.08]">
          <a
            href={targetUrl}
            target="_blank"
            class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors flex items-center space-x-1.5"
          >
            <span>Buka di Tab</span>
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 6H5.25A2.25 2.25 0 0 0 3 8.25v10.5A2.25 2.25 0 0 0 5.25 21h10.5A2.25 2.25 0 0 0 18 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25" />
            </svg>
          </a>

          <div class="flex items-center space-x-2">
            <button
              onclick={copyTargetUrl}
              class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] hover:text-white transition-colors cursor-pointer"
            >
              Salin URL
            </button>
            <button
              onclick={() => (qrModalTatami = null)}
              class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors cursor-pointer"
            >
              Tutup
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>
