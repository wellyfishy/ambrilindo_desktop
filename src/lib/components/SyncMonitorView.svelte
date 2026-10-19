<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    EventItem,
    BaganItem,
    SyncConfig,
    EventMapping,
    HostedEventOption,
    PingResult,
    SyncQueueStats,
    SyncQueueEntry,
    PullResult,
  } from "$lib/types";

  interface Props {
    eventId: number;
    activeEvent: EventItem | null;
    onShowToast?: (msg: string) => void;
  }

  let { eventId, activeEvent, onShowToast }: Props = $props();

  // Core Sync State
  let syncConfig = $state<SyncConfig>({
    mode: "local",
    url: "http://127.0.0.1:8001",
    label: "Dev Lokal (http://127.0.0.1:8001/)",
  });
  let eventMapping = $state<EventMapping>({
    hosted_event_id: null,
    hosted_event_name: null,
    target_mode: "local",
    is_mismatch: false,
  });
  let queueStats = $state<SyncQueueStats>({
    total_count: 0,
    failed_count: 0,
    pending_count: 0,
    synced_count: 0,
  });
  let queueItems = $state<SyncQueueEntry[]>([]);
  let bagans = $state<BaganItem[]>([]);

  // Filter & Search
  let statusFilter = $state<string>("all");
  let searchKeyword = $state<string>("");
  let autoRefresh = $state<boolean>(true);
  let isRefreshing = $state<boolean>(false);
  let refreshTimer: any = null;

  // Action Spinners
  let isPinging = $state<boolean>(false);
  let pingResult = $state<PingResult | null>(null);
  let isTogglingLive = $state<boolean>(false);
  let isPulling = $state<boolean>(false);
  let isPushing = $state<boolean>(false);
  let isForceSyncing = $state<boolean>(false);
  let isRetryingAll = $state<boolean>(false);

  // Modals
  let showLinkModal = $state<boolean>(false);
  let hostedEvents = $state<HostedEventOption[]>([]);
  let isLoadingHostedEvents = $state<boolean>(false);
  let selectedHostedId = $state<number | null>(null);
  let selectedHostedName = $state<string>("");

  let showPushBaganModal = $state<boolean>(false);
  let pushScope = $state<"all" | "selected">("all");
  let selectedBaganIds = $state<number[]>([]);

  let showForceSyncModal = $state<boolean>(false);
  let showClearAllModal = $state<boolean>(false);
  let showPayloadModal = $state<boolean>(false);
  let selectedPayloadJson = $state<string>("");
  let selectedPayloadId = $state<number | null>(null);
  let showCustomUrlModal = $state<boolean>(false);
  let customUrlInput = $state<string>("");

  function toast(msg: string) {
    if (onShowToast) onShowToast(msg);
  }

  async function loadAllSyncData() {
    if (!eventId) return;
    try {
      isRefreshing = true;
      const cfg = await invoke<SyncConfig>("get_sync_config");
      syncConfig = cfg;

      const mapping = await invoke<EventMapping>("get_event_mapping", {
        localEventId: eventId,
        localEventName: activeEvent?.nama_event || null,
        targetMode: cfg.mode,
      });
      eventMapping = mapping;

      const stats = await invoke<SyncQueueStats>("get_sync_queue_stats", { eventId });
      queueStats = stats;

      const items = await invoke<SyncQueueEntry[]>("get_sync_queue_items", {
        eventId,
        statusFilter: statusFilter,
        limit: 200,
      });
      queueItems = items;

      const bList = await invoke<BaganItem[]>("get_bagans", { eventId });
      bagans = bList;
    } catch (err: any) {
      console.error("Gagal sinkron data:", err);
    } finally {
      isRefreshing = false;
    }
  }

  function setupAutoRefresh() {
    if (refreshTimer) clearInterval(refreshTimer);
    if (autoRefresh) {
      refreshTimer = setInterval(() => {
        loadAllSyncData();
      }, 4000);
    }
  }

  $effect(() => {
    setupAutoRefresh();
  });

  onDestroy(() => {
    if (refreshTimer) clearInterval(refreshTimer);
  });

  onMount(async () => {
    await loadAllSyncData();
    setupAutoRefresh();
  });

  $effect(() => {
    if (eventId || statusFilter) {
      loadAllSyncData();
    }
  });

  // Target Server Switch
  async function handleSetTarget(mode: "local" | "production" | "custom", customUrl?: string) {
    try {
      const updated = await invoke<SyncConfig>("set_sync_config", {
        mode,
        customUrl: customUrl || null,
      });
      syncConfig = updated;
      toast(`Target: ${updated.label}`);
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal mengubah target: ${err}`);
    }
  }

  async function handlePing() {
    isPinging = true;
    pingResult = null;
    try {
      const res = await invoke<PingResult>("test_sync_connection", { url: syncConfig.url });
      pingResult = res;
      if (res.success) {
        toast(`Koneksi OK (${res.latency_ms} ms)`);
      } else {
        toast(`Gagal ping: ${res.message}`);
      }
    } catch (err: any) {
      toast(`Error koneksi: ${err}`);
    } finally {
      isPinging = false;
    }
  }

  async function handleToggleLiveSync() {
    if (!activeEvent) return;
    isTogglingLive = true;
    const newState = !activeEvent.is_live_sync_enabled;
    try {
      await invoke("toggle_live_sync", { localEventId: eventId, enabled: newState });
      activeEvent.is_live_sync_enabled = newState;
      toast(newState ? "Live Sync AKTIF" : "Live Sync NONAKTIF");
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal: ${err}`);
    } finally {
      isTogglingLive = false;
    }
  }

  // Link Event
  async function openLinkEventModal() {
    showLinkModal = true;
    isLoadingHostedEvents = true;
    try {
      const list = await invoke<HostedEventOption[]>("get_hosted_events", { url: syncConfig.url });
      hostedEvents = list;
      if (eventMapping.hosted_event_id) {
        selectedHostedId = eventMapping.hosted_event_id;
        selectedHostedName = eventMapping.hosted_event_name || "";
      } else if (list.length > 0) {
        const localName = (activeEvent?.nama_event || "").toLowerCase();
        const matched = list.find((e) =>
          e.nama_event.toLowerCase().includes(localName) ||
          localName.includes(e.nama_event.toLowerCase())
        );
        selectedHostedId = matched ? matched.id : list[0].id;
        selectedHostedName = matched ? matched.nama_event : list[0].nama_event;
      }
    } catch (err: any) {
      toast(`Gagal memuat event web: ${err}`);
    } finally {
      isLoadingHostedEvents = false;
    }
  }

  async function handleSaveEventMapping() {
    if (!selectedHostedId) {
      toast("Pilih event di web terlebih dahulu");
      return;
    }
    const found = hostedEvents.find((e) => e.id === selectedHostedId);
    const hName = found ? found.nama_event : selectedHostedName;

    try {
      await invoke("set_event_mapping", {
        localEventId: eventId,
        hostedEventId: selectedHostedId,
        hostedEventName: hName,
        targetMode: syncConfig.mode,
      });
      toast(`Terhubung ke: ${hName}`);
      showLinkModal = false;
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal: ${err}`);
    }
  }

  // Pull Athletes
  async function handlePullAthletes() {
    if (!eventMapping.hosted_event_id) {
      toast("Hubungkan ke event web terlebih dahulu");
      openLinkEventModal();
      return;
    }
    if (!confirm(`Tarik data atlet terbaru dari Web?\n\nEvent Web: [#${eventMapping.hosted_event_id}] ${eventMapping.hosted_event_name}`)) return;

    isPulling = true;
    try {
      const res = await invoke<PullResult>("pull_athletes_from_hosted", { localEventId: eventId });
      toast(res.message);
      await loadAllSyncData();
    } catch (err: any) {
      alert(`Gagal menarik atlet: ${err}`);
    } finally {
      isPulling = false;
    }
  }

  // Push Bagan
  async function handleConfirmPushBagan() {
    if (!eventMapping.hosted_event_id) {
      toast("Hubungkan ke event web terlebih dahulu");
      showPushBaganModal = false;
      openLinkEventModal();
      return;
    }

    const baganIdsToSend = pushScope === "all" ? null : selectedBaganIds;
    if (pushScope === "selected" && selectedBaganIds.length === 0) {
      toast("Pilih minimal 1 bagan");
      return;
    }

    isPushing = true;
    try {
      const msg = await invoke<string>("push_bagan_to_hosted", {
        localEventId: eventId,
        baganIds: baganIdsToSend,
      });
      toast(msg);
      showPushBaganModal = false;
      await loadAllSyncData();
    } catch (err: any) {
      alert(`Gagal mengirim bagan: ${err}`);
    } finally {
      isPushing = false;
    }
  }

  // Force Sync Results
  async function handleConfirmForceSync() {
    if (!eventMapping.hosted_event_id) {
      toast("Hubungkan ke event web terlebih dahulu");
      showForceSyncModal = false;
      openLinkEventModal();
      return;
    }

    isForceSyncing = true;
    try {
      const msg = await invoke<string>("force_sync_results_to_hosted", { localEventId: eventId });
      toast(msg);
      showForceSyncModal = false;
      await loadAllSyncData();
    } catch (err: any) {
      alert(`Gagal force sync: ${err}`);
    } finally {
      isForceSyncing = false;
    }
  }

  // Single Item Actions
  async function handleRetrySingle(item: SyncQueueEntry) {
    try {
      await invoke("retry_sync_item", { itemId: item.id });
      toast(`Item #${item.id} terkirim`);
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal kirim #${item.id}: ${err}`);
      await loadAllSyncData();
    }
  }

  async function handleMarkSynced(item: SyncQueueEntry) {
    try {
      await invoke("mark_sync_item_synced", { itemId: item.id });
      toast(`Item #${item.id} dilewati`);
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal: ${err}`);
    }
  }

  async function handleDeleteSingle(item: SyncQueueEntry) {
    if (!confirm(`Hapus item #${item.id}?`)) return;
    try {
      await invoke("delete_sync_item", { itemId: item.id });
      toast(`Item #${item.id} dihapus`);
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal: ${err}`);
    }
  }

  async function handleRetryAllFailed() {
    isRetryingAll = true;
    try {
      const count = await invoke<number>("retry_all_failed_sync", { eventId });
      toast(`${count} item dikirim ulang`);
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal kirim ulang: ${err}`);
    } finally {
      isRetryingAll = false;
    }
  }

  async function handleClearSynced() {
    try {
      const count = await invoke<number>("clear_synced_items", { eventId });
      toast(`${count} item dibersihkan`);
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal: ${err}`);
    }
  }

  async function handleConfirmClearAll() {
    try {
      const count = await invoke<number>("clear_all_sync_queue", { eventId });
      toast(`Antrean (${count} item) dikosongkan`);
      showClearAllModal = false;
      await loadAllSyncData();
    } catch (err: any) {
      toast(`Gagal: ${err}`);
    }
  }

  function openPayloadModal(item: SyncQueueEntry) {
    selectedPayloadId = item.id;
    try {
      const parsed = JSON.parse(item.payload);
      selectedPayloadJson = JSON.stringify(parsed, null, 2);
    } catch {
      selectedPayloadJson = item.payload;
    }
    showPayloadModal = true;
  }

  async function copyPayloadJson() {
    try {
      await navigator.clipboard.writeText(selectedPayloadJson);
      toast("JSON berhasil disalin!");
    } catch {
      toast("Gagal menyalin JSON");
    }
  }

  let filteredQueue = $derived(
    queueItems.filter((item) => {
      if (!searchKeyword.trim()) return true;
      const kw = searchKeyword.toLowerCase();
      return (
        item.id.toString().includes(kw) ||
        item.endpoint.toLowerCase().includes(kw) ||
        (item.kode_realtime && item.kode_realtime.toLowerCase().includes(kw)) ||
        (item.winner && item.winner.toLowerCase().includes(kw)) ||
        (item.last_error && item.last_error.toLowerCase().includes(kw))
      );
    })
  );
</script>

<div class="h-full flex flex-col p-6 space-y-6 overflow-y-auto select-none bg-[#14161b] text-zinc-200 font-sans scrollbar-thin">
  <!-- Clean Header Bar (1:1 with Tatami Manager) -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4 shrink-0">
    <div>
      <div class="flex items-center space-x-2">
        <h2 class="text-xl font-black text-white tracking-tight uppercase">SINKRONISASI WEB</h2>
        {#if isRefreshing}
          <span class="w-3 h-3 border-2 border-emerald-400 border-t-transparent rounded-full animate-spin"></span>
        {/if}
      </div>
      <p class="text-xs text-zinc-400 mt-0.5 font-normal">
        Target: <strong class="text-zinc-200">{syncConfig.label}</strong>
        {#if eventMapping.hosted_event_name}
          • Terhubung ke: <strong class="text-emerald-400">[#{eventMapping.hosted_event_id}] {eventMapping.hosted_event_name}</strong>
        {:else}
          • <span class="text-amber-400 font-medium">Belum Terhubung</span>
        {/if}
      </p>
    </div>

    <!-- Quick Toolbar -->
    <div class="flex items-center gap-2 flex-wrap">
      <button
        onclick={handlePing}
        disabled={isPinging}
        class="px-3 py-2 bg-[#1c1e24] hover:bg-[#252830] active:scale-[0.98] text-zinc-300 font-bold text-xs rounded-xl border border-white/[0.08] transition flex items-center gap-1.5"
        title="Uji koneksi ke web server"
      >
        <span class="w-2 h-2 rounded-full {isPinging ? 'bg-amber-400 animate-ping' : pingResult?.success ? 'bg-emerald-400' : 'bg-zinc-500'}"></span>
        <span>{isPinging ? "Ping..." : pingResult ? `${pingResult.latency_ms} ms` : "Ping Web"}</span>
      </button>

      {#if queueStats.failed_count > 0}
        <button
          onclick={handleRetryAllFailed}
          disabled={isRetryingAll}
          class="px-3 py-2 bg-rose-600 hover:bg-rose-500 active:scale-[0.98] text-white font-bold text-xs rounded-xl shadow transition flex items-center gap-1.5"
        >
          <span>Kirim Ulang ({queueStats.failed_count})</span>
        </button>
      {/if}

      {#if queueStats.synced_count > 0}
        <button
          onclick={handleClearSynced}
          class="px-3 py-2 bg-[#1c1e24] hover:bg-[#252830] active:scale-[0.98] text-zinc-400 hover:text-zinc-200 font-bold text-xs rounded-xl border border-white/[0.08] transition"
        >
          Bersihkan Sukses
        </button>
      {/if}

      <label class="flex items-center gap-2 bg-[#181a20] px-3 py-2 rounded-xl border border-white/[0.08] cursor-pointer">
        <input
          type="checkbox"
          bind:checked={autoRefresh}
          class="w-3.5 h-3.5 accent-emerald-500 rounded cursor-pointer"
        />
        <span class="text-xs font-semibold text-zinc-300">Auto-Refresh</span>
      </label>
    </div>
  </div>

  <!-- 1. Live Sync Control Strip -->
  <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex items-center justify-between gap-4 flex-wrap">
    <div class="flex items-center gap-3">
      <div class="w-10 h-10 rounded-xl flex items-center justify-center shrink-0 border {activeEvent?.is_live_sync_enabled ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400' : 'bg-zinc-800 border-white/[0.08] text-zinc-500'}">
        <svg class="w-5 h-5 {activeEvent?.is_live_sync_enabled ? 'animate-pulse' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      </div>
      <div>
        <div class="flex items-center gap-2">
          <span class="text-xs font-black uppercase text-white">Live Streaming:</span>
          <span class="text-[10px] font-black uppercase px-2 py-0.5 rounded-full border {activeEvent?.is_live_sync_enabled ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/30' : 'bg-zinc-800 text-zinc-400 border-white/[0.08]'}">
            {activeEvent?.is_live_sync_enabled ? "AKTIF" : "OFFLINE ONLY"}
          </span>
        </div>
        <p class="text-[11px] text-zinc-400 mt-0.5">
          {activeEvent?.is_live_sync_enabled ? "Skor pertandingan tatami langsung disiarkan ke server web." : "Pertandingan berjalan lokal tanpa pengiriman realtime."}
        </p>
      </div>
    </div>

    <div class="flex items-center gap-2">
      {#if !eventMapping.hosted_event_id}
        <button
          onclick={openLinkEventModal}
          class="bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 font-bold text-xs px-3 py-2 rounded-xl border border-amber-500/30 transition"
        >
          Hubungkan Event Web
        </button>
      {/if}
      <button
        onclick={handleToggleLiveSync}
        disabled={isTogglingLive}
        class="font-bold text-xs px-4 py-2 rounded-xl transition active:scale-[0.98] {activeEvent?.is_live_sync_enabled ? 'bg-rose-600 hover:bg-rose-500 text-white' : 'bg-white text-black hover:bg-zinc-200'}"
      >
        {activeEvent?.is_live_sync_enabled ? "Matikan Live Sync" : "Aktifkan Live Sync"}
      </button>
    </div>
  </div>

  <!-- 2. Clean 3-Cards Operations Row -->
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    <!-- Card 1: Target Server -->
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex flex-col justify-between space-y-3">
      <div>
        <div class="flex items-center justify-between text-xs font-bold text-zinc-400 uppercase tracking-wider mb-2">
          <span>Target Web Server</span>
          <span class="text-[10px] px-1.5 py-0.5 rounded bg-[#111215] text-zinc-400 font-mono">{syncConfig.mode}</span>
        </div>
        <div class="grid grid-cols-3 gap-1.5">
          <button
            onclick={() => handleSetTarget("local")}
            class="py-2 px-1 rounded-xl text-center text-xs font-bold transition border {syncConfig.mode === 'local' ? 'bg-white text-black border-white' : 'bg-[#111215] text-zinc-400 border-white/[0.06] hover:text-white'}"
          >
            Dev Lokal
          </button>
          <button
            onclick={() => handleSetTarget("production")}
            class="py-2 px-1 rounded-xl text-center text-xs font-bold transition border {syncConfig.mode === 'production' ? 'bg-white text-black border-white' : 'bg-[#111215] text-zinc-400 border-white/[0.06] hover:text-white'}"
          >
            Production
          </button>
          <button
            onclick={() => {
              customUrlInput = syncConfig.mode === "custom" ? syncConfig.url : "http://127.0.0.1:8001";
              showCustomUrlModal = true;
            }}
            class="py-2 px-1 rounded-xl text-center text-xs font-bold transition border {syncConfig.mode === 'custom' ? 'bg-white text-black border-white' : 'bg-[#111215] text-zinc-400 border-white/[0.06] hover:text-white'}"
          >
            Kustom
          </button>
        </div>
      </div>
      <div class="text-[11px] font-mono text-zinc-400 truncate bg-[#111215] px-2.5 py-1.5 rounded-lg border border-white/[0.04]">
        {syncConfig.url}
      </div>
    </div>

    <!-- Card 2: Event Web Mapping -->
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex flex-col justify-between space-y-3">
      <div>
        <div class="flex items-center justify-between text-xs font-bold text-zinc-400 uppercase tracking-wider mb-2">
          <span>Event Web Tujuan</span>
          {#if eventMapping.is_mismatch}
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/20 text-amber-400 font-bold">Beda Nama</span>
          {/if}
        </div>
        <div class="text-xs font-bold text-white truncate">
          {eventMapping.hosted_event_name ? `[#${eventMapping.hosted_event_id}] ${eventMapping.hosted_event_name}` : "Belum dihubungkan"}
        </div>
        <div class="text-[11px] text-zinc-400 truncate mt-0.5">
          Lokal: {activeEvent?.nama_event || "-"}
        </div>
      </div>
      <button
        onclick={openLinkEventModal}
        class="w-full py-2 bg-[#1c1e24] hover:bg-[#252830] active:scale-[0.98] text-zinc-200 font-bold text-xs rounded-xl border border-white/[0.08] transition text-center"
      >
        {eventMapping.hosted_event_id ? "Ubah Hubungan Event" : "Hubungkan ke Web"}
      </button>
    </div>

    <!-- Card 3: Batch Mass Sync -->
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex flex-col justify-between space-y-3">
      <div>
        <div class="flex items-center justify-between text-xs font-bold text-zinc-400 uppercase tracking-wider mb-2">
          <span>Aksi Sinkronisasi Massal</span>
          <span class="text-[10px] text-zinc-500">{bagans.length} bagan</span>
        </div>
        <div class="grid grid-cols-3 gap-1.5">
          <button
            onclick={handlePullAthletes}
            disabled={isPulling}
            class="py-2 rounded-xl text-center text-xs font-bold bg-[#111215] hover:bg-[#1f222a] text-sky-400 border border-sky-500/30 transition flex flex-col items-center justify-center gap-1"
          >
            <span>{isPulling ? "Tarik..." : "Tarik Atlet"}</span>
          </button>
          <button
            onclick={() => (showPushBaganModal = true)}
            class="py-2 rounded-xl text-center text-xs font-bold bg-[#111215] hover:bg-[#1f222a] text-amber-400 border border-amber-500/30 transition flex flex-col items-center justify-center gap-1"
          >
            <span>Kirim Bagan</span>
          </button>
          <button
            onclick={() => (showForceSyncModal = true)}
            class="py-2 rounded-xl text-center text-xs font-bold bg-[#111215] hover:bg-[#1f222a] text-rose-400 border border-rose-500/30 transition flex flex-col items-center justify-center gap-1"
          >
            <span>Force Sync</span>
          </button>
        </div>
      </div>
      <div class="text-[10.5px] text-zinc-500">
        Audit atau pemulihan data bagan dan hasil partai.
      </div>
    </div>
  </div>

  <!-- 3. Stat Cards (Matching Tatami Manager) -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-4">
    <div class="p-4 bg-[#181a20] rounded-2xl border border-white/[0.08] flex items-center justify-between">
      <div>
        <span class="text-[11px] font-bold text-zinc-400 uppercase tracking-wider block">Total Antrean</span>
        <span class="text-2xl font-black text-white font-mono">{queueStats.total_count}</span>
      </div>
      <span class="w-2.5 h-2.5 rounded-full bg-zinc-500"></span>
    </div>

    <button
      onclick={() => (statusFilter = "failed")}
      class="p-4 bg-[#181a20] rounded-2xl border flex items-center justify-between transition text-left {queueStats.failed_count > 0 ? 'border-rose-500/50 bg-rose-950/20' : 'border-white/[0.08]'}"
    >
      <div>
        <span class="text-[11px] font-bold uppercase tracking-wider block {queueStats.failed_count > 0 ? 'text-rose-400' : 'text-zinc-400'}">Gagal</span>
        <span class="text-2xl font-black font-mono {queueStats.failed_count > 0 ? 'text-rose-400' : 'text-zinc-400'}">{queueStats.failed_count}</span>
      </div>
      <span class="w-2.5 h-2.5 rounded-full {queueStats.failed_count > 0 ? 'bg-rose-500 animate-ping' : 'bg-zinc-700'}"></span>
    </button>

    <button
      onclick={() => (statusFilter = "pending")}
      class="p-4 bg-[#181a20] rounded-2xl border border-white/[0.08] flex items-center justify-between transition text-left"
    >
      <div>
        <span class="text-[11px] font-bold text-amber-400 uppercase tracking-wider block">Tertunda</span>
        <span class="text-2xl font-black text-amber-400 font-mono">{queueStats.pending_count}</span>
      </div>
      <span class="w-2.5 h-2.5 rounded-full bg-amber-400"></span>
    </button>

    <button
      onclick={() => (statusFilter = "synced")}
      class="p-4 bg-[#181a20] rounded-2xl border border-white/[0.08] flex items-center justify-between transition text-left"
    >
      <div>
        <span class="text-[11px] font-bold text-emerald-400 uppercase tracking-wider block">Terkirim</span>
        <span class="text-2xl font-black text-emerald-400 font-mono">{queueStats.synced_count}</span>
      </div>
      <span class="w-2.5 h-2.5 rounded-full bg-emerald-400"></span>
    </button>
  </div>

  <!-- 4. Queue Table & Search Bar -->
  <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] overflow-hidden shadow-lg">
    <!-- Table Toolbar -->
    <div class="p-3.5 border-b border-white/[0.08] flex items-center justify-between gap-3 flex-wrap">
      <!-- Segmented Status Tabs -->
      <div class="flex items-center gap-1 bg-[#111215] p-1 rounded-xl border border-white/[0.06]">
        <button
          onclick={() => (statusFilter = "all")}
          class="px-3 py-1 rounded-lg text-xs font-bold transition {statusFilter === 'all' ? 'bg-zinc-800 text-white' : 'text-zinc-400 hover:text-white'}"
        >
          Semua ({queueStats.total_count})
        </button>
        <button
          onclick={() => (statusFilter = "failed")}
          class="px-3 py-1 rounded-lg text-xs font-bold transition {statusFilter === 'failed' ? 'bg-rose-900/60 text-rose-300' : 'text-zinc-400 hover:text-rose-300'}"
        >
          Gagal ({queueStats.failed_count})
        </button>
        <button
          onclick={() => (statusFilter = "pending")}
          class="px-3 py-1 rounded-lg text-xs font-bold transition {statusFilter === 'pending' ? 'bg-amber-900/60 text-amber-300' : 'text-zinc-400 hover:text-amber-300'}"
        >
          Tertunda ({queueStats.pending_count})
        </button>
        <button
          onclick={() => (statusFilter = "synced")}
          class="px-3 py-1 rounded-lg text-xs font-bold transition {statusFilter === 'synced' ? 'bg-emerald-900/60 text-emerald-300' : 'text-zinc-400 hover:text-emerald-300'}"
        >
          Terkirim ({queueStats.synced_count})
        </button>
      </div>

      <!-- Search Input -->
      <div class="w-64">
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari ID, partai, error..."
          class="w-full bg-[#111215] text-xs text-white px-3 py-1.5 rounded-xl border border-white/[0.08] focus:border-white outline-none"
        />
      </div>
    </div>

    <!-- Table -->
    <div class="overflow-x-auto">
      <table class="w-full text-left text-xs">
        <thead class="bg-[#111215] text-zinc-400 uppercase font-bold text-[10.5px] border-b border-white/[0.08]">
          <tr>
            <th class="px-4 py-2.5 text-center w-16">ID</th>
            <th class="px-4 py-2.5 w-36">Endpoint</th>
            <th class="px-4 py-2.5">Pertandingan & Data</th>
            <th class="px-4 py-2.5 text-center w-28">Status</th>
            <th class="px-4 py-2.5 text-center w-16">Retry</th>
            <th class="px-4 py-2.5">Error</th>
            <th class="px-4 py-2.5 text-center w-32">Aksi</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-white/[0.04]">
          {#if filteredQueue.length === 0}
            <tr>
              <td colspan="7" class="py-10 text-center text-zinc-500 text-xs">
                Tidak ada antrean sinkronisasi
              </td>
            </tr>
          {:else}
            {#each filteredQueue as item (item.id)}
              <tr class="hover:bg-white/[0.02] transition">
                <td class="px-4 py-2.5 text-center font-mono font-bold text-zinc-400">
                  #{item.id}
                </td>
                <td class="px-4 py-2.5">
                  <span class="font-mono text-[11px] font-bold text-zinc-200">/{item.endpoint}</span>
                </td>
                <td class="px-4 py-2.5">
                  <div class="flex items-center gap-2 flex-wrap">
                    {#if item.ring}
                      <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-300">T{item.ring}</span>
                    {/if}
                    {#if item.round && item.urutan}
                      <span class="text-[10px] text-zinc-400 font-mono">B{item.round}P{item.urutan}</span>
                    {/if}
                    {#if item.score}
                      <span class="text-amber-400 font-bold font-mono text-[11px]">{item.score}</span>
                    {/if}
                    {#if item.winner}
                      <span class="text-emerald-400 font-bold text-[11px]">{item.winner}</span>
                    {/if}
                    <button
                      onclick={() => openPayloadModal(item)}
                      class="text-[10px] text-zinc-400 hover:text-white underline ml-auto"
                    >
                      JSON
                    </button>
                  </div>
                </td>
                <td class="px-4 py-2.5 text-center">
                  {#if item.status === "failed"}
                    <span class="text-[10px] font-bold uppercase px-2 py-0.5 rounded bg-rose-500/10 text-rose-400 border border-rose-500/30">GAGAL</span>
                  {:else if item.status === "pending" || item.status === "processing"}
                    <span class="text-[10px] font-bold uppercase px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/30">PENDING</span>
                  {:else if item.status === "synced"}
                    <span class="text-[10px] font-bold uppercase px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30">SYNCED</span>
                  {:else}
                    <span class="text-[10px] font-bold uppercase px-2 py-0.5 rounded bg-zinc-800 text-zinc-400">{item.status}</span>
                  {/if}
                </td>
                <td class="px-4 py-2.5 text-center font-mono text-xs text-zinc-400">
                  {item.retry_count > 0 ? `${item.retry_count}x` : "-"}
                </td>
                <td class="px-4 py-2.5 text-[11px] font-mono text-rose-300 max-w-xs truncate">
                  {item.last_error || "-"}
                </td>
                <td class="px-4 py-2.5 text-center">
                  <div class="inline-flex items-center gap-1">
                    {#if item.status === "failed" || item.status === "pending"}
                      <button
                        onclick={() => handleRetrySingle(item)}
                        class="bg-zinc-800 hover:bg-zinc-700 text-white font-bold text-[11px] px-2 py-0.5 rounded transition"
                      >
                        Kirim
                      </button>
                      <button
                        onclick={() => handleMarkSynced(item)}
                        class="bg-[#111215] hover:bg-zinc-800 text-zinc-400 hover:text-white font-bold text-[11px] px-2 py-0.5 rounded transition"
                      >
                        Lewati
                      </button>
                    {/if}
                    <button
                      onclick={() => handleDeleteSingle(item)}
                      class="text-zinc-500 hover:text-rose-400 p-1 transition"
                      title="Hapus"
                    >
                      ✕
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
  </div>
</div>

<!-- ================= MODALS ================= -->

<!-- Modal Link Event -->
{#if showLinkModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-md shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-sm font-black uppercase text-white tracking-wider">Hubungkan ke Event Web</h3>
        <button onclick={() => (showLinkModal = false)} class="text-zinc-400 hover:text-white">✕</button>
      </div>

      <div class="space-y-3">
        <div class="bg-[#111215] p-3 rounded-xl border border-white/[0.06] text-xs space-y-1">
          <div class="flex justify-between">
            <span class="text-zinc-500">Target Server:</span>
            <span class="text-zinc-200 font-bold">{syncConfig.label}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-zinc-500">Event Lokal:</span>
            <span class="text-zinc-200 font-bold">{activeEvent?.nama_event}</span>
          </div>
        </div>

        {#if isLoadingHostedEvents}
          <div class="py-4 text-center text-xs text-zinc-500">Mengambil event dari web server...</div>
        {:else}
          <div>
            <div class="text-xs font-bold text-zinc-300 mb-1">Pilih Event di Web Public:</div>
            <select
              bind:value={selectedHostedId}
              class="w-full bg-[#111215] text-xs text-white p-2.5 rounded-xl border border-white/[0.08] focus:border-white outline-none"
            >
              <option value={null}>-- Pilih Event --</option>
              {#each hostedEvents as ev}
                <option value={ev.id}>[#{ev.id}] {ev.nama_event}</option>
              {/each}
            </select>
          </div>
        {/if}
      </div>

      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={() => (showLinkModal = false)} class="px-4 py-2 text-xs font-semibold text-zinc-400">Batal</button>
        <button onclick={handleSaveEventMapping} class="bg-white text-black font-bold text-xs px-4 py-2 rounded-xl transition hover:bg-zinc-200">
          Simpan Hubungan
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Push Bagan -->
{#if showPushBaganModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-md shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-sm font-black uppercase text-white tracking-wider">Kirim Bagan ke Web</h3>
        <button onclick={() => (showPushBaganModal = false)} class="text-zinc-400 hover:text-white">✕</button>
      </div>

      <div class="space-y-3">
        <label class="p-3 rounded-xl border border-white/[0.08] bg-[#111215] flex items-center gap-3 cursor-pointer">
          <input type="radio" name="push_scope" value="all" bind:group={pushScope} class="accent-white cursor-pointer" />
          <div>
            <div class="text-xs font-bold text-white">Kirim Semua ({bagans.length} bagan)</div>
          </div>
        </label>

        <label class="p-3 rounded-xl border border-white/[0.08] bg-[#111215] flex items-center gap-3 cursor-pointer">
          <input type="radio" name="push_scope" value="selected" bind:group={pushScope} class="accent-white cursor-pointer" />
          <div>
            <div class="text-xs font-bold text-white">Pilih Bagan Tertentu</div>
          </div>
        </label>

        {#if pushScope === "selected"}
          <div class="max-h-48 overflow-y-auto space-y-1 p-2 bg-[#111215] rounded-xl border border-white/[0.06] scrollbar-thin">
            {#each bagans as b}
              <label class="flex items-center gap-2 p-1.5 hover:bg-white/[0.04] rounded-lg text-xs cursor-pointer">
                <input type="checkbox" value={b.id} bind:group={selectedBaganIds} class="accent-white rounded cursor-pointer" />
                <span class="text-zinc-300 font-medium">{b.nama_bagan || `Bagan #${b.id}`}</span>
              </label>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={() => (showPushBaganModal = false)} class="px-4 py-2 text-xs font-semibold text-zinc-400">Batal</button>
        <button onclick={handleConfirmPushBagan} disabled={isPushing} class="bg-white text-black font-bold text-xs px-5 py-2 rounded-xl transition hover:bg-zinc-200">
          {isPushing ? "Mengirim..." : "Kirim"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Force Sync -->
{#if showForceSyncModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-sm shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-sm font-black uppercase text-rose-400 tracking-wider">Force Sync Hasil</h3>
        <button onclick={() => (showForceSyncModal = false)} class="text-zinc-400 hover:text-white">✕</button>
      </div>
      <p class="text-xs text-zinc-300 leading-relaxed">
        Kirimkan ulang seluruh hasil pertandingan lokal yang telah selesai ke server web?
      </p>
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={() => (showForceSyncModal = false)} class="px-4 py-2 text-xs font-semibold text-zinc-400">Batal</button>
        <button onclick={handleConfirmForceSync} disabled={isForceSyncing} class="bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs px-5 py-2 rounded-xl transition">
          {isForceSyncing ? "Memproses..." : "Ya, Force Sync"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Payload JSON -->
{#if showPayloadModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-lg shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-sm font-black uppercase text-white tracking-wider">Payload JSON #{selectedPayloadId}</h3>
        <button onclick={() => (showPayloadModal = false)} class="text-zinc-400 hover:text-white">✕</button>
      </div>
      <pre class="bg-[#111215] p-3 rounded-xl border border-white/[0.08] font-mono text-xs text-zinc-200 overflow-auto max-h-80 select-text whitespace-pre-wrap">{selectedPayloadJson}</pre>
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={copyPayloadJson} class="bg-zinc-800 hover:bg-zinc-700 text-white font-bold text-xs px-3 py-1.5 rounded-lg border border-white/[0.08] transition">
          Salin JSON
        </button>
        <button onclick={() => (showPayloadModal = false)} class="bg-white text-black font-bold text-xs px-4 py-1.5 rounded-xl transition hover:bg-zinc-200">
          Tutup
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Custom URL -->
{#if showCustomUrlModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-sm shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-sm font-black uppercase text-white tracking-wider">URL Server Kustom</h3>
        <button onclick={() => (showCustomUrlModal = false)} class="text-zinc-400 hover:text-white">✕</button>
      </div>
      <input
        type="text"
        bind:value={customUrlInput}
        placeholder="http://192.168.1.100:8001"
        class="w-full bg-[#111215] text-xs text-white p-2.5 rounded-xl border border-white/[0.08] focus:border-white outline-none font-mono"
      />
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={() => (showCustomUrlModal = false)} class="px-4 py-2 text-xs font-semibold text-zinc-400">Batal</button>
        <button
          onclick={() => {
            showCustomUrlModal = false;
            handleSetTarget("custom", customUrlInput);
          }}
          class="bg-white text-black font-bold text-xs px-4 py-2 rounded-xl transition hover:bg-zinc-200"
        >
          Terapkan
        </button>
      </div>
    </div>
  </div>
{/if}
