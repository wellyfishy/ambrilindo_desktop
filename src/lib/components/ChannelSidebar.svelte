<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { ActiveChannel, AppMode, BaganItem, EventItem, TatamiItem } from "$lib/types";

  interface MonitorOption {
    id: number;
    name: string;
    width: number;
    height: number;
    is_primary: boolean;
    label: string;
  }

  interface Props {
    activeChannel: ActiveChannel;
    mode: AppMode;
    activeEvent: EventItem | null;
    tatamis: TatamiItem[];
    bagans: BaganItem[];
    hostAddress: string;
    activeRole: string; // "admin_utama" | "admin_tatami_1" | etc.
    onSelectRole: (role: string) => void;
    onSelectChannel: (channel: ActiveChannel) => void;
    onOpenQrModal: () => void;
    onOpenDrawingModal?: () => void;
    onReturnToLanding: () => void;
  }

  let {
    activeChannel,
    mode,
    activeEvent,
    tatamis,
    bagans,
    hostAddress,
    activeRole,
    onSelectRole,
    onSelectChannel,
    onOpenQrModal,
    onOpenDrawingModal,
    onReturnToLanding,
  }: Props = $props();

  let collapseDashboard = $state(false);
  let collapseRekapan = $state(false);
  let collapseAdminTools = $state(false);
  let collapseBagan = $state(false);
  let searchBagan = $state("");
  let isRoleDropdownOpen = $state(false);

  // Scoreboard monitor, flyout state & running scoreboards
  let monitors = $state<MonitorOption[]>([]);
  let runningScoreboards = $state<number[]>([]);
  let isScoreboardMenuOpen = $state(false);
  let hoveredTatami = $state<number | null>(null);
  let scoreboardBtnEl = $state<HTMLElement | null>(null);
  let scoreboardFlyoutPos = $state<{ top: number; left: number }>({ top: 0, left: 330 });
  let closeMenuTimer: any = null;

  function scheduleCloseMenu() {
    if (closeMenuTimer) clearTimeout(closeMenuTimer);
    closeMenuTimer = setTimeout(() => {
      isScoreboardMenuOpen = false;
      hoveredTatami = null;
    }, 300);
  }

  function cancelCloseMenu() {
    if (closeMenuTimer) {
      clearTimeout(closeMenuTimer);
      closeMenuTimer = null;
    }
  }

  function openScoreboardMenu(el: HTMLElement) {
    cancelCloseMenu();
    const rect = el.getBoundingClientRect();
    scoreboardFlyoutPos = {
      top: Math.max(10, rect.top),
      left: rect.right + 4,
    };
    loadMonitors();
    refreshRunningScoreboards();
    isScoreboardMenuOpen = true;
  }

  async function refreshRunningScoreboards() {
    try {
      runningScoreboards = await invoke<number[]>("get_running_scoreboards");
    } catch (_) {}
  }

  async function handleStopScoreboard(tatamiNum: number) {
    try {
      await invoke("close_scoreboard", { tatamiNumber: tatamiNum });
      await refreshRunningScoreboards();
    } catch (err) {
      console.warn("Failed to close scoreboard:", err);
    }
  }

  async function loadMonitors() {
    try {
      monitors = await invoke("get_available_monitors");
    } catch (e) {
      console.warn("Failed to get monitors:", e);
      monitors = [{ id: 0, name: "Display 1", width: 1920, height: 1080, is_primary: true, label: "Monitor 1 (Utama)" }];
    }
  }

  async function handleLaunchScoreboard(tatamiNum: number, monitorId: number) {
    try {
      await invoke("project_scoreboard", { monitorId, tatamiNumber: tatamiNum });
      await refreshRunningScoreboards();
    } catch (err) {
      console.warn("Tauri project_scoreboard failed, fallback to window.open:", err);
      window.open(`/scoreboard?tatami=${tatamiNum}`, `scoreboard_tatami_${tatamiNum}`);
    } finally {
      isScoreboardMenuOpen = false;
      hoveredTatami = null;
    }
  }

  onMount(() => {
    refreshRunningScoreboards();
    let unlisten: any;
    listen("scoreboard_status_changed", () => {
      refreshRunningScoreboards();
    }).then((u) => (unlisten = u));

    return () => {
      if (unlisten) unlisten();
      if (closeMenuTimer) clearTimeout(closeMenuTimer);
    };
  });

  // Horizontal resizer state (resizable channel bar)
  let sidebarWidth = $state(260);
  let isResizing = $state(false);

  onMount(() => {
    try {
      const saved = localStorage.getItem("ambrilindo_sidebar_width");
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!isNaN(parsed) && parsed >= 200 && parsed <= 550) {
          sidebarWidth = parsed;
        }
      }
    } catch (_) {}
  });

  function startResizing(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    const onMouseMove = (moveEvent: MouseEvent) => {
      // Leftmost rail width is 72px
      const newWidth = moveEvent.clientX - 72;
      if (newWidth >= 200 && newWidth <= 550) {
        sidebarWidth = newWidth;
      }
    };

    const onMouseUp = () => {
      isResizing = false;
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      try {
        localStorage.setItem("ambrilindo_sidebar_width", sidebarWidth.toString());
      } catch (_) {}
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  let filteredBagans = $derived(
    bagans.filter((b) =>
      (b.nama_bagan || "").toLowerCase().includes(searchBagan.toLowerCase()) ||
      (b.kode || "").toLowerCase().includes(searchBagan.toLowerCase())
    )
  );

  let activeRoleLabel = $derived.by(() => {
    if (activeRole.startsWith("admin_tatami_")) {
      const num = activeRole.replace("admin_tatami_", "");
      return `Admin Tatami ${num}`;
    }
    return "Admin Utama";
  });
</script>

<aside
  style="width: {sidebarWidth}px;"
  class="no-print bg-[#111216] flex flex-col justify-between shrink-0 select-none z-10 border-r border-white/[0.08] font-sans relative"
>
  <!-- Top Tournament Event Header -->
  <div class="h-14 border-b border-white/[0.08] px-4 flex items-center justify-between shadow-sm">
    <div class="truncate mr-2">
      <h2 class="text-xs font-black tracking-wide text-white uppercase truncate">
        {activeEvent?.nama_event || 'Ambrilindo'}
      </h2>
      <span class="text-[10px] font-mono text-zinc-400 uppercase tracking-wider block">
        {activeEvent?.kode_event || 'LOCAL'} • {tatamis.length} TATAMI
      </span>
    </div>
  </div>

  <!-- Channel Navigation (Scrollable) -->
  <div class="flex-1 overflow-y-auto overflow-x-hidden px-2 py-3 space-y-4 scrollbar-thin">
    <!-- Group 1: Dashboard -->
    <div>
      <button
        onclick={() => (collapseDashboard = !collapseDashboard)}
        class="w-full flex items-center justify-between px-2 py-1 text-[10px] font-mono font-bold tracking-widest text-zinc-500 hover:text-zinc-300 uppercase transition"
      >
        <span>DASHBOARD</span>
        <span class="text-[9px]">{collapseDashboard ? '▶' : '▼'}</span>
      </button>

      {#if !collapseDashboard}
        <div class="mt-1 space-y-0.5">
          <!-- Tatami Manager (CRUD) -->
          <button
            onclick={() =>
              onSelectChannel({
                type: "tatami",
                id: "tatami-manager",
                name: "tatami",
                subtitle: "Manajemen Tatami Turnamen",
              })}
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'tatami' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <div class="flex items-center space-x-2 truncate">
              <span class="text-zinc-500 font-mono">#</span>
              <span class="truncate">tatami</span>
            </div>
            <span class="text-[10px] font-mono text-zinc-500">{tatamis.length}</span>
          </button>

          <!-- Data Atlet -->
          <button
            onclick={() =>
              onSelectChannel({
                type: "roster",
                id: "athletes",
                name: "data-atlet",
                subtitle: "Daftar Atlet & Roster Peserta",
              })}
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'data-atlet' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <span class="text-zinc-500 font-mono">#</span>
            <span>data-atlet</span>
          </button>

          <!-- Nomor Tanding -->
          <button
            onclick={() =>
              onSelectChannel({
                type: "roster",
                id: "categories",
                name: "nomor-tanding",
                subtitle: "Kategori & Nomor Pertandingan",
              })}
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'nomor-tanding' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <span class="text-zinc-500 font-mono">#</span>
            <span>nomor-tanding</span>
          </button>

          <!-- Buka Scoring Board Channel Item -->
          <div
            bind:this={scoreboardBtnEl}
            class="relative"
            role="region"
            aria-label="Menu Scoring Board"
            onmouseenter={() => {
              if (scoreboardBtnEl) openScoreboardMenu(scoreboardBtnEl);
            }}
            onmouseleave={scheduleCloseMenu}
          >
            <button
              type="button"
              onclick={() => {
                if (scoreboardBtnEl) {
                  if (isScoreboardMenuOpen) {
                    isScoreboardMenuOpen = false;
                    hoveredTatami = null;
                  } else {
                    openScoreboardMenu(scoreboardBtnEl);
                  }
                }
              }}
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs font-medium transition text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200"
              title="Buka Layar Scoring Board Arena"
            >
              <div class="flex items-center space-x-2 truncate">
                <span class="text-amber-500 font-mono">#</span>
                <span class="truncate">buka-scoring-board</span>
              </div>
              {#if runningScoreboards.length > 0}
                <span class="flex items-center space-x-1 px-1.5 py-0.5 rounded bg-red-500/20 border border-red-500/40 text-[9px] font-mono font-bold text-red-400 shadow-sm">
                  <span class="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse"></span>
                  <span>{runningScoreboards.length} AKTIF</span>
                </span>
              {/if}
            </button>

            <!-- Flyout Menu Popover (Fixed to viewport with anti-flicker bridge) -->
            {#if isScoreboardMenuOpen}
              <div
                role="region"
                aria-label="Menu Pilihan Tatami dan Monitor"
                style="top: {scoreboardFlyoutPos.top}px; left: {scoreboardFlyoutPos.left}px;"
                onmouseenter={cancelCloseMenu}
                onmouseleave={scheduleCloseMenu}
                class="fixed z-50 bg-[#181a20] border border-[#2b2d31] rounded-xl shadow-2xl p-2 w-64 text-xs animate-in fade-in zoom-in-95 duration-100 before:content-[''] before:absolute before:-left-3 before:top-0 before:w-4 before:h-full"
              >
                {#if activeRole === "admin_utama"}
                  <div class="px-2 py-1 text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-500 border-b border-[#2b2d31] mb-1 flex items-center justify-between">
                    <span>PILIH TATAMI</span>
                    {#if runningScoreboards.length > 0}
                      <span class="text-[9px] text-red-400 font-bold">{runningScoreboards.length} Berjalan</span>
                    {/if}
                  </div>
                  <div class="space-y-0.5">
                    {#each (tatamis.length > 0 ? tatamis : [{ id: 1, tatami_number: 1 }]) as tItem}
                      {@const tNum = tItem.tatami_number || 1}
                      {@const isRunning = runningScoreboards.includes(tNum)}
                      <div
                        class="relative"
                        role="menuitem"
                        tabindex="-1"
                        onmouseenter={() => (hoveredTatami = tNum)}
                      >
                        <div class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg font-bold transition {hoveredTatami === tNum ? 'bg-[#5865f2] text-white shadow' : 'text-zinc-300 hover:bg-white/[0.05]'}">
                          <button
                            type="button"
                            class="flex-1 text-left flex items-center space-x-1.5 min-w-0"
                            onclick={() => (hoveredTatami = tNum)}
                          >
                            <span>Tatami {tNum}</span>
                            <span class="text-[10px] font-mono">›</span>
                          </button>

                          <!-- Red Stop Logo & Badge if running -->
                          {#if isRunning}
                            <button
                              type="button"
                              onclick={(e) => {
                                e.stopPropagation();
                                handleStopScoreboard(tNum);
                              }}
                              class="px-2 py-0.5 rounded bg-red-600 hover:bg-red-500 text-white font-mono text-[9px] font-black tracking-wider flex items-center space-x-1 shadow transition active:scale-95 shrink-0 ml-1.5"
                              title="Hentikan & Tutup Scoreboard Tatami {tNum}"
                            >
                              <span class="w-1.5 h-1.5 bg-white rounded-xs"></span>
                              <span>STOP</span>
                            </button>
                          {/if}
                        </div>

                        <!-- Nested Monitors Menu -->
                        {#if hoveredTatami === tNum}
                          <div class="absolute left-full top-0 ml-1 z-50 bg-[#14161b] border border-[#2b2d31] rounded-xl shadow-2xl p-2 w-56 text-xs before:content-[''] before:absolute before:-left-3 before:top-0 before:w-4 before:h-full">
                            <div class="px-2 py-1 text-[10px] font-mono font-bold uppercase tracking-wider text-amber-400 border-b border-[#2b2d31] mb-1 flex items-center justify-between">
                              <span>Tatami {tNum} - Pilih Monitor:</span>
                              {#if isRunning}
                                <span class="text-[9px] text-red-400 font-bold">AKTIF</span>
                              {/if}
                            </div>
                            {#if monitors.length === 0}
                              <div class="px-2 py-2 text-zinc-500 text-[11px]">Memuat monitor...</div>
                            {:else}
                              {#each monitors as mon}
                                <button
                                  type="button"
                                  onclick={() => handleLaunchScoreboard(tNum, mon.id)}
                                  class="w-full text-left px-2 py-1.5 rounded-lg text-zinc-200 hover:bg-[#5865f2] hover:text-white transition flex flex-col mb-0.5"
                                >
                                  <span class="font-bold">{mon.name} {mon.is_primary ? '(Utama)' : '(Eksternal)'}</span>
                                  <span class="text-[10px] font-mono text-zinc-400">{mon.width}x{mon.height}</span>
                                </button>
                              {/each}
                            {/if}
                          </div>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {:else}
                  <!-- Admin Tatami X: Tatami automatically locked, only select monitor -->
                  {@const assignedTatamiNum = parseInt(activeRole.replace("admin_tatami_", "")) || 1}
                  {@const isRunning = runningScoreboards.includes(assignedTatamiNum)}
                  <div class="px-2 py-1 text-[10px] font-mono font-bold uppercase tracking-wider text-amber-400 border-b border-[#2b2d31] mb-1 flex items-center justify-between">
                    <span>Tatami {assignedTatamiNum} - Monitor</span>
                    {#if isRunning}
                      <button
                        type="button"
                        onclick={() => handleStopScoreboard(assignedTatamiNum)}
                        class="px-2 py-0.5 rounded bg-red-600 hover:bg-red-500 text-white font-mono text-[9px] font-black tracking-wider flex items-center space-x-1 shadow transition active:scale-95"
                        title="Hentikan & Tutup Scoreboard Tatami {assignedTatamiNum}"
                      >
                        <span class="w-1.5 h-1.5 bg-white rounded-xs"></span>
                        <span>STOP</span>
                      </button>
                    {/if}
                  </div>
                  {#if monitors.length === 0}
                    <div class="px-2 py-2 text-zinc-500 text-[11px]">Memuat monitor...</div>
                  {:else}
                    {#each monitors as mon}
                      <button
                        type="button"
                        onclick={() => handleLaunchScoreboard(assignedTatamiNum, mon.id)}
                        class="w-full text-left px-2 py-1.5 rounded-lg text-zinc-200 hover:bg-[#5865f2] hover:text-white transition flex flex-col mb-0.5"
                      >
                        <span class="font-bold">{mon.name} {mon.is_primary ? '(Utama)' : '(Eksternal)'}</span>
                        <span class="text-[10px] font-mono text-zinc-400">{mon.width}x{mon.height}</span>
                      </button>
                    {/each}
                  {/if}
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <!-- Group 2: Rekapan & Summary -->
    <div>
      <button
        onclick={() => (collapseRekapan = !collapseRekapan)}
        class="w-full flex items-center justify-between px-2 py-1 text-[10px] font-mono font-bold tracking-widest text-zinc-500 hover:text-zinc-300 uppercase transition"
      >
        <span>REKAPAN & SUMMARY</span>
        <span class="text-[9px]">{collapseRekapan ? '▶' : '▼'}</span>
      </button>

      {#if !collapseRekapan}
        <div class="mt-1 space-y-0.5">
          <button
            onclick={() =>
              onSelectChannel({
                type: "contingent",
                id: "contingents",
                name: "kontingen-utusan",
                subtitle: "Daftar Kontingen & Perguruan Peserta",
              })}
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'kontingen-utusan' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <span class="text-zinc-500 font-mono">#</span>
            <span>kontingen-utusan</span>
          </button>

          <button
            onclick={() =>
              onSelectChannel({
                type: "summary_juara",
                id: "summary-juara",
                name: "summary-juara",
                subtitle: "Perhitungan Medali Juara Umum (Kontingen & Perguruan)",
              })}
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'summary-juara' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <span class="text-zinc-500 font-mono">#</span>
            <span>summary-juara</span>
          </button>
        </div>
      {/if}
    </div>

    <!-- Group 3: Admin Tools -->
    <div>
      <button
        onclick={() => (collapseAdminTools = !collapseAdminTools)}
        class="w-full flex items-center justify-between px-2 py-1 text-[10px] font-mono font-bold tracking-widest text-zinc-500 hover:text-zinc-300 uppercase transition"
      >
        <span>ADMIN TOOLS</span>
        <span class="text-[9px]">{collapseAdminTools ? '▶' : '▼'}</span>
      </button>

      {#if !collapseAdminTools}
        <div class="mt-1 space-y-0.5">
          <button
            onclick={() =>
              onSelectChannel({
                type: "sync",
                id: "cloud-sync",
                name: "sinkronisasi-web",
                subtitle: "Integrasi Live Sync Portal Publik ambrilindo.com",
              })}
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'sinkronisasi-web' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <span class="text-zinc-500 font-mono">#</span>
            <span>sinkronisasi-web</span>
          </button>

          <!-- Data Master & Reset -->
          <button
            onclick={() =>
              onSelectChannel({
                type: "master",
                id: "master-data",
                name: "data-master",
                subtitle: "Reset Bagan & Manajemen Data Master",
              })}
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition {activeChannel.name === 'data-master' ? 'bg-white/[0.08] text-white font-bold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200'}"
          >
            <span class="text-zinc-500 font-mono">#</span>
            <span>data-master</span>
          </button>
        </div>
      {/if}
    </div>

    <!-- Group 4: Bagan Kategori ({bagans.length}) -->
    <div>
      <div class="flex items-center justify-between px-2 py-1 text-[10px] font-mono font-bold tracking-widest text-zinc-500 uppercase transition">
        <button
          onclick={() => (collapseBagan = !collapseBagan)}
          class="flex items-center space-x-1.5 hover:text-zinc-300 uppercase transition"
        >
          <span>BAGAN KATEGORI ({bagans.length})</span>
          <span class="text-[9px]">{collapseBagan ? '▶' : '▼'}</span>
        </button>

        <button
          type="button"
          onclick={(e) => {
            e.stopPropagation();
            onOpenDrawingModal?.();
          }}
          title="Buka Drawing Bagan Otomatis"
          class="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold text-amber-400 hover:text-amber-300 hover:bg-amber-400/10 border border-amber-500/20 transition flex items-center space-x-1"
        >
          <span>+ DRAWING</span>
        </button>
      </div>

      {#if !collapseBagan}
        {#if bagans.length > 5}
          <div class="px-2 my-1">
            <input
              type="text"
              bind:value={searchBagan}
              placeholder="Cari kategori bagan..."
              class="w-full bg-black/40 text-zinc-200 text-[11px] px-2.5 py-1.5 rounded-lg border border-white/[0.08] focus:border-white/[0.2] outline-none"
            />
          </div>
        {/if}

        <!-- Expandable without nested scroll: flows naturally with sidebar -->
        <div class="mt-1 space-y-0.5">
          {#if filteredBagans.length > 0}
            {#each filteredBagans as bagan}
              {@const isKata = String(bagan.tipe_tanding) === '1'}
              {@const isActive = activeChannel.type === 'bagan' && activeChannel.id === bagan.id}
              {@const baganCode = bagan.kode || String(bagan.id).padStart(3, '0')}
              <button
                onclick={() =>
                  onSelectChannel({
                    type: "bagan",
                    id: bagan.id,
                    name: bagan.nama_bagan || `bagan-${bagan.id}`,
                    subtitle: `Bagan [${baganCode}]: ${bagan.nama_bagan || ''}`,
                  })}
                title="[{baganCode}] {bagan.nama_bagan || `bagan-${bagan.id}`}"
                class="w-full flex items-start space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition text-left group {isActive ? (isKata ? 'bg-sky-500/10 text-sky-200 font-semibold border border-sky-500/20 shadow-sm' : 'bg-amber-500/10 text-amber-200 font-semibold border border-amber-500/20 shadow-sm') : 'hover:bg-white/[0.04]'}"
              >
                <!-- Monospace Bagan Code Badge replacing '#' -->
                <span class="font-mono text-[10px] font-bold px-1.5 py-0.5 rounded tracking-wider shrink-0 mt-0.5 {isKata ? 'bg-sky-950/50 text-sky-400/80 border border-sky-500/20' : 'bg-amber-950/50 text-amber-400/80 border border-amber-500/20'}">
                  {baganCode}
                </span>

                <!-- Multi-line title wrap with toned-down, eye-friendly soft colors -->
                <span class="line-clamp-2 leading-snug break-words flex-1 {isActive ? (isKata ? 'text-sky-200' : 'text-amber-200') : (isKata ? 'text-sky-300/70 group-hover:text-sky-200' : 'text-amber-300/70 group-hover:text-amber-200')}">
                  {bagan.nama_bagan || `bagan-${bagan.id}`}
                </span>
              </button>
            {/each}
          {:else}
            <div class="px-3 py-2 text-[11px] text-zinc-500 italic">
              {bagans.length === 0 ? 'Belum ada bagan' : 'Bagan tidak ditemukan'}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <!-- Bottom User Profile & Role Selector (Discord Bottom-Left 56px) -->
  <div class="h-14 bg-[#0d0e12] px-3 flex items-center justify-between border-t border-white/[0.08] relative">
    <!-- Role Selector Dropdown Trigger -->
    <div class="relative flex-1 min-w-0 mr-1">
      <button
        onclick={() => (isRoleDropdownOpen = !isRoleDropdownOpen)}
        class="w-full flex items-center space-x-2 text-left p-1 rounded-lg hover:bg-white/[0.04] transition group"
      >
        <div class="relative shrink-0">
          <div class="w-7 h-7 rounded-lg bg-white/[0.08] border border-white/[0.1] flex items-center justify-center font-mono font-bold text-[10px] text-zinc-300">
            {activeRole.startsWith('admin_tatami_') ? `T${activeRole.replace('admin_tatami_', '')}` : 'AU'}
          </div>
          <div class="absolute -bottom-0.5 -right-0.5 w-2 h-2 rounded-full bg-emerald-500 border border-[#0d0e12]"></div>
        </div>
        <div class="truncate flex-1 min-w-0">
          <p class="text-xs font-bold text-white truncate leading-tight group-hover:text-zinc-200">
            {activeRoleLabel}
          </p>
          <p class="text-[10px] font-mono text-zinc-500 truncate leading-tight flex items-center space-x-1">
            <span>Ganti Role</span>
            <span class="text-[8px]">▼</span>
          </p>
        </div>
      </button>

      <!-- Role Dropdown Menu -->
      {#if isRoleDropdownOpen}
        <!-- Backdrop to close on click outside -->
        <button
          type="button"
          class="fixed inset-0 z-40 bg-black/30 backdrop-blur-[0.5px] cursor-default"
          onclick={() => (isRoleDropdownOpen = false)}
          tabindex="-1"
          aria-label="Tutup menu role"
        ></button>

        <div
          class="absolute bottom-16 left-0 w-64 bg-[#181a20] border border-white/[0.14] rounded-xl shadow-2xl p-1.5 space-y-1 z-50 animate-in fade-in zoom-in-95 duration-100"
        >
          <div class="px-2.5 py-1.5 text-[10px] font-mono font-bold text-zinc-400 uppercase border-b border-white/[0.08] mb-1 flex items-center justify-between">
            <span>PILIH ROLE</span>
            <span class="text-[9px] text-zinc-500 font-normal">ESC untuk tutup</span>
          </div>

          <!-- Option: Admin Utama -->
          <button
            onclick={() => {
              onSelectRole("admin_utama");
              isRoleDropdownOpen = false;
            }}
            class="w-full text-left px-2.5 py-2 rounded-lg text-xs font-medium transition flex items-center justify-between {activeRole === 'admin_utama' ? 'bg-white text-black font-bold shadow' : 'text-zinc-300 hover:bg-white/[0.06] hover:text-white'}"
          >
            <div class="flex items-center space-x-2">
              <span class="w-2 h-2 rounded-full {activeRole === 'admin_utama' ? 'bg-black' : 'bg-emerald-400'}"></span>
              <span>Admin Utama (Semua Tatami)</span>
            </div>
            {#if activeRole === 'admin_utama'}
              <span class="text-[10px] font-mono font-bold uppercase">Aktif</span>
            {/if}
          </button>

          <!-- Option per Tatami -->
          {#if tatamis.length > 0}
            <div class="pt-1 border-t border-white/[0.06] mt-1 space-y-0.5">
              <span class="px-2.5 py-1 text-[9px] font-mono uppercase text-zinc-500 block">
                ADMIN TATAMI SPESIFIK
              </span>
              {#each tatamis as tatami}
                {@const roleKey = `admin_tatami_${tatami.tatami_number || 1}`}
                <button
                  onclick={() => {
                    onSelectRole(roleKey);
                    isRoleDropdownOpen = false;
                  }}
                  class="w-full text-left px-2.5 py-2 rounded-lg text-xs font-medium transition flex items-center justify-between {activeRole === roleKey ? 'bg-white text-black font-bold shadow' : 'text-zinc-300 hover:bg-white/[0.06] hover:text-white'}"
                >
                  <div class="flex items-center space-x-2">
                    <span class="w-2 h-2 rounded-full {activeRole === roleKey ? 'bg-black' : 'bg-sky-400'}"></span>
                    <span>Admin Tatami {tatami.tatami_number}</span>
                  </div>
                  {#if activeRole === roleKey}
                    <span class="text-[10px] font-mono font-bold uppercase">Aktif</span>
                  {/if}
                </button>
              {/each}
            </div>
          {:else}
            <div class="px-2.5 py-2 text-[11px] text-zinc-500 italic">
              Belum ada tatami terdaftar di event ini
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Quick Tool Buttons -->
    <div class="flex items-center space-x-1 shrink-0">
      <button
        onclick={onOpenQrModal}
        class="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.06] transition"
        title="QR Code Tablet"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <rect x="3" y="3" width="7" height="7"/>
          <rect x="14" y="3" width="7" height="7"/>
          <rect x="3" y="14" width="7" height="7"/>
          <rect x="14" y="14" width="7" height="7"/>
        </svg>
      </button>

      <button
        onclick={onReturnToLanding}
        class="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.06] transition"
        title="Keluar / Ganti Mode"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 9V5.25A2.25 2.25 0 0013.5 3h-6a2.25 2.25 0 00-2.25 2.25v13.5A2.25 2.25 0 007.5 21h6a2.25 2.25 0 002.25-2.25V15m3 0l3-3m0 0l-3-3m3 3H9" />
        </svg>
      </button>
    </div>
  </div>

  <!-- Draggable Horizontal Resize Handle -->
  <button
    type="button"
    onmousedown={startResizing}
    class="absolute top-0 -right-1 w-2.5 h-full cursor-col-resize hover:bg-sky-500/40 active:bg-sky-500 transition-colors z-30 group p-0 border-0 bg-transparent focus:outline-none"
    aria-label="Resize channel sidebar"
  >
    <div class="w-[2px] h-full mx-auto {isResizing ? 'bg-sky-400' : 'group-hover:bg-sky-400/80'} transition-colors"></div>
  </button>
</aside>
