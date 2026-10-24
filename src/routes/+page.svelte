<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  import type {
    ActiveChannel,
    AppMode,
    BaganItem,
    DatabaseSummary,
    EventItem,
    MonitorOption,
    NetworkInfo,
    TatamiItem,
  } from "$lib/types";

  import LandingGateway from "$lib/components/LandingGateway.svelte";
  import ServerSidebar from "$lib/components/ServerSidebar.svelte";
  import ChannelSidebar from "$lib/components/ChannelSidebar.svelte";
  import TopNavbar from "$lib/components/TopNavbar.svelte";
  import TatamiManagerView from "$lib/components/TatamiManagerView.svelte";
  import BaganView from "$lib/components/BaganView.svelte";
  import RosterView from "$lib/components/RosterView.svelte";
  import SyncMonitorView from "$lib/components/SyncMonitorView.svelte";
  import DataMasterView from "$lib/components/DataMasterView.svelte";
  import SummaryJuaraView from "$lib/components/SummaryJuaraView.svelte";
  import MonitorPickerModal from "$lib/components/MonitorPickerModal.svelte";
  import PortConflictModal from "$lib/components/PortConflictModal.svelte";
  import QrCodeModal from "$lib/components/QrCodeModal.svelte";
  import AddEventModal from "$lib/components/AddEventModal.svelte";
  import DrawingBaganModal from "$lib/components/DrawingBaganModal.svelte";

  // Application Flow State
  let appMode = $state<AppMode>("landing");
  let hostUrl = $state<string>("http://127.0.0.1:8000");

  // Discord Shell Active State
  let activeRole = $state<string>("admin_utama");
  let activeChannel = $state<ActiveChannel>({
    type: "tatami",
    id: "tatami-manager",
    name: "tatami",
    subtitle: "Manajemen Tatami Turnamen",
  });

  // Tournament Data State
  let databaseSummary = $state<DatabaseSummary | null>(null);
  let selectedEventId = $state<number | null>(null);
  let tatamis = $state<TatamiItem[]>([]);
  let bagans = $state<BaganItem[]>([]);

  // Server & LAN State
  let networkInfo = $state<NetworkInfo | null>(null);
  let isServerRunning = $state(false);
  let isStartingServer = $state(false);

  // Modals State
  let portErrorModal = $state(false);
  let portErrorMessage = $state("");
  let monitorPickerModal = $state(false);
  let qrModal = $state(false);
  let availableMonitors = $state<MonitorOption[]>([]);
  let selectedMonitorId = $state<number>(0);
  let selectedTatamiForProjection = $state<number>(1);
  let isProjecting = $state(false);
  let addEventModal = $state(false);
  let isCreatingEvent = $state(false);
  let drawingModal = $state(false);

  // Toast State
  let toastMessage = $state<string | null>(null);

  function showToast(msg: string) {
    toastMessage = msg;
    setTimeout(() => {
      if (toastMessage === msg) toastMessage = null;
    }, 3000);
  }

  async function handleDrawingComplete() {
    if (selectedEventId) {
      await selectEvent(selectedEventId);
    }
    showToast("Drawing bagan turnamen berhasil disimpan!");
  }

  // --- Data Loading & Mode Handlers ---

  async function handleSelectMode(mode: AppMode, remoteUrl?: string) {
    if (mode === "solo") {
      appMode = "solo";
      showToast("Mode Offline Aktif");
      if (!databaseSummary) {
        await autoMountDefaultDb();
      }
    } else if (mode === "host") {
      appMode = "host";
      await startHostServer();
      if (!databaseSummary) {
        await autoMountDefaultDb();
      }
    } else if (mode === "client" && remoteUrl) {
      appMode = "client";
      hostUrl = remoteUrl;
      await connectToRemoteHost(remoteUrl);
    }
  }

  async function startHostServer() {
    isStartingServer = true;
    try {
      const info = await invoke<NetworkInfo>("start_arena_server");
      networkInfo = info;
      isServerRunning = true;
      portErrorModal = false;
      hostUrl = info.server_url;
      showToast("Server Host LAN aktif di port 8000");
    } catch (err: any) {
      portErrorMessage = typeof err === "string" ? err : JSON.stringify(err);
      portErrorModal = true;
      isServerRunning = false;
    } finally {
      isStartingServer = false;
    }
  }

  async function connectToRemoteHost(remoteUrl: string) {
    try {
      showToast(`Menghubungkan ke host ${remoteUrl}...`);
      const eventsRes = await fetch(`${remoteUrl}/api/events`);
      if (eventsRes.ok) {
        const json = await eventsRes.json();
        const evList: EventItem[] = json.data || [];
        if (evList.length > 0) {
          selectedEventId = evList[0].id;
          databaseSummary = {
            db_path: `Remote Host (${remoteUrl})`,
            media_path: `${remoteUrl}/media`,
            events: evList,
          };
          await loadEventDetailsRemote(remoteUrl, evList[0].id);
        }
      }
      showToast("Terhubung ke Host Turnamen");
    } catch (e: any) {
      alert("Gagal memuat data dari host: " + e.message);
      appMode = "landing";
    }
  }

  async function loadEventDetailsRemote(remoteUrl: string, eventId: number) {
    try {
      const [tRes, bRes] = await Promise.all([
        fetch(`${remoteUrl}/api/tatamis/${eventId}`),
        fetch(`${remoteUrl}/api/bagans/${eventId}`),
      ]);
      if (tRes.ok) {
        const tJson = await tRes.json();
        tatamis = tJson.data || [];
      }
      if (bRes.ok) {
        const bJson = await bRes.json();
        bagans = bJson.data || [];
      }
    } catch (err) {
      console.error("Gagal sinkron data remote:", err);
    }
  }

  async function autoMountDefaultDb() {
    try {
      const summary = await invoke<DatabaseSummary | null>("auto_mount_default_database");
      if (summary) {
        databaseSummary = summary;
        if (summary.events && summary.events.length > 0) {
          await selectEvent(summary.events[0].id);
        }
        showToast("Database tournament_data otomatis dimuat");
      }
    } catch (e) {
      console.warn("No default database mounted:", e);
      if (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) {
        await selectEvent(1);
      }
    }
  }

  async function pickAndMountDatabase() {
    try {
      const summary = await invoke<DatabaseSummary | null>("pick_and_mount_database");
      if (summary) {
        databaseSummary = summary;
        if (summary.events && summary.events.length > 0) {
          await selectEvent(summary.events[0].id);
        } else {
          tatamis = [];
          bagans = [];
        }
        showToast("Database berhasil dimuat");
      }
    } catch (err: any) {
      alert("Gagal memuat database: " + err);
    }
  }

  function withFestivalBagan(list: BaganItem[], eventId: number = 1): BaganItem[] {
    const festivalBagan: BaganItem = {
      id: 0,
      event_id: eventId,
      nama_bagan: "Festival",
      nomor_tanding_id: null,
      tipe_tanding: "2",
      juara_1_id: null,
      juara_2_id: null,
      juara_3a_id: null,
      juara_3b_id: null,
      peringkat_5_id: null,
      peringkat_6_id: null,
      peringkat_7_id: null,
      peringkat_8_id: null,
      round_robin: false,
      pool: 1,
      kode: "000",
      is_bob: false,
      has_vr: false,
    };
    const result = [festivalBagan, ...list.filter((b) => b.id !== 0 && b.kode !== "000")];
    if (result.length === 1 && typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) {
      result.push({
        id: 2,
        event_id: eventId,
        nama_bagan: "KATA PERORANGAN KADET PUTRA",
        nomor_tanding_id: 2,
        tipe_tanding: "1",
        juara_1_id: null,
        juara_2_id: null,
        juara_3a_id: null,
        juara_3b_id: null,
        peringkat_5_id: null,
        peringkat_6_id: null,
        peringkat_7_id: null,
        peringkat_8_id: null,
        round_robin: false,
        pool: 1,
        kode: "002",
        is_bob: false,
        has_vr: false,
      });
    }
    return result;
  }

  async function selectEvent(eventId: number) {
    selectedEventId = eventId;
    try {
      const [tatamiList, baganList] = await Promise.all([
        invoke<TatamiItem[]>("get_tatamis", { eventId }),
        invoke<BaganItem[]>("get_bagans", { eventId }),
      ]);
      tatamis = tatamiList;
      bagans = withFestivalBagan(baganList, eventId);
    } catch (err) {
      console.error("Gagal memuat data event:", err);
      if (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) {
        tatamis = [];
        bagans = withFestivalBagan([], eventId);
      }
    }
  }

  async function handleCreateEvent(nama: string, kode?: string) {
    isCreatingEvent = true;
    try {
      const newEv = await invoke<EventItem>("create_event", {
        namaEvent: nama,
        kodeEvent: kode || null,
      });
      if (databaseSummary) {
        databaseSummary = {
          ...databaseSummary,
          events: [newEv, ...(databaseSummary.events || [])],
        };
      }
      await selectEvent(newEv.id);
      addEventModal = false;
      showToast(`Event "${newEv.nama_event}" berhasil dibuat!`);
    } catch (err: any) {
      alert("Gagal menambahkan event: " + err);
    } finally {
      isCreatingEvent = false;
    }
  }

  async function handleSelectChannel(channel: ActiveChannel) {
    if (channel.type === "bagan" && (channel.id === 0 || channel.name === "Festival")) {
      const assignedTatami = activeRole.startsWith("admin_tatami_")
        ? parseInt(activeRole.replace("admin_tatami_", "")) || 1
        : 1;
      try {
        await invoke("open_control_panel_window", {
          matchId: 0,
          baganId: 0,
          tatami: assignedTatami,
        });
      } catch (err) {
        console.warn("Tauri open_control_panel_window failed, fallback window.open:", err);
        if (typeof window !== "undefined" && window.open) {
          window.open(
            `/control-panel?matchId=0&baganId=0&isFestival=true&tatami=${assignedTatami}`,
            "control_panel_0",
            "width=1366,height=820"
          );
        }
      }
      return;
    }
    activeChannel = channel;
  }

  // --- Projections & Modals ---

  async function openMonitorPicker(tatamiNum?: number) {
    selectedTatamiForProjection = tatamiNum ?? 1;
    try {
      const monitors = await invoke<MonitorOption[]>("get_available_monitors");
      availableMonitors = monitors;
      const secondary = monitors.find((m) => !m.is_primary);
      selectedMonitorId = secondary ? secondary.id : (monitors[0]?.id ?? 0);
      monitorPickerModal = true;
    } catch (err) {
      alert("Gagal mendeteksi monitor: " + err);
    }
  }

  async function projectScoreboard() {
    isProjecting = true;
    try {
      await invoke("project_scoreboard", {
        monitorId: selectedMonitorId,
        tatamiNumber: selectedTatamiForProjection,
      });
      monitorPickerModal = false;
      showToast(`Scoreboard Tatami ${selectedTatamiForProjection} diproyeksikan!`);
    } catch (err: any) {
      alert("Gagal membuka proyeksi scoreboard: " + err);
    } finally {
      isProjecting = false;
    }
  }

  let activeEvent = $derived(
    databaseSummary?.events.find((e) => e.id === selectedEventId) ?? null
  );

  onMount(async () => {
    try {
      networkInfo = await invoke<NetworkInfo>("get_network_info");
    } catch (_) {}
    await autoMountDefaultDb();

    // Restore previous session if user was working in solo or host mode
    try {
      const savedMode = sessionStorage.getItem("ambrilindo_app_mode") as AppMode | null;
      const savedEventId = sessionStorage.getItem("ambrilindo_event_id");
      const savedChannel = sessionStorage.getItem("ambrilindo_active_channel");
      const savedRole = sessionStorage.getItem("ambrilindo_active_role");

      if (savedMode && savedMode !== "landing") {
        appMode = savedMode;
        if (savedEventId) {
          const eid = parseInt(savedEventId, 10);
          if (!isNaN(eid)) {
            await selectEvent(eid);
          }
        }
        if (savedChannel) {
          try {
            activeChannel = JSON.parse(savedChannel);
          } catch (_) {}
        }
        if (savedRole) {
          activeRole = savedRole;
        }
      }
    } catch (_) {}
  });

  $effect(() => {
    try {
      sessionStorage.setItem("ambrilindo_app_mode", appMode);
      if (selectedEventId) {
        sessionStorage.setItem("ambrilindo_event_id", selectedEventId.toString());
      }
      sessionStorage.setItem("ambrilindo_active_channel", JSON.stringify(activeChannel));
      sessionStorage.setItem("ambrilindo_active_role", activeRole);
    } catch (_) {}
  });
