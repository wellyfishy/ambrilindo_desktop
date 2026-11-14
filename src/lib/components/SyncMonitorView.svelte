<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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
  import FuzzySelect, { type SelectOption } from "./FuzzySelect.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";
  import { openLicensePanel } from "$lib/license.svelte";

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
  // How this computer authenticates to the web: its license (only granted events) or the old shared token.
  let authStatus = $state<{ mode: string; message: string | null; blocked_reason: string | null }>({
    mode: "license",
    message: null,
    blocked_reason: null,
  });
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

  // Confirmation Modal State (Bulletproof: No window.confirm)
  let confirmModalConfig = $state<{
    open: boolean;
    title: string;
    message: string;
    confirmText: string;
    cancelText: string;
    isDestructive: boolean;
    isLoading: boolean;
    onConfirm: () => Promise<void> | void;
  }>({
    open: false,
    title: "",
    message: "",
    confirmText: "Lanjutkan",
    cancelText: "Batal",
    isDestructive: true,
    isLoading: false,
    onConfirm: () => {},
  });

  function openConfirmModal(opts: {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    isDestructive?: boolean;
    onConfirm: () => Promise<void> | void;
  }) {
    confirmModalConfig = {
      open: true,
      title: opts.title,
      message: opts.message,
      confirmText: opts.confirmText || "Lanjutkan",
      cancelText: opts.cancelText || "Batal",
      isDestructive: opts.isDestructive !== false,
      isLoading: false,
      onConfirm: opts.onConfirm,
    };
  }

  function closeConfirmModal() {
    confirmModalConfig.open = false;
  }
  let isLoadingHostedEvents = $state<boolean>(false);
  let selectedHostedId = $state<number | null>(null);
  let selectedHostedName = $state<string>("");

  let hostedEventOptions = $derived<SelectOption[]>(
    hostedEvents.map((ev) => ({
      value: ev.id,
      label: `[#${ev.id}] ${ev.nama_event}`,
    }))
  );

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

      authStatus = await invoke("get_sync_auth_status");
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

  let unlistenSync: UnlistenFn | null = null;

  onDestroy(() => {
    if (refreshTimer) clearInterval(refreshTimer);
    if (unlistenSync) unlistenSync();
  });

  onMount(async () => {
    await loadAllSyncData();
    setupAutoRefresh();

    unlistenSync = await listen("sync_queue_changed", () => {
      loadAllSyncData();
    });
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
      toast(newState ? "Live sync aktif" : "Live sync dimatikan");
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
  function handlePullAthletes() {
    if (!eventMapping.hosted_event_id) {
      toast("Hubungkan ke event web terlebih dahulu");
      openLinkEventModal();
      return;
    }
    openConfirmModal({
      title: "Tarik atlet dari web?",
      message: `Atlet terbaru dari event web "${eventMapping.hosted_event_name}" (ID ${eventMapping.hosted_event_id}) akan ditarik ke event ini.`,
      confirmText: "Tarik atlet",
      isDestructive: false,
      onConfirm: async () => {
        isPulling = true;
        try {
          const res = await invoke<PullResult>("pull_athletes_from_hosted", { localEventId: eventId });
          toast(res.message);
          closeConfirmModal();
          await loadAllSyncData();
        } catch (err: any) {
          toast(`Gagal menarik atlet: ${err}`);
        } finally {
          isPulling = false;
        }
      },
    });
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
      toast(`Gagal mengirim bagan: ${err}`);
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
      toast(`Gagal mengirim ulang hasil: ${err}`);
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

  function handleDeleteSingle(item: SyncQueueEntry) {
    openConfirmModal({
      title: `Hapus item #${item.id}?`,
      message: `Item ini dibuang dari antrean dan tidak akan dikirim ke web.`,
      confirmText: "Hapus item",
      isDestructive: true,
      onConfirm: async () => {
        try {
          await invoke("delete_sync_item", { itemId: item.id });
          toast(`Item #${item.id} dihapus`);
          closeConfirmModal();
          await loadAllSyncData();
        } catch (err: any) {
          toast(`Gagal: ${err}`);
        }
      },
    });
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

  let isLinked = $derived(!!eventMapping.hosted_event_id);
  let liveOn = $derived(!!activeEvent?.is_live_sync_enabled);
  let targetName = $derived(
    syncConfig.mode === "production" ? "Production" : syncConfig.mode === "local" ? "Dev lokal" : "Server kustom"
  );

  let statusTabs = $derived([
    { key: "all", label: "Semua", count: queueStats.total_count, warn: false, dot: "" },
    { key: "failed", label: "Gagal", count: queueStats.failed_count, warn: queueStats.failed_count > 0, dot: "bg-rose-400" },
    { key: "pending", label: "Tertunda", count: queueStats.pending_count, warn: false, dot: "bg-amber-400" },
    { key: "synced", label: "Terkirim", count: queueStats.synced_count, warn: false, dot: "bg-emerald-400" },
  ]);

  function statusInfo(status: string): { label: string; cls: string; dot: string } {
    if (status === "failed") return { label: "Gagal", cls: "text-rose-300", dot: "bg-rose-400" };
    if (status === "pending" || status === "processing") return { label: "Tertunda", cls: "text-amber-300", dot: "bg-amber-400" };
    if (status === "synced") return { label: "Terkirim", cls: "text-emerald-300", dot: "bg-emerald-400" };
    if (status === "abandoned") return { label: "Dibatalkan", cls: "text-zinc-400", dot: "bg-zinc-500" };
    return { label: status, cls: "text-zinc-400", dot: "bg-zinc-500" };
  }
</script>

<div class="h-full flex flex-col p-6 overflow-y-auto select-none bg-[#14161b] text-zinc-200 font-sans scrollbar-thin">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4 shrink-0">
    <div class="min-w-0">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">Sinkronisasi web</h2>
      <p class="text-xs text-zinc-400 mt-1 truncate">
        {#if isLinked}
          Terhubung ke <span class="text-zinc-200 font-medium select-text">{eventMapping.hosted_event_name}</span>
          <span class="text-zinc-500">· {targetName}</span>
        {:else}
          Belum terhubung ke event web
        {/if}
      </p>
    </div>

    <div class="flex items-center gap-2 shrink-0 self-start sm:self-auto">
      <button
        type="button"
        role="switch"
        aria-checked={autoRefresh}
        onclick={() => (autoRefresh = !autoRefresh)}
        class="h-9 px-3.5 font-semibold text-xs rounded-xl border transition-colors flex items-center gap-2 cursor-pointer bg-white/[0.04] text-zinc-300 border-white/[0.1] hover:bg-white/[0.08]"
      >
        <span class="relative inline-block w-7 h-4 rounded-full transition {autoRefresh ? 'bg-emerald-500' : 'bg-zinc-600'}">
          <span class="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all {autoRefresh ? 'left-3.5' : 'left-0.5'}"></span>
        </span>
        <span>Perbarui otomatis</span>
      </button>
      <button
        type="button"
        onclick={() => loadAllSyncData()}
        aria-label="Segarkan antrean"
        title="Segarkan antrean"
        class="w-9 h-9 flex items-center justify-center rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.1] transition-colors cursor-pointer"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
      </button>
    </div>
  </div>

  {#if authStatus.mode === "none"}
    <div class="mt-4 p-3.5 rounded-xl border border-amber-500/35 bg-amber-500/[0.06] flex flex-col sm:flex-row sm:items-center justify-between gap-3 shrink-0" role="status">
      <div class="flex items-start gap-2.5 text-sm text-amber-100">
        <svg class="w-4 h-4 mt-0.5 shrink-0 text-amber-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" /></svg>
        <span>{authStatus.message || "Sinkron web butuh lisensi aktif."} Antrean tetap disimpan dan dikirim setelah lisensi aktif.</span>
      </div>
      <button type="button" onclick={openLicensePanel} class="shrink-0 h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors cursor-pointer">Buka lisensi</button>
    </div>
  {:else if authStatus.blocked_reason}
    <div class="mt-4 p-3.5 rounded-xl border border-rose-500/35 bg-rose-500/[0.06] flex flex-col sm:flex-row sm:items-center justify-between gap-3 shrink-0" role="alert">
      <div class="flex items-start gap-2.5 text-sm text-rose-100 min-w-0">
        <svg class="w-4 h-4 mt-0.5 shrink-0 text-rose-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
        <span class="min-w-0">
          <span class="block font-semibold">Web menolak komputer ini. Pengiriman dijeda 5 menit.</span>
          <span class="block text-xs text-rose-200/80 mt-0.5 select-text break-words">{authStatus.blocked_reason}</span>
        </span>
      </div>
      <button type="button" onclick={handleRetryAllFailed} disabled={isRetryingAll} class="shrink-0 h-9 px-3.5 bg-white/[0.06] hover:bg-white/[0.12] text-white font-semibold text-xs rounded-xl border border-white/[0.15] transition-colors disabled:opacity-50 cursor-pointer">Coba lagi sekarang</button>
    </div>
  {:else if authStatus.mode === "legacy"}
    <p class="mt-3 text-xs text-zinc-500 shrink-0">Memakai token lama, bukan lisensi. Aktifkan lisensi supaya sinkron tetap jalan setelah token lama dimatikan.</p>
  {/if}

  <!-- Setup: server → event web → live sync -->
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-3 mt-4 shrink-0">
    <!-- 1. Server -->
    <section class="bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex flex-col gap-3" aria-labelledby="sync-server-title">
      <div class="flex items-center gap-2">
        <span class="w-5 h-5 rounded-full bg-white/[0.08] text-[11px] font-bold text-zinc-300 flex items-center justify-center">1</span>
        <h3 id="sync-server-title" class="text-sm font-bold text-white">Server web</h3>
      </div>
      <div class="grid grid-cols-3 gap-1 bg-[#111215] p-1 rounded-xl border border-white/[0.06]" role="radiogroup" aria-label="Server tujuan">
        {#each [{ key: "production", label: "Production" }, { key: "local", label: "Dev lokal" }, { key: "custom", label: "Kustom…" }] as opt (opt.key)}
          {@const selected = syncConfig.mode === opt.key}
          <button
            type="button"
            role="radio"
            aria-checked={selected}
            onclick={() => {
              if (opt.key === "custom") {
                customUrlInput = syncConfig.mode === "custom" ? syncConfig.url : "http://127.0.0.1:8001";
                showCustomUrlModal = true;
              } else {
                handleSetTarget(opt.key as "local" | "production");
              }
            }}
            class="h-8 rounded-lg text-xs font-semibold transition-colors cursor-pointer {selected ? 'bg-white text-black' : 'text-zinc-400 hover:text-white'}"
          >
            {opt.label}
          </button>
        {/each}
      </div>
      <div class="flex items-center justify-between gap-2 min-w-0">
        <span class="font-mono text-[11px] text-zinc-400 truncate select-text" title={syncConfig.url}>{syncConfig.url}</span>
        <button
          type="button"
          onclick={handlePing}
          disabled={isPinging}
          class="shrink-0 h-8 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-lg border border-white/[0.1] transition-colors disabled:opacity-50 cursor-pointer"
        >
          {isPinging ? "Menguji…" : "Uji koneksi"}
        </button>
      </div>
      {#if pingResult}
        <p class="text-xs flex items-center gap-1.5 {pingResult.success ? 'text-emerald-300' : 'text-rose-300'}" role="status">
          {#if pingResult.success}
            <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
            <span>Terhubung · {pingResult.latency_ms} ms</span>
          {:else}
            <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
            <span class="truncate select-text" title={pingResult.message}>Gagal: {pingResult.message}</span>
          {/if}
        </p>
      {/if}
    </section>

    <!-- 2. Event web -->
    <section class="bg-[#181a20] rounded-2xl border p-4 flex flex-col gap-3 {isLinked ? 'border-white/[0.08]' : 'border-amber-500/35'}" aria-labelledby="sync-event-title">
      <div class="flex items-center gap-2">
        <span class="w-5 h-5 rounded-full bg-white/[0.08] text-[11px] font-bold text-zinc-300 flex items-center justify-center">2</span>
        <h3 id="sync-event-title" class="text-sm font-bold text-white">Event web</h3>
      </div>
      {#if isLinked}
        <div class="min-w-0">
          <p class="text-sm font-semibold text-white truncate select-text" title={eventMapping.hosted_event_name || ""}>{eventMapping.hosted_event_name}</p>
          <p class="text-xs text-zinc-500 mt-0.5 font-mono">ID web {eventMapping.hosted_event_id}</p>
          {#if eventMapping.is_mismatch}
            <p class="text-xs text-amber-300 mt-1.5 flex items-center gap-1.5">
              <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
              <span>Namanya beda dengan event lokal</span>
            </p>
          {/if}
        </div>
      {:else}
        <p class="text-xs text-amber-300 flex items-center gap-1.5">
          <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
          <span>Belum terhubung. Atlet, bagan dan hasil belum bisa dikirim.</span>
        </p>
      {/if}
      <button
        type="button"
        onclick={openLinkEventModal}
        class="mt-auto h-9 px-3.5 font-semibold text-xs rounded-xl transition-colors cursor-pointer {isLinked
          ? 'bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 border border-white/[0.1]'
          : 'bg-white text-black hover:bg-zinc-200 font-bold shadow-md'}"
      >
        {isLinked ? "Ganti event web" : "Hubungkan event web"}
      </button>
    </section>

    <!-- 3. Live sync -->
    <section class="bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex flex-col gap-3" aria-labelledby="sync-live-title">
      <div class="flex items-center gap-2">
        <span class="w-5 h-5 rounded-full bg-white/[0.08] text-[11px] font-bold text-zinc-300 flex items-center justify-center">3</span>
        <h3 id="sync-live-title" class="text-sm font-bold text-white">Live sync</h3>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={liveOn}
        aria-label="Kirim skor langsung ke web"
        onclick={handleToggleLiveSync}
        disabled={isTogglingLive || !activeEvent}
        class="flex items-center justify-between gap-3 w-full rounded-xl border px-3 py-2.5 text-left transition-colors cursor-pointer disabled:opacity-50 {liveOn ? 'border-emerald-500/40 bg-emerald-500/[0.06]' : 'border-white/[0.08] bg-[#111215] hover:border-white/20'}"
      >
        <span class="text-xs font-semibold {liveOn ? 'text-emerald-200' : 'text-zinc-300'}">Kirim skor langsung</span>
        <span class="relative inline-block w-9 h-5 rounded-full transition shrink-0 {liveOn ? 'bg-emerald-500' : 'bg-zinc-600'}">
          <span class="absolute top-0.5 w-4 h-4 rounded-full bg-white transition-all {liveOn ? 'left-4.5' : 'left-0.5'}"></span>
        </span>
      </button>
      <p class="text-xs text-zinc-400">
        {liveOn
          ? "Status dan hasil partai dikirim ke web saat pertandingan berjalan."
          : "Mati. Pertandingan tetap berjalan di komputer ini tanpa dikirim ke web."}
      </p>
    </section>
  </div>

  <!-- Bulk transfer -->
  <section class="mt-3 bg-[#181a20] rounded-2xl border border-white/[0.08] p-4 flex flex-col md:flex-row md:items-center justify-between gap-3 shrink-0" aria-labelledby="sync-bulk-title">
    <div class="min-w-0">
      <h3 id="sync-bulk-title" class="text-sm font-bold text-white">Kirim dan tarik data</h3>
      <p class="text-xs text-zinc-400 mt-0.5">
        {isLinked ? `${bagans.length} bagan di event ini` : "Hubungkan event web dulu."}
      </p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <button
        type="button"
        onclick={handlePullAthletes}
        disabled={isPulling}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 disabled:opacity-50 cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-sky-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
        <span>{isPulling ? "Menarik…" : "Tarik atlet"}</span>
      </button>
      <button
        type="button"
        onclick={() => (isLinked ? (showPushBaganModal = true) : openLinkEventModal())}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" /></svg>
        <span>Kirim bagan</span>
      </button>
      <button
        type="button"
        onclick={() => (isLinked ? (showForceSyncModal = true) : openLinkEventModal())}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 cursor-pointer"
      >
        <svg class="w-3.5 h-3.5 text-rose-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
        <span>Kirim ulang semua hasil</span>
      </button>
    </div>
  </section>

  <!-- Queue -->
  <div class="flex items-center gap-2.5 mt-6 shrink-0">
    <h3 class="text-sm font-bold text-white">Antrean kirim</h3>
    <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">{queueStats.total_count}</span>
  </div>

  <div class="grid grid-cols-2 sm:grid-cols-4 gap-2 my-3 shrink-0" role="radiogroup" aria-label="Filter antrean">
    {#each statusTabs as tab (tab.key)}
      {@const selected = statusFilter === tab.key}
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        onclick={() => (statusFilter = tab.key)}
        class="text-left rounded-xl px-3 py-2.5 border transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {selected
          ? 'bg-white/[0.08] border-white/50'
          : tab.warn
            ? 'bg-[#181a20] border-rose-500/35 hover:border-rose-500/55'
            : 'bg-[#181a20] border-white/[0.08] hover:border-white/20'}"
      >
        <span class="flex items-center gap-1.5 text-xs font-semibold {tab.warn ? 'text-rose-300' : 'text-zinc-400'}">
          {#if tab.dot}<span class="w-2 h-2 rounded-full {tab.dot}" aria-hidden="true"></span>{/if}
          {tab.label}
        </span>
        <span class="block mt-1 text-2xl font-black font-mono tabular-nums {tab.warn ? 'text-rose-300' : 'text-white'}">{tab.count}</span>
      </button>
    {/each}
  </div>

  <div class="flex flex-wrap items-center gap-2 mb-3 shrink-0">
    <div class="relative flex-1 min-w-[220px]">
      <svg class="w-4 h-4 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
      <input
        type="text"
        bind:value={searchKeyword}
        placeholder="Cari nomor item, endpoint, pemenang, atau error"
        aria-label="Cari antrean"
        class="w-full h-9 bg-[#181a20] text-sm text-white pl-9 pr-9 rounded-xl border border-white/[0.1] focus:border-white outline-none transition-colors placeholder-zinc-500 select-text"
      />
      {#if searchKeyword}
        <button
          type="button"
          onclick={() => (searchKeyword = "")}
          aria-label="Hapus pencarian"
          class="absolute right-1.5 top-1/2 -translate-y-1/2 w-7 h-7 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] cursor-pointer"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
        </button>
      {/if}
    </div>
    {#if queueStats.failed_count > 0}
      <button
        type="button"
        onclick={handleRetryAllFailed}
        disabled={isRetryingAll}
        class="h-9 px-3.5 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl transition-colors shadow-md disabled:opacity-50 cursor-pointer"
      >
        {isRetryingAll ? "Mengirim…" : `Kirim ulang yang gagal (${queueStats.failed_count})`}
      </button>
    {/if}
    {#if queueStats.synced_count > 0}
      <button
        type="button"
        onclick={handleClearSynced}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors cursor-pointer"
      >
        Bersihkan yang terkirim
      </button>
    {/if}
  </div>

  <div class="rounded-2xl border border-white/[0.08] bg-[#181a20] overflow-x-auto shrink-0">
    <table class="w-full text-left text-sm">
      <thead class="bg-[#111215] text-zinc-400 text-xs font-semibold border-b border-white/[0.08]">
        <tr>
          <th scope="col" class="px-4 py-2.5 w-16">No.</th>
          <th scope="col" class="px-4 py-2.5">Data</th>
          <th scope="col" class="px-4 py-2.5 w-28">Status</th>
          <th scope="col" class="px-4 py-2.5 w-24">Percobaan</th>
          <th scope="col" class="px-4 py-2.5">Error</th>
          <th scope="col" class="px-4 py-2.5 w-48 text-right">Aksi</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-white/[0.04] select-text">
        {#if filteredQueue.length === 0}
          <tr>
            <td colspan="6" class="py-10 text-center text-zinc-500 text-sm">
              {searchKeyword.trim() ? "Tidak ada item yang cocok." : statusFilter === "all" ? "Antrean kosong." : "Tidak ada item dengan status ini."}
            </td>
          </tr>
        {:else}
          {#each filteredQueue as item (item.id)}
            {@const st = statusInfo(item.status)}
            <tr class="hover:bg-white/[0.02] transition-colors align-top">
              <td class="px-4 py-2.5 font-mono text-xs text-zinc-500 tabular-nums">#{item.id}</td>
              <td class="px-4 py-2.5">
                <span class="font-mono text-xs text-zinc-200">/{item.endpoint}</span>
                {#if item.ring || (item.round && item.urutan) || item.score || item.winner}
                  <span class="block text-xs text-zinc-400 mt-0.5">
                    {[item.ring ? `Tatami ${item.ring}` : "", item.round && item.urutan ? `Babak ${item.round} partai ${item.urutan}` : "", item.score || ""].filter(Boolean).join(" · ")}
                    {#if item.winner}<span class="text-zinc-200"> · {item.winner}</span>{/if}
                  </span>
                {/if}
              </td>
              <td class="px-4 py-2.5">
                <span class="inline-flex items-center gap-1.5 text-xs font-semibold {st.cls}">
                  <span class="w-2 h-2 rounded-full {st.dot}" aria-hidden="true"></span>
                  {st.label}
                </span>
              </td>
              <td class="px-4 py-2.5 font-mono text-xs text-zinc-400 tabular-nums">{item.retry_count > 0 ? `${item.retry_count}×` : "–"}</td>
              <td class="px-4 py-2.5 text-xs font-mono text-rose-300 max-w-xs">
                <span class="block truncate" title={item.last_error || ""}>{item.last_error || ""}</span>
                {#if !item.last_error}<span class="text-zinc-600">–</span>{/if}
              </td>
              <td class="px-4 py-2 select-none">
                <div class="flex items-center justify-end gap-1">
                  {#if item.status === "failed" || item.status === "pending"}
                    <button
                      type="button"
                      onclick={() => handleRetrySingle(item)}
                      aria-label={`Kirim item #${item.id}`}
                      class="h-8 px-2.5 bg-white/[0.06] hover:bg-white/[0.12] text-white font-semibold text-xs rounded-lg transition-colors cursor-pointer"
                    >
                      Kirim
                    </button>
                    <button
                      type="button"
                      onclick={() => handleMarkSynced(item)}
                      aria-label={`Lewati item #${item.id}`}
                      title="Tandai terkirim tanpa mengirim"
                      class="h-8 px-2.5 text-zinc-400 hover:text-white hover:bg-white/[0.08] font-semibold text-xs rounded-lg transition-colors cursor-pointer"
                    >
                      Lewati
                    </button>
                  {/if}
                  <button
                    type="button"
                    onclick={() => openPayloadModal(item)}
                    aria-label={`Lihat data item #${item.id}`}
                    title="Lihat data (JSON)"
                    class="w-9 h-9 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" /></svg>
                  </button>
                  <button
                    type="button"
                    onclick={() => handleDeleteSingle(item)}
                    aria-label={`Hapus item #${item.id}`}
                    title="Hapus dari antrean"
                    class="w-9 h-9 flex items-center justify-center rounded-lg text-zinc-500 hover:text-rose-300 hover:bg-rose-500/10 transition-colors cursor-pointer"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
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

<!-- ================= MODALS ================= -->

<!-- Modal Link Event -->
{#if showLinkModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div role="dialog" aria-modal="true" class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-md shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white">Hubungkan event web</h3>
        <button type="button" onclick={() => (showLinkModal = false)} aria-label="Tutup" class="w-9 h-9 -m-2 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"><svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg></button>
      </div>

      <div class="space-y-3">
        <div class="bg-[#111215] p-3 rounded-xl border border-white/[0.06] text-xs space-y-1">
          <div class="flex justify-between">
            <span class="text-zinc-500">Server</span>
            <span class="text-zinc-200 font-bold">{syncConfig.label}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-zinc-500">Event lokal</span>
            <span class="text-zinc-200 font-bold">{activeEvent?.nama_event}</span>
          </div>
        </div>

        {#if isLoadingHostedEvents}
          <div class="py-4 text-center text-xs text-zinc-500">Mengambil daftar event dari web…</div>
        {:else if hostedEvents.length === 0 && authStatus.mode === "license"}
          <p class="text-xs text-amber-200 bg-amber-500/[0.06] border border-amber-500/30 rounded-xl p-3 leading-relaxed">
            Belum ada event web yang diizinkan untuk lisensi ini. Minta admin Ambrilindo menambahkan event di halaman Lisensi Desktop, lalu buka dialog ini lagi.
          </p>
        {:else}
          <div>
            <div class="text-xs font-semibold text-zinc-300 mb-1">Event di web</div>
            <FuzzySelect
              items={hostedEventOptions}
              bind:value={selectedHostedId}
              placeholder="Pilih event"
              allowClear={true}
              clearLabel="Kosongkan pilihan"
              clearValue={null}
              class="w-full"
            />
          </div>
        {/if}
      </div>

      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={() => (showLinkModal = false)} class="px-4 py-2 text-xs font-semibold text-zinc-400">Batal</button>
        <button onclick={handleSaveEventMapping} class="bg-white text-black font-bold text-xs px-4 py-2 rounded-xl transition hover:bg-zinc-200">
          Hubungkan
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Push Bagan -->
{#if showPushBaganModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div role="dialog" aria-modal="true" class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-md shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white">Kirim bagan ke web</h3>
        <button type="button" onclick={() => (showPushBaganModal = false)} aria-label="Tutup" class="w-9 h-9 -m-2 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"><svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg></button>
      </div>

      <div class="space-y-3">
        <label class="p-3 rounded-xl border border-white/[0.08] bg-[#111215] flex items-center gap-3 cursor-pointer has-[:checked]:border-white/40">
          <input type="radio" name="push_scope" value="all" bind:group={pushScope} class="accent-white cursor-pointer" />
          <div>
            <div class="text-xs font-bold text-white">Semua bagan ({bagans.length})</div>
          </div>
        </label>

        <label class="p-3 rounded-xl border border-white/[0.08] bg-[#111215] flex items-center gap-3 cursor-pointer has-[:checked]:border-white/40">
          <input type="radio" name="push_scope" value="selected" bind:group={pushScope} class="accent-white cursor-pointer" />
          <div>
            <div class="text-xs font-bold text-white">Pilih bagan</div>
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
          {isPushing ? "Mengirim…" : "Kirim bagan"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Force Sync -->
{#if showForceSyncModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div role="dialog" aria-modal="true" class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-sm shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white">Kirim ulang semua hasil?</h3>
        <button type="button" onclick={() => (showForceSyncModal = false)} aria-label="Tutup" class="w-9 h-9 -m-2 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"><svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg></button>
      </div>
      <p class="text-xs text-zinc-300 leading-relaxed">
        Semua hasil partai yang sudah selesai di komputer ini dikirim lagi ke web. Dipakai bila hasil di web tertinggal atau berbeda.
      </p>
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={() => (showForceSyncModal = false)} class="px-4 py-2 text-xs font-semibold text-zinc-400">Batal</button>
        <button onclick={handleConfirmForceSync} disabled={isForceSyncing} class="bg-white text-black hover:bg-zinc-200 font-bold text-xs px-5 py-2 rounded-xl transition disabled:opacity-50">
          {isForceSyncing ? "Mengirim…" : "Kirim ulang hasil"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Payload JSON -->
{#if showPayloadModal}
  <div class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div role="dialog" aria-modal="true" class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-lg shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white">Data item #{selectedPayloadId}</h3>
        <button type="button" onclick={() => (showPayloadModal = false)} aria-label="Tutup" class="w-9 h-9 -m-2 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"><svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg></button>
      </div>
      <pre class="bg-[#111215] p-3 rounded-xl border border-white/[0.08] font-mono text-xs text-zinc-200 overflow-auto max-h-80 select-text whitespace-pre-wrap">{selectedPayloadJson}</pre>
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-white/[0.08]">
        <button onclick={copyPayloadJson} class="bg-zinc-800 hover:bg-zinc-700 text-white font-bold text-xs px-3 py-1.5 rounded-lg border border-white/[0.08] transition">
          Salin
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
    <div role="dialog" aria-modal="true" class="bg-[#181a20] rounded-2xl border border-white/[0.12] w-full max-w-sm shadow-2xl p-6 space-y-4">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white">Server kustom</h3>
        <button type="button" onclick={() => (showCustomUrlModal = false)} aria-label="Tutup" class="w-9 h-9 -m-2 flex items-center justify-center rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors cursor-pointer"><svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg></button>
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

<!-- Reusable Bulletproof Confirmation Modal (Zero Browser confirm()) -->
<ConfirmModal
  open={confirmModalConfig.open}
  title={confirmModalConfig.title}
  message={confirmModalConfig.message}
  confirmText={confirmModalConfig.confirmText}
  cancelText={confirmModalConfig.cancelText}
  isDestructive={confirmModalConfig.isDestructive}
  isLoading={confirmModalConfig.isLoading}
  onConfirm={confirmModalConfig.onConfirm}
  onCancel={closeConfirmModal}
/>
