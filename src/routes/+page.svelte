<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, setConnectionConfig, withHostParam } from "$lib/api_client";
  import { initLiveSync, subscribeBracketUpdates } from "$lib/liveSync";

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
  import SummaryBaganView from "$lib/components/SummaryBaganView.svelte";
  import NomorTandingView from "$lib/components/NomorTandingView.svelte";
  import TimetableEditorView from "$lib/components/TimetableEditorView.svelte";
  import FestivalRosterView from "$lib/components/FestivalRosterView.svelte";
  import WasitManagerView from "$lib/components/WasitManagerView.svelte";
  import ContingentManagerView from "$lib/components/ContingentManagerView.svelte";
  import PerguruanManagerView from "$lib/components/PerguruanManagerView.svelte";
  import ResultsBookletView from "$lib/components/ResultsBookletView.svelte";
  import PrintCenterView from "$lib/components/PrintCenterView.svelte";
  import MonitorPickerModal from "$lib/components/MonitorPickerModal.svelte";
  import PortConflictModal from "$lib/components/PortConflictModal.svelte";
  import QrCodeModal from "$lib/components/QrCodeModal.svelte";
  import AddEventModal from "$lib/components/AddEventModal.svelte";
  import DrawingBaganModal from "$lib/components/DrawingBaganModal.svelte";
  import DrawingFestivalModal from "$lib/components/DrawingFestivalModal.svelte";
  import VenueWebPortal from "$lib/components/VenueWebPortal.svelte";

  // Application Flow State
  let appMode = $state<AppMode>("landing");
  let hostUrl = $state<string>("http://127.0.0.1:8000");
  let isBrowserClient = $state(false);

  // Discord Shell Active State
  let activeRole = $state<string>("admin_utama");

  // The role is remembered per event (server): coming back to an event restores the
  // role last used there, and a role whose tatami does not exist in the event is
  // dropped back to Admin Utama.
  const ROLES_STORAGE_KEY = "ambrilindo_roles_by_event";
  let rolesByEvent: Record<number, string> = {};

  function persistRolesByEvent() {
    try {
      sessionStorage.setItem(ROLES_STORAGE_KEY, JSON.stringify(rolesByEvent));
    } catch (_) {}
  }

  function roleLabel(role: string): string {
    return role.startsWith("admin_tatami_")
      ? `Admin Tatami ${role.replace("admin_tatami_", "")}`
      : "Admin Utama";
  }

  function isRoleValidForTatamis(role: string, list: TatamiItem[]): boolean {
    if (!role.startsWith("admin_tatami_")) return true;
    const num = parseInt(role.replace("admin_tatami_", ""), 10);
    return list.some((t) => (t.tatami_number || 1) === num);
  }

  function setRole(role: string) {
    activeRole = role;
    if (selectedEventId) {
      rolesByEvent[selectedEventId] = role;
      persistRolesByEvent();
    }
  }

  /** Applies the remembered role for `eventId`, validated against its tatamis. */
  function applyRoleForEvent(eventId: number, eventChanged: boolean) {
    const remembered = rolesByEvent[eventId] ?? (eventChanged ? "admin_utama" : activeRole);
    const previous = activeRole;
    let next = remembered;
    if (!isRoleValidForTatamis(remembered, tatamis)) {
      next = "admin_utama";
      showToast(`${roleLabel(remembered)} tidak ada di event ini. Role kembali ke Admin Utama.`);
    } else if (eventChanged && next !== previous) {
      showToast(`Role di event ini: ${roleLabel(next)}`);
    }
    activeRole = next;
    rolesByEvent[eventId] = next;
    persistRolesByEvent();
  }

  const DEFAULT_CHANNEL: ActiveChannel = {
    type: "tatami",
    id: "tatami-manager",
    name: "tatami",
    subtitle: "Manajemen Tatami Turnamen",
  };

  /** A bracket channel from another event must not stay open after switching. */
  function validateActiveChannel() {
    if (activeChannel.type !== "bagan") return;
    const id = activeChannel.id;
    if (typeof id === "number" && id !== 0 && !bagans.some((b) => b.id === id)) {
      activeChannel = { ...DEFAULT_CHANNEL };
    }
  }
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
  let drawingFestivalModal = $state(false);

  async function handleDrawingFestivalComplete() {
    drawingFestivalModal = false;
    if (selectedEventId) {
      await selectEvent(selectedEventId);
    }
    showToast("Drawing bagan festival berhasil disimpan!");
  }

  function handleServerStopped() {
    isServerRunning = false;
    appMode = "solo";
    setConnectionConfig("solo");
    showToast("Server Host LAN dinonaktifkan. Beralih ke Mode Offline.");
  }

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

  async function handleSelectMode(mode: AppMode, remoteUrl?: string, selectedIp?: string) {
    if (mode === "solo") {
      appMode = "solo";
      setConnectionConfig("solo");
      showToast("Mode Offline Aktif");
      if (!databaseSummary) {
        await autoMountDefaultDb();
      }
    } else if (mode === "host") {
      appMode = "host";
      await startHostServer(selectedIp);
      if (!databaseSummary) {
        await autoMountDefaultDb();
      }
    } else if (mode === "client" && remoteUrl) {
      appMode = "client";
      hostUrl = remoteUrl;
      setConnectionConfig("client", remoteUrl);
      initLiveSync(remoteUrl);
      await connectToRemoteHost(remoteUrl);
    }
  }

  async function startHostServer(selectedIp?: string) {
    isStartingServer = true;
    try {
      const info = await invoke<NetworkInfo>("start_arena_server", { selectedIp: selectedIp || null });
      networkInfo = info;
      isServerRunning = true;
      portErrorModal = false;
      hostUrl = info.server_url;
      setConnectionConfig("host", info.server_url);
      showToast(`Server Host LAN aktif di ${info.server_url}`);
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
      setConnectionConfig("client", remoteUrl);
      const evList = await invoke<EventItem[]>("get_events");
      if (evList && evList.length > 0) {
        selectedEventId = evList[0].id;
        databaseSummary = {
          db_path: `Remote Host (${remoteUrl})`,
          media_path: `${remoteUrl}/media`,
          events: evList,
        };
        await selectEvent(evList[0].id);
      }
      showToast("Terhubung ke Host Turnamen");
    } catch (e: any) {
      alert("Gagal memuat data dari host: " + (e.message || e));
      appMode = "landing";
      setConnectionConfig("solo");
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
    return result;
  }

  // Increments on every selectEvent call so a slow response for an event the user
  // already left cannot overwrite the data of the event now on screen.
  let eventLoadSeq = 0;

  async function selectEvent(eventId: number) {
    const seq = ++eventLoadSeq;
    const eventChanged = selectedEventId !== eventId;
    selectedEventId = eventId;
    try {
      const [tatamiList, baganList] = await Promise.all([
        invoke<TatamiItem[]>("get_tatamis", { eventId }),
        invoke<BaganItem[]>("get_bagans", { eventId }),
      ]);
      if (seq !== eventLoadSeq) return;
      tatamis = tatamiList;
      bagans = withFestivalBagan(baganList, eventId);
    } catch (err) {
      if (seq !== eventLoadSeq) return;
      console.error("Gagal memuat data event:", err);
      // Never keep showing the previous event's tatamis and brackets under this one.
      tatamis = [];
      bagans = withFestivalBagan([], eventId);
      showToast("Gagal memuat data event. Coba pilih event lagi.");
    }
    applyRoleForEvent(eventId, eventChanged);
    validateActiveChannel();
  }

  /** After an event was deleted in Data Master: drop it from the rail and move on. */
  async function handleEventDeleted(deletedId: number) {
    if (databaseSummary) {
      databaseSummary = {
        ...databaseSummary,
        events: (databaseSummary.events || []).filter((e) => e.id !== deletedId),
      };
    }
    delete rolesByEvent[deletedId];
    persistRolesByEvent();
    activeChannel = { ...DEFAULT_CHANNEL };
    const next = databaseSummary?.events?.[0];
    if (next) {
      await selectEvent(next.id);
    } else {
      eventLoadSeq++;
      selectedEventId = null;
      tatamis = [];
      bagans = [];
      activeRole = "admin_utama";
      try {
        sessionStorage.removeItem("ambrilindo_event_id");
      } catch (_) {}
    }
  }

  /**
   * Re-reads the event list. Events can change outside this window (another laptop on the
   * host, a seed script, a restored backup), and the list is otherwise loaded only once.
   */
  async function refreshEventList(): Promise<EventItem[] | null> {
    if (!databaseSummary) return null;
    try {
      const list = await invoke<EventItem[]>("get_events");
      if (databaseSummary) databaseSummary = { ...databaseSummary, events: list };
      return list;
    } catch (err) {
      console.warn("Gagal memperbarui daftar event:", err);
      return null;
    }
  }

  /** Refresh the list; if the open event no longer exists, move to the first remaining one. */
  async function syncEventList() {
    const list = await refreshEventList();
    if (!list || selectedEventId === null) return;
    if (!list.some((e) => e.id === selectedEventId)) {
      await handleEventDeleted(selectedEventId);
      showToast("Event yang dibuka sudah tidak ada di database. Daftar event diperbarui.");
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
            withHostParam(`/control-panel?matchId=0&baganId=0&isFestival=true&tatami=${assignedTatami}`),
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

  let isSessionLoaded = $state(false);

  onMount(async () => {
    isBrowserClient = typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__;
    initLiveSync(hostUrl);
    try {
      networkInfo = await invoke<NetworkInfo>("get_network_info");
    } catch (_) {}
    await autoMountDefaultDb();

    // 1. Authoritative check: Is the arena server already running in Rust process?
    let serverRunning = false;
    try {
      serverRunning = await invoke<boolean>("get_server_status");
    } catch (_) {}

    if (serverRunning) {
      appMode = "host";
      isServerRunning = true;
      if (networkInfo?.server_url) {
        hostUrl = networkInfo.server_url;
      }
      setConnectionConfig("host", hostUrl);
      showToast("Melanjutkan sesi server host aktif");
    } else {
      // 2. Restore previous session if working in solo or client mode
      try {
        const savedMode = sessionStorage.getItem("ambrilindo_app_mode") as AppMode | null;
        const savedHost = sessionStorage.getItem("ambrilindo_host_url");
        if (savedHost) {
          hostUrl = savedHost;
        }
        if (savedMode && savedMode !== "landing") {
          setConnectionConfig(savedMode, hostUrl);
          appMode = savedMode;
        }
      } catch (_) {}
    }

    try {
      const savedEventId = sessionStorage.getItem("ambrilindo_event_id");
      const savedChannel = sessionStorage.getItem("ambrilindo_active_channel");
      const savedRole = sessionStorage.getItem("ambrilindo_active_role");
      const savedRoles = sessionStorage.getItem(ROLES_STORAGE_KEY);

      if (savedRoles) {
        try {
          rolesByEvent = { ...rolesByEvent, ...JSON.parse(savedRoles) };
        } catch (_) {}
      }
      const eid = savedEventId ? parseInt(savedEventId, 10) : NaN;
      // Sessions saved before roles were remembered per event.
      if (!isNaN(eid) && savedRole && rolesByEvent[eid] === undefined) {
        rolesByEvent[eid] = savedRole;
      }
      if (savedChannel) {
        try {
          activeChannel = JSON.parse(savedChannel);
        } catch (_) {}
      }
      // Skip an event that no longer exists (deleted or re-created under a new id meanwhile);
      // the first event selected by autoMountDefaultDb stays open instead.
      const eventStillExists = !databaseSummary || databaseSummary.events.some((e) => e.id === eid);
      if (!isNaN(eid) && eventStillExists) {
        // Also validates the restored role and channel against this event.
        await selectEvent(eid);
      }
    } catch (_) {}

    isSessionLoaded = true;
  });

  // Brackets drawn, deleted or edited on another laptop: refresh this event's bracket list.
  $effect(() => {
    if (!isSessionLoaded) return;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const unsub = subscribeBracketUpdates(() => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        if (selectedEventId) selectEvent(selectedEventId);
      }, 600);
    });
    return () => {
      unsub();
      if (timer) clearTimeout(timer);
    };
  });

  // Pick up events added or removed elsewhere whenever the window comes back to the front.
  $effect(() => {
    if (!isSessionLoaded) return;
    const onWindowFocus = () => {
      if (document.visibilityState === "visible") syncEventList();
    };
    window.addEventListener("focus", onWindowFocus);
    document.addEventListener("visibilitychange", onWindowFocus);
    return () => {
      window.removeEventListener("focus", onWindowFocus);
      document.removeEventListener("visibilitychange", onWindowFocus);
    };
  });

  $effect(() => {
    if (!isSessionLoaded) return;
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
  {#if isBrowserClient}
    <!-- Dedicated Web Browser Venue Mobile & Tablet Portal (auth.html inspired) -->
    <VenueWebPortal />
  {:else if appMode === "landing"}
    <!-- Mode Picker / Gateway Landing Screen for Desktop App -->
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
      onReturnToLanding={() => {
        appMode = "landing";
        setConnectionConfig("solo");
      }}
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
        setRole(role);
        showToast(`Role aktif: ${roleLabel(role)}`);
      }}
      onSelectChannel={handleSelectChannel}
      onOpenQrModal={() => (qrModal = true)}
      onOpenDrawingModal={() => (drawingModal = true)}
      onOpenDrawingFestivalModal={() => (drawingFestivalModal = true)}
      onFestivalFormatChanged={async (format) => {
        if (format === "bagan" && activeChannel.type === "festival") activeChannel = { ...DEFAULT_CHANNEL };
        if (selectedEventId) await selectEvent(selectedEventId);
      }}
      onNotify={showToast}
      onReturnToLanding={() => {
        appMode = "landing";
        setConnectionConfig("solo");
      }}
    />

    <!-- 3. Channel Main Workspace -->
    <div class="workspace-column flex-1 flex flex-col min-w-0 bg-[#14161b] overflow-hidden">
      <!-- Top Navbar (Channel Header) -->
      <TopNavbar
        {activeChannel}
        eventName={activeEvent?.nama_event || ""}
        onServerStopped={handleServerStopped}
        onShowToast={showToast}
      />

      <!-- Main Workspace Views -->
      <main class="main-content flex-1 overflow-hidden relative bg-[#14161b]">
        {#if activeChannel.type === "tatami"}
          <!-- Tatami CRUD Manager View -->
          <TatamiManagerView
            eventId={selectedEventId || 1}
            {tatamis}
            hostAddress={appMode === "host" ? (networkInfo?.server_url || "http://127.0.0.1:8000") : hostUrl}
            onRefreshTatamis={() => selectedEventId && selectEvent(selectedEventId)}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "bagan"}
          <!-- Visual Bracket Tree View -->
          <BaganView
            baganId={typeof activeChannel.id === "number" ? activeChannel.id : (bagans[0]?.id || 1)}
            baganName={activeChannel.name}
            baganInfo={bagans.find((b) => b.id === (typeof activeChannel.id === "number" ? activeChannel.id : (bagans[0]?.id || 1)))}
            eventName={activeEvent?.nama_event || ""}
            hostUrl={hostUrl}
            {tatamis}
            {activeRole}
            onBaganUpdated={() => selectedEventId && selectEvent(selectedEventId)}
          />
        {:else if activeChannel.type === "sync"}
          <!-- Web Sync Monitor View -->
          <SyncMonitorView
            eventId={selectedEventId || 1}
            {activeEvent}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "contingent"}
          <!-- Contingents & Utusan Management View -->
          <ContingentManagerView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
            onNavigate={(channel) => handleSelectChannel(channel)}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "perguruan"}
          <!-- Perguruan Karate Management View -->
          <PerguruanManagerView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
            onNavigate={(channel) => handleSelectChannel(channel)}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "roster"}
          <!-- Athletes Roster View -->
          <RosterView
            viewType="roster"
            eventId={selectedEventId || 1}
            hostAddress={hostUrl}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "master"}
          <!-- Tournament Data Master & Reset Control View -->
          <DataMasterView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            onRefreshEvent={() => selectedEventId && selectEvent(selectedEventId)}
            onEventDeleted={handleEventDeleted}
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
        {:else if activeChannel.type === "summary_bagan"}
          <!-- Summary Bagan (Daftar Juara Seluruh Bagan) -->
          <SummaryBaganView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
          />
        {:else if activeChannel.type === "nomor_tanding"}
          <!-- Nomor Tanding (Competition Categories & Division Management) -->
          <NomorTandingView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
            onNavigate={(channel) => handleSelectChannel(channel)}
            onOpenDrawingModal={() => (drawingModal = true)}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "timetable"}
          <!-- Timetable Editor View -->
          <TimetableEditorView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "festival"}
          <!-- Festival Roster Maker View -->
          <FestivalRosterView
            eventId={selectedEventId || 1}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "wasit"}
          <!-- Referee & Tatami Panel Assigner View -->
          <WasitManagerView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            {tatamis}
            hostAddress={hostUrl}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "cetak"}
          <PrintCenterView
            eventId={selectedEventId || 1}
            eventName={activeEvent?.nama_event || ""}
            hostAddress={hostUrl}
            onShowToast={showToast}
          />
        {:else if activeChannel.type === "buku_hasil"}
          <!-- Results Booklet & Official Tournament Book View (In-App) -->
          <ResultsBookletView
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

  {#if drawingFestivalModal && selectedEventId}
    <DrawingFestivalModal
      eventId={selectedEventId}
      eventName={activeEvent?.nama_event || 'Event'}
      onClose={() => (drawingFestivalModal = false)}
      onComplete={handleDrawingFestivalComplete}
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