</script>

<svelte:head>
  <title>Ambrilindo — {activeChannel.name}</title>
</svelte:head>

<div class="app-shell h-screen w-screen bg-[#0d0e12] text-zinc-200 flex overflow-hidden font-sans select-none">
  {#if appMode === "landing"}
    <!-- Mode Picker / Gateway Landing Screen -->
    <LandingGateway
      currentDb={databaseSummary}
      {networkInfo}
      onSelectMode={handleSelectMode}
      onPickDatabase={pickAndMountDatabase}
    />
  {:else}
    <!-- Discord 1:1 Workspace Layout -->

    <!-- 1. Leftmost Server Rail (72px) - Events ONLY as servers -->
    <ServerSidebar
      events={databaseSummary?.events || []}
      {selectedEventId}
      onSelectEvent={(id) => selectEvent(id)}
      onAddEvent={() => (addEventModal = true)}
      onReturnToLanding={() => (appMode = "landing")}
    />

    <!-- 2. Channel Sidebar (256px) - Matching base-dashboard.html -->
    <ChannelSidebar
      {activeChannel}
      mode={appMode}
      {activeEvent}
      {tatamis}
      {bagans}
      hostAddress={appMode === "host" ? (networkInfo?.server_url || "Port 8000") : hostUrl}
      {activeRole}
      onSelectRole={(role) => {
        activeRole = role;
        const label = role.startsWith("admin_tatami_")
          ? `Admin Tatami ${role.replace("admin_tatami_", "")}`
          : "Admin Utama";
        showToast(`Role aktif: ${label}`);
      }}
      onSelectChannel={handleSelectChannel}
      onOpenQrModal={() => (qrModal = true)}
      onOpenDrawingModal={() => (drawingModal = true)}
      onReturnToLanding={() => (appMode = "landing")}
    />

    <!-- 3. Channel Main Workspace -->
    <div class="workspace-column flex-1 flex flex-col min-w-0 bg-[#14161b] overflow-hidden">
      <!-- Top Navbar (Channel Header) -->
      <TopNavbar {activeChannel} />

      <!-- Main Workspace Views -->
      <main class="main-content flex-1 overflow-hidden relative bg-[#14161b]">
        {#if activeChannel.type === "tatami"}
          <!-- Tatami CRUD Manager View -->
          <TatamiManagerView
            eventId={selectedEventId || 1}
            {tatamis}
            hostAddress={appMode === "host" ? (networkInfo?.server_url || "http://127.0.0.1:8000") : hostUrl}
            onRefreshTatamis={() => selectedEventId && selectEvent(selectedEventId)}
            onOpenMonitorPicker={(tNum) => openMonitorPicker(tNum)}
          />
        {:else if activeChannel.type === "bagan"}
          <!-- Visual Bracket Tree View -->
          <BaganView
            baganId={typeof activeChannel.id === "number" ? activeChannel.id : (bagans[0]?.id || 1)}
            baganName={activeChannel.name}
            baganInfo={bagans.find((b) => b.id === (typeof activeChannel.id === "number" ? activeChannel.id : (bagans[0]?.id || 1)))}
            eventName={activeEvent?.nama_event || ""}
            hostUrl={hostUrl}
            onBaganUpdated={() => selectedEventId && selectEvent(selectedEventId)}
          />
        {:else if activeChannel.type === "sync"}
          <!-- Web Sync Monitor View -->
          <SyncMonitorView
            eventId={selectedEventId || 1}
            {activeEvent}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "roster" || activeChannel.type === "contingent"}
          <!-- Athletes & Contingents View -->
          <RosterView
            viewType={activeChannel.type}
            eventId={selectedEventId || 1}
            hostAddress={hostUrl}
          />
        {:else if activeChannel.type === "master"}
          <!-- Tournament Data Master & Reset Control View -->
          <DataMasterView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            onRefreshEvent={() => selectedEventId && selectEvent(selectedEventId)}
            onNavigate={(channel) => handleSelectChannel(channel)}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "summary_juara"}
          <!-- Summary Juara Standings & Medalist View -->
          <SummaryJuaraView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
          />
        {/if}
      </main>
    </div>
  {/if}

  <!-- Modals -->
  {#if monitorPickerModal}
    <MonitorPickerModal
      tatamiNumber={selectedTatamiForProjection}
      {availableMonitors}
      {selectedMonitorId}
      {isProjecting}
      onSelectMonitor={(id) => (selectedMonitorId = id)}
      onConfirmProject={projectScoreboard}
      onClose={() => (monitorPickerModal = false)}
    />
  {/if}

  {#if portErrorModal}
    <PortConflictModal
      errorMessage={portErrorMessage}
      isRetrying={isStartingServer}
      onRetry={startHostServer}
      onClose={() => (portErrorModal = false)}
    />
  {/if}

  {#if qrModal}
    <QrCodeModal
      {networkInfo}
      onClose={() => (qrModal = false)}
      onShowToast={showToast}
    />
  {/if}

  {#if addEventModal}
    <AddEventModal
      isSubmitting={isCreatingEvent}
      onConfirm={handleCreateEvent}
      onClose={() => (addEventModal = false)}
    />
  {/if}

  {#if drawingModal && selectedEventId}
    <DrawingBaganModal
      eventId={selectedEventId}
      eventName={activeEvent?.nama_event || 'Event'}
      onClose={() => (drawingModal = false)}
      onComplete={handleDrawingComplete}
    />
  {/if}

  <!-- Toast Notification Overlay -->
  {#if toastMessage}
    <div class="fixed bottom-6 right-6 z-50 bg-[#181a20] border border-white/[0.12] text-white font-medium text-xs px-4 py-3 rounded-xl shadow-2xl flex items-center space-x-2 animate-in slide-in-from-bottom duration-200">
      <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
      <span>{toastMessage}</span>
    </div>
  {/if}
</div>
