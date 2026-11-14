<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, withHostParam } from "$lib/api_client";
  import { listen } from "@tauri-apps/api/event";
  import type { ActiveChannel, AppMode, BaganItem, EventItem, FestivalFormat, FestivalFormatInfo, TatamiItem } from "$lib/types";
  import { initLiveSync, getLiveTatamisForBagan, subscribeLiveSync } from "$lib/liveSync";
  import { hasModule } from "$lib/license.svelte";

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
    onOpenDrawingFestivalModal?: () => void;
    /** Festival format switched (bagans may have been deleted): reload the event. */
    onFestivalFormatChanged?: (format: FestivalFormat) => void;
    onNotify?: (msg: string) => void;
    onReturnToLanding: () => void;
    /** Event switcher (replaces the old event rail). */
    events?: EventItem[];
    onSelectEvent?: (eventId: number) => void;
    onAddEvent?: () => void;
  }

  let {
    events = [],
    onSelectEvent,
    onAddEvent,
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
    onOpenDrawingFestivalModal,
    onFestivalFormatChanged,
    onNotify,
    onReturnToLanding,
  }: Props = $props();

  // --- Festival format (one per event): roster 1-vs-1 OR bagan with groups of 4 ---
  let festivalFormat = $state<FestivalFormatInfo | null>(null);
  let festivalSwitchConfirm = $state(false);
  let isBaganFormat = $derived(festivalFormat?.format !== "roster");
  let isSwitchingFestival = $state(false);

  async function loadFestivalFormat(eventId: number) {
    try {
      const info = await invoke<FestivalFormatInfo>("get_festival_format", { eventId });
      if (activeEvent?.id === eventId) festivalFormat = info;
    } catch (err) {
      console.warn("Gagal memuat format festival:", err);
      festivalFormat = null;
    }
  }

  // Reload when the event changes and when festival bagans appear/disappear (drawing, delete).
  $effect(() => {
    const id = activeEvent?.id;
    void festivalBagans.length;
    if (id) loadFestivalFormat(id);
    else festivalFormat = null;
  });

  async function applyFestivalFormat(format: FestivalFormat, deleteBagans: boolean) {
    if (!activeEvent || isSwitchingFestival) return;
    isSwitchingFestival = true;
    try {
      festivalFormat = await invoke<FestivalFormatInfo>("set_festival_format", {
        eventId: activeEvent.id,
        format,
        deleteBagans,
      });
      festivalSwitchConfirm = false;
      onFestivalFormatChanged?.(format);
      onNotify?.(format === "roster" ? "Festival memakai format Roster." : "Festival memakai format Bagan Grup.");
    } catch (err: any) {
      onNotify?.(`Gagal mengganti format festival: ${err?.message || err}`);
    } finally {
      isSwitchingFestival = false;
    }
  }

  function requestFestivalFormat(format: FestivalFormat) {
    if (!festivalFormat || festivalFormat.format === format) return;
    if (format === "roster" && festivalFormat.festival_bagan_count > 0) {
      festivalSwitchConfirm = true;
      return;
    }
    applyFestivalFormat(format, false);
  }

  // Live synchronization state version for reactivity
  let liveSyncVersion = $state(0);

  // ---------------------------------------------------------------------------
  // Navigation groups (labels + icons; matched to the active channel by type)
  // ---------------------------------------------------------------------------
  type NavItem = { type: ActiveChannel["type"]; id: string; name: string; subtitle: string; icon: string };
  const ICONS = {
    tatami: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
    tv: "M3 5h18v12H3zM8 21h8M12 17v4",
    users: "M16 21v-2a4 4 0 00-4-4H6a4 4 0 00-4 4v2M9 11a4 4 0 100-8 4 4 0 000 8zM22 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75",
    flag: "M4 22V4M4 4h13l-2 4 2 4H4",
    school: "M3 21h18M5 21V8l7-5 7 5v13M9 21v-6h6v6",
    list: "M10 6h11M10 12h11M10 18h11M4 6h1M4 12h1M4 18h1",
    calendar: "M3 5h18v16H3zM16 3v4M8 3v4M3 10h18",
    referee: "M16 11l2 2 4-4M2 21v-2a4 4 0 014-4h6a4 4 0 014 4v2M9 11a4 4 0 100-8 4 4 0 000 8z",
    trophy: "M8 21h8M12 17v4M7 4h10v5a5 5 0 01-10 0V4zM17 5h3v2a3 3 0 01-3 3M7 5H4v2a3 3 0 003 3",
    medal: "M12 15a6 6 0 100-12 6 6 0 000 12zM8.2 13.9L7 22l5-3 5 3-1.2-8.1",
    printer: "M6 9V2h12v7M6 18H4a2 2 0 01-2-2v-5a2 2 0 012-2h16a2 2 0 012 2v5a2 2 0 01-2 2h-2M6 14h12v8H6z",
    book: "M4 19.5A2.5 2.5 0 016.5 17H20V2H6.5A2.5 2.5 0 004 4.5v15zM20 17v5H6.5A2.5 2.5 0 014 19.5",
    cloud: "M17.5 19H9a7 7 0 116.71-9h1.79a4.5 4.5 0 010 9z",
    database: "M12 8c4.97 0 9-1.34 9-3s-4.03-3-9-3-9 1.34-9 3 4.03 3 9 3zM3 5v14c0 1.66 4.03 3 9 3s9-1.34 9-3V5M3 12c0 1.66 4.03 3 9 3s9-1.34 9-3",
  } as const;

  const PERSIAPAN: NavItem[] = [
    { type: "roster", id: "athletes", name: "Atlet", subtitle: "Daftar atlet dan roster peserta", icon: ICONS.users },
    { type: "contingent", id: "contingents", name: "Kontingen", subtitle: "Data kontingen dan logo", icon: ICONS.flag },
    { type: "perguruan", id: "perguruans", name: "Perguruan", subtitle: "Data perguruan", icon: ICONS.school },
    { type: "nomor_tanding", id: "categories", name: "Nomor tanding", subtitle: "Kategori dan nomor pertandingan", icon: ICONS.list },
    { type: "timetable", id: "timetable-editor", name: "Jadwal", subtitle: "Jadwal pertandingan", icon: ICONS.calendar },
    { type: "wasit", id: "wasit-manager", name: "Wasit dan juri", subtitle: "Data wasit dan juri", icon: ICONS.referee },
  ];
  const HASIL: NavItem[] = [
    { type: "summary_bagan", id: "summary-bagan", name: "Juara per bagan", subtitle: "Daftar juara seluruh bagan", icon: ICONS.trophy },
    { type: "summary_juara", id: "summary-juara", name: "Juara umum", subtitle: "Perolehan medali kontingen dan perguruan", icon: ICONS.medal },
    { type: "cetak", id: "cetak", name: "Cetak", subtitle: "Buku hasil dan dokumen cetak", icon: ICONS.printer },
  ];
  const PENGATURAN: NavItem[] = [
    { type: "sync", id: "cloud-sync", name: "Sinkronisasi web", subtitle: "Live sync ke ambrilindo.com", icon: ICONS.cloud },
    { type: "master", id: "master-data", name: "Data master", subtitle: "Reset bagan dan data master", icon: ICONS.database },
  ];
  const TATAMI_ITEM: NavItem = { type: "tatami", id: "tatami-manager", name: "Tatami", subtitle: "Arena dan akses venue", icon: ICONS.tatami };

  function selectNav(item: NavItem) {
    onSelectChannel({ type: item.type, id: item.id, name: item.name, subtitle: item.subtitle });
  }

  let liveTatamiCount = $derived(tatamis.filter((t) => !!t.detail_bagan_id).length);

  let collapsed = $state<Record<string, boolean>>({});
  function toggleGroup(key: string) {
    collapsed[key] = !collapsed[key];
  }

  // Event switcher
  let isEventMenuOpen = $state(false);
  function eventInitials(nama: string | null | undefined): string {
    const words = (nama || "").trim().split(/\s+/).filter(Boolean);
    if (words.length === 0) return "EV";
    if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
    return (words[0][0] + words[1][0]).toUpperCase();
  }

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

  function portal(node: HTMLElement) {
    if (typeof document !== "undefined") {
      document.body.appendChild(node);
    }
    return {
      destroy() {
        if (node.parentNode) {
          node.parentNode.removeChild(node);
        }
      },
    };
  }

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
      window.open(withHostParam(`/scoreboard?tatami=${tatamiNum}`), `scoreboard_tatami_${tatamiNum}`);
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

    initLiveSync(hostAddress);
    const unsubLive = subscribeLiveSync(() => {
      liveSyncVersion++;
    });

    return () => {
      if (unlisten) unlisten();
      unsubLive();
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
      // The sidebar starts at the window's left edge (no event rail any more).
      const newWidth = moveEvent.clientX;
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

  // Separate Open / Prestasi bagans and Festival bagans
  let openBagans = $derived(
    bagans.filter(
      (b) => b.id !== 0 && b.kode !== "000" && !(b.nama_bagan || "").toUpperCase().includes("FESTIVAL")
    )
  );

  let festivalBagans = $derived(
    bagans.filter(
      (b) => b.id !== 0 && b.kode !== "000" && (b.nama_bagan || "").toUpperCase().includes("FESTIVAL")
    )
  );

  let filteredOpenBagans = $derived(
    openBagans.filter(
      (b) =>
        (b.nama_bagan || "").toLowerCase().includes(searchBagan.toLowerCase()) ||
        (b.kode || "").toLowerCase().includes(searchBagan.toLowerCase())
    )
  );

  let collapseFestival = $state(false);
  let searchFestival = $state("");

  let filteredFestivalBagans = $derived(
    festivalBagans.filter(
      (b) =>
        (b.nama_bagan || "").toLowerCase().includes(searchFestival.toLowerCase()) ||
        (b.kode || "").toLowerCase().includes(searchFestival.toLowerCase())
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

<!-- The role menu says "ESC untuk tutup"; make it true (also closes the scoreboard flyout). -->
<svelte:window
  onkeydown={(e) => {
    if (e.key !== "Escape") return;
    if (isRoleDropdownOpen) isRoleDropdownOpen = false;
    if (isEventMenuOpen) isEventMenuOpen = false;
    if (isScoreboardMenuOpen) {
      isScoreboardMenuOpen = false;
      hoveredTatami = null;
    }
  }}
/>

<aside
  style="width: {sidebarWidth}px;"
  class="no-print bg-[#111216] flex flex-col justify-between shrink-0 select-none z-30 border-r border-white/[0.08] font-sans relative"
>
  <!-- Event switcher -->
  <div class="relative h-14 border-b border-white/[0.08] px-2 flex items-center">
    <button
      type="button"
      onclick={() => (isEventMenuOpen = !isEventMenuOpen)}
      aria-haspopup="menu"
      aria-expanded={isEventMenuOpen}
      class="w-full flex items-center gap-2.5 px-2 py-1.5 rounded-lg hover:bg-white/[0.05] transition text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
    >
      <span class="w-8 h-8 rounded-lg bg-red-600/15 border border-red-500/30 text-red-300 text-[11px] font-mono font-bold flex items-center justify-center shrink-0">
        {eventInitials(activeEvent?.nama_event)}
      </span>
      <span class="flex-1 min-w-0">
        <span class="block text-[13px] font-bold text-white truncate" title={activeEvent?.nama_event || ""}>{activeEvent?.nama_event || "Pilih event"}</span>
        <span class="block text-[11px] text-zinc-500 truncate">{activeEvent?.kode_event || "Lokal"} · {tatamis.length} tatami</span>
      </span>
      <svg class="w-4 h-4 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M8 9l4-4 4 4M16 15l-4 4-4-4" /></svg>
    </button>

    {#if isEventMenuOpen}
      <button type="button" class="fixed inset-0 z-40 cursor-default" onclick={() => (isEventMenuOpen = false)} tabindex="-1" aria-label="Tutup menu event"></button>
      <div role="menu" class="absolute top-[52px] left-2 right-2 z-50 bg-[#181a20] border border-white/[0.14] rounded-xl shadow-2xl p-1.5 max-h-[70vh] overflow-y-auto">
        <p class="px-2.5 pt-1 pb-1.5 text-[11px] font-semibold text-zinc-500">Event</p>
        {#each events as ev (ev.id)}
          {@const current = ev.id === activeEvent?.id}
          <button
            type="button"
            role="menuitemradio"
            aria-checked={current}
            onclick={() => {
              isEventMenuOpen = false;
              if (!current) onSelectEvent?.(ev.id);
            }}
            class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-left text-[13px] transition {current ? 'bg-white/[0.08] text-white font-semibold' : 'text-zinc-300 hover:bg-white/[0.05] hover:text-white'}"
          >
            <span class="w-6 h-6 rounded-md bg-white/[0.06] border border-white/[0.08] text-[10px] font-mono font-bold text-zinc-300 flex items-center justify-center shrink-0">{eventInitials(ev.nama_event)}</span>
            <span class="flex-1 min-w-0 truncate" title={ev.nama_event || ""}>{ev.nama_event || `Event ${ev.id}`}</span>
            {#if current}
              <svg class="w-4 h-4 text-emerald-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
            {/if}
          </button>
        {:else}
          <p class="px-2.5 py-2 text-[13px] text-zinc-500">Belum ada event</p>
        {/each}
        <div class="border-t border-white/[0.08] mt-1.5 pt-1.5 space-y-0.5">
          {#if onAddEvent}
            <button
              type="button"
              role="menuitem"
              onclick={() => {
                isEventMenuOpen = false;
                onAddEvent?.();
              }}
              class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-left text-[13px] text-zinc-300 hover:bg-white/[0.05] hover:text-white transition"
            >
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14" /></svg>
              Tambah event
            </button>
          {/if}
          <button
            type="button"
            role="menuitem"
            onclick={() => {
              isEventMenuOpen = false;
              onReturnToLanding();
            }}
            class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-left text-[13px] text-zinc-300 hover:bg-white/[0.05] hover:text-white transition"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" /></svg>
            Kembali ke layar awal
          </button>
        </div>
      </div>
    {/if}
  </div>

  {#snippet navIcon(d: string)}
    <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={d} /></svg>
  {/snippet}

  {#snippet navButton(item: NavItem, badge: string = "")}
    {@const active = activeChannel.type === item.type}
    <button
      type="button"
      onclick={() => selectNav(item)}
      aria-current={active ? "page" : undefined}
      class="w-full h-8 flex items-center gap-2.5 px-2.5 rounded-lg text-[13px] transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {active ? 'bg-white/[0.08] text-white font-semibold' : 'text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-100'}"
    >
      {@render navIcon(item.icon)}
      <span class="flex-1 min-w-0 truncate text-left">{item.name}</span>
      {#if badge}<span class="text-[11px] text-zinc-500 shrink-0">{badge}</span>{/if}
    </button>
  {/snippet}

  {#snippet groupHeader(key: string, label: string)}
    <button
      type="button"
      onclick={() => toggleGroup(key)}
      aria-expanded={!collapsed[key]}
      class="w-full flex items-center justify-between px-2.5 py-1 text-[11px] font-semibold text-zinc-500 hover:text-zinc-300 transition rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
    >
      <span>{label}</span>
      <svg class="w-3.5 h-3.5 transition-transform {collapsed[key] ? '-rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" /></svg>
    </button>
  {/snippet}

  <!-- Channel Navigation (Scrollable) -->
  <div class="flex-1 overflow-y-auto overflow-x-hidden px-2 py-3 space-y-4 scrollbar-thin">
    <!-- Pertandingan -->
    <div>
      {@render groupHeader("pertandingan", "Pertandingan")}
      {#if !collapsed["pertandingan"]}
        <div class="mt-1 space-y-0.5">
          {@render navButton(TATAMI_ITEM, liveTatamiCount > 0 ? `${liveTatamiCount} live` : `${tatamis.length}`)}

          <!-- Scoreboard (flyout: pick tatami, then monitor) -->
          <div
            bind:this={scoreboardBtnEl}
            class="relative"
            role="region"
            aria-label="Menu scoreboard"
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
              aria-haspopup="menu"
              aria-expanded={isScoreboardMenuOpen}
              class="w-full h-8 flex items-center gap-2.5 px-2.5 rounded-lg text-[13px] transition text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
            >
              {@render navIcon(ICONS.tv)}
              <span class="flex-1 min-w-0 truncate text-left">Scoreboard</span>
              {#if runningScoreboards.length > 0}
                <span class="flex items-center gap-1 text-[11px] text-red-300 shrink-0">
                  <span class="w-1.5 h-1.5 rounded-full bg-red-500" aria-hidden="true"></span>
                  {runningScoreboards.length} tampil
                </span>
              {:else}
                <svg class="w-3.5 h-3.5 text-zinc-600 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
              {/if}
            </button>

            <!-- Flyout Menu Popover (Fixed to viewport with anti-flicker bridge) -->
            {#if isScoreboardMenuOpen}
              <div
                use:portal
                role="region"
                aria-label="Menu Pilihan Tatami dan Monitor"
                style="top: {scoreboardFlyoutPos.top}px; left: {scoreboardFlyoutPos.left}px;"
                onmouseenter={cancelCloseMenu}
                onmouseleave={scheduleCloseMenu}
                class="fixed z-[9999] bg-[#181a20] border border-[#2b2d31] rounded-xl shadow-2xl p-2 w-64 text-xs animate-in fade-in zoom-in-95 duration-100 before:content-[''] before:absolute before:-left-3 before:top-0 before:w-4 before:h-full"
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

    <!-- Persiapan -->
    <div>
      {@render groupHeader("persiapan", "Persiapan")}
      {#if !collapsed["persiapan"]}
        <div class="mt-1 space-y-0.5">
          {#each PERSIAPAN as item (item.id)}
            {@render navButton(item)}
          {/each}
        </div>
      {/if}
    </div>

    <!-- Hasil -->
    <div>
      {@render groupHeader("hasil", "Hasil")}
      {#if !collapsed["hasil"]}
        <div class="mt-1 space-y-0.5">
          {#each HASIL as item (item.id)}
            {@render navButton(item)}
          {/each}
        </div>
      {/if}
    </div>

    <!-- Pengaturan -->
    <div>
      {@render groupHeader("pengaturan", "Pengaturan")}
      {#if !collapsed["pengaturan"]}
        <div class="mt-1 space-y-0.5">
          {#each PENGATURAN as item (item.id)}
            {@render navButton(item)}
          {/each}
        </div>
      {/if}
    </div>

    <!-- Group 4: Bagan Kategori ({openBagans.length}) -->
    <div>
      <div class="flex items-center justify-between gap-2 px-2.5 py-1">
        <button
          type="button"
          onclick={() => (collapseBagan = !collapseBagan)}
          aria-expanded={!collapseBagan}
          class="flex items-center gap-1.5 text-[11px] font-semibold text-zinc-500 hover:text-zinc-300 transition rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        >
          <span>Bagan ({openBagans.length})</span>
          <svg class="w-3.5 h-3.5 transition-transform {collapseBagan ? '-rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" /></svg>
        </button>

        <button
          type="button"
          onclick={(e) => {
            e.stopPropagation();
            onOpenDrawingModal?.();
          }}
          title="Drawing bagan prestasi otomatis"
          class="h-7 px-2 rounded-lg text-[11px] font-semibold text-amber-300 hover:text-amber-200 hover:bg-amber-400/10 border border-amber-500/25 transition flex items-center gap-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-300/50"
        >
          {#if !hasModule("drawing")}
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-label="Terkunci"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 018 0v4" /></svg>
          {:else}
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14" /></svg>
          {/if}
          <span>Drawing</span>
        </button>
      </div>

      {#if !collapseBagan}
        {#if openBagans.length > 5}
          <div class="px-2 my-1">
            <input
              type="text"
              bind:value={searchBagan}
              placeholder="Cari bagan atau kode"
              aria-label="Cari bagan"
              class="w-full bg-black/40 text-zinc-200 text-[13px] px-2.5 py-1.5 rounded-lg border border-white/[0.08] focus:border-white/[0.3] outline-none select-text"
            />
          </div>
        {/if}

        <div class="mt-1 space-y-0.5">
          {#if filteredOpenBagans.length > 0}
            {#each filteredOpenBagans as bagan}
              {@const isKata = String(bagan.tipe_tanding) === '1'}
              {@const isActive = activeChannel.type === 'bagan' && activeChannel.id === bagan.id}
              {@const baganCode = bagan.kode || String(bagan.id).padStart(3, '0')}
              {@const runningTatamis = liveSyncVersion >= 0 ? getLiveTatamisForBagan(bagan.id, tatamis) : []}
              <button
                onclick={() =>
                  onSelectChannel({
                    type: "bagan",
                    id: bagan.id,
                    name: bagan.nama_bagan || `bagan-${bagan.id}`,
                    subtitle: `Bagan [${baganCode}]: ${bagan.nama_bagan || ''}`,
                  })}
                title={`[${baganCode}] ${bagan.nama_bagan || `bagan-${bagan.id}`}`}
                class="w-full flex items-start space-x-2 px-2.5 py-1.5 rounded-lg text-[13px] font-medium transition text-left group {isActive ? (isKata ? 'bg-sky-500/10 text-sky-200 font-semibold border border-sky-500/20 shadow-sm' : 'bg-amber-500/10 text-amber-200 font-semibold border border-amber-500/20 shadow-sm') : 'hover:bg-white/[0.04]'}"
              >
                <!-- Monospace Bagan Code Badge -->
                <span class="font-mono text-[10px] font-bold px-1.5 py-0.5 rounded tracking-wider shrink-0 mt-0.5 {isKata ? 'bg-sky-950/50 text-sky-400/80 border border-sky-500/20' : 'bg-amber-950/50 text-amber-400/80 border border-amber-500/20'}">
                  {baganCode}
                </span>

                <div class="flex-1 min-w-0">
                  <span class="line-clamp-2 leading-snug break-words block {isActive ? (isKata ? 'text-sky-200 font-bold' : 'text-amber-200 font-bold') : (isKata ? 'text-sky-300/80 group-hover:text-sky-200' : 'text-amber-300/80 group-hover:text-amber-200')}">
                    {bagan.nama_bagan || `bagan-${bagan.id}`}
                  </span>
                  <div class="mt-0.5 flex items-center gap-1.5 flex-wrap text-[11px]">
                    <span class="text-zinc-500">{isKata ? "Kata" : "Kumite"}</span>
                    {#each runningTatamis as tNum}
                      <span class="inline-flex items-center gap-1 text-amber-300 font-semibold">
                        <span class="w-1.5 h-1.5 rounded-full bg-amber-400" aria-hidden="true"></span>
                        Tatami {tNum}
                      </span>
                    {/each}
                  </div>
                </div>
              </button>
            {/each}
          {:else}
            <div class="px-3 py-2 text-[13px] text-zinc-500">
              {openBagans.length === 0 ? 'Belum ada bagan. Klik Drawing untuk membuat.' : 'Bagan tidak ditemukan'}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Group 5: Festival (one format per event: Roster or Bagan Grup) -->
    <div class="pt-2 border-t border-white/[0.06]">
      <div class="flex items-center justify-between gap-2 px-2.5 py-1">
        <button
          type="button"
          onclick={() => (collapseFestival = !collapseFestival)}
          aria-expanded={!collapseFestival}
          class="flex items-center gap-1.5 text-[11px] font-semibold text-emerald-400/90 hover:text-emerald-300 transition rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-300/50"
        >
          <span class="whitespace-nowrap">Festival{isBaganFormat ? ` (${festivalBagans.length})` : ""}</span>
          <svg class="w-3.5 h-3.5 transition-transform {collapseFestival ? '-rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" /></svg>
        </button>

        {#if festivalFormat}
          <div
            class="flex items-center rounded-md border border-emerald-500/30 overflow-hidden shrink-0"
            role="radiogroup"
            aria-label="Format festival"
            title={activeRole === "admin_utama" ? "Format festival event ini" : "Format festival hanya bisa diganti Admin Utama"}
          >
            {#each [["roster", "Roster"], ["bagan", "Bagan Grup"]] as [value, label]}
              {@const selected = festivalFormat.format === value}
              <button
                type="button"
                role="radio"
                aria-checked={selected}
                disabled={activeRole !== "admin_utama" || isSwitchingFestival}
                onclick={() => requestFestivalFormat(value as FestivalFormat)}
                class="px-1.5 py-0.5 text-[9px] font-mono font-bold transition disabled:cursor-default {selected ? 'bg-emerald-500/20 text-emerald-200' : 'text-zinc-500 hover:text-emerald-300 disabled:hover:text-zinc-500'}"
              >
                {label}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      {#if !collapseFestival && !isBaganFormat}
        <div class="mt-1 space-y-0.5">
          <button
            onclick={() =>
              onSelectChannel({
                type: "festival",
                id: "festival-roster",
                name: "Roster festival",
                subtitle: "Roster dan auto-pairing festival",
              })}
            class="w-full h-8 flex items-center gap-2.5 px-2.5 rounded-lg text-[13px] transition {activeChannel.type === 'festival' ? 'bg-emerald-500/15 text-emerald-200 font-semibold' : 'text-zinc-400 hover:bg-emerald-500/[0.06] hover:text-zinc-100'}"
          >
            {@render navIcon(ICONS.users)}
            <span>Roster festival</span>
          </button>
        </div>
      {/if}

      {#if !collapseFestival && isBaganFormat}
        <div class="px-2 mt-1">
          <button
            type="button"
            onclick={() => onOpenDrawingFestivalModal?.()}
            title="Drawing bagan festival otomatis (4 atlet per grup)"
            class="w-full px-2 py-1 rounded-lg text-[10px] font-mono font-bold text-emerald-400 hover:text-emerald-300 hover:bg-emerald-400/10 border border-emerald-500/30 transition inline-flex items-center justify-center gap-1"
          >
            {#if !hasModule("drawing")}
              <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-label="Terkunci"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 018 0v4" /></svg>
            {/if}
            + DRAWING
          </button>
        </div>
        {#if festivalBagans.length > 5}
          <div class="px-2 my-1">
            <input
              type="text"
              bind:value={searchFestival}
              placeholder="Cari kategori festival..."
              class="w-full bg-black/40 text-zinc-200 text-[11px] px-2.5 py-1.5 rounded-lg border border-white/[0.08] focus:border-emerald-500/40 outline-none"
            />
          </div>
        {/if}

        <div class="mt-1 space-y-0.5">
          {#if filteredFestivalBagans.length > 0}
            {#each filteredFestivalBagans as bagan}
              {@const isKata = String(bagan.tipe_tanding) === '1'}
              {@const isActive = activeChannel.type === 'bagan' && activeChannel.id === bagan.id}
              {@const baganCode = bagan.kode || String(bagan.id).padStart(3, '0')}
              {@const poolCount = bagan.pool || 1}
              {@const runningTatamis = liveSyncVersion >= 0 ? getLiveTatamisForBagan(bagan.id, tatamis) : []}
              <button
                onclick={() =>
                  onSelectChannel({
                    type: "bagan",
                    id: bagan.id,
                    name: bagan.nama_bagan || `festival-${bagan.id}`,
                    subtitle: `Bagan Festival [${baganCode}]: ${bagan.nama_bagan || ''} (${poolCount} Grup)`,
                  })}
                title={`[${baganCode}] ${bagan.nama_bagan || `festival-${bagan.id}`} (${poolCount} Grup)`}
                class="w-full flex items-start space-x-2 px-2.5 py-1.5 rounded-lg text-xs font-medium transition text-left group {isActive ? 'bg-emerald-500/15 text-emerald-200 font-semibold border border-emerald-500/30 shadow-sm' : 'hover:bg-emerald-500/[0.06] text-zinc-300'}"
              >
                <!-- Monospace Bagan Code Badge -->
                <span class="font-mono text-[10px] font-bold px-1.5 py-0.5 rounded tracking-wider shrink-0 mt-0.5 bg-emerald-950/70 text-emerald-400 border border-emerald-500/30">
                  {baganCode}
                </span>

                <!-- Title & Group Tag -->
                <div class="flex-1 min-w-0">
                  <span class="line-clamp-2 leading-snug break-words block {isActive ? 'text-emerald-200 font-bold' : 'text-emerald-300/80 group-hover:text-emerald-200'}">
                    {bagan.nama_bagan || `festival-${bagan.id}`}
                  </span>
                  <div class="flex items-center gap-1.5 mt-0.5 flex-wrap">
                    <span class="text-[9px] font-mono px-1 py-0.2 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                      {poolCount} Grup
                    </span>
                    <span class="text-[9px] font-mono text-zinc-400">
                      {isKata ? 'Kata' : 'Kumite'}
                    </span>
                    {#each runningTatamis as tNum}
                      <span class="inline-flex items-center gap-1 text-[11px] text-amber-300 font-semibold">
                        <span class="w-1.5 h-1.5 rounded-full bg-amber-400" aria-hidden="true"></span>
                        Tatami {tNum}
                      </span>
                    {/each}
                  </div>
                </div>
              </button>
            {/each}
          {:else}
            <div class="px-3 py-2 text-[11px] text-zinc-500 italic">
              {festivalBagans.length === 0 ? 'Belum ada bagan festival' : 'Bagan festival tidak ditemukan'}
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
        aria-haspopup="menu"
        aria-expanded={isRoleDropdownOpen}
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
          <p class="text-[11px] text-zinc-500 truncate leading-tight flex items-center gap-1">
            <span>Ganti peran</span>
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 15l7-7 7 7" /></svg>
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
        class="p-2 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.06] transition"
        title="QR untuk tablet"
        aria-label="QR untuk tablet"
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
        class="p-2 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.06] transition"
        title="Kembali ke layar awal"
        aria-label="Kembali ke layar awal"
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

{#if festivalSwitchConfirm && festivalFormat}
  <div use:portal class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs" role="dialog" aria-modal="true" aria-labelledby="festival-switch-title">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-md w-full">
      <h3 id="festival-switch-title" class="text-base font-black text-white uppercase tracking-tight">Pakai Format Roster</h3>
      <p class="text-xs text-zinc-300 mt-3 leading-relaxed">
        Event ini masih punya <strong class="text-white">{festivalFormat.festival_bagan_count} bagan festival</strong>{#if festivalFormat.finished_match_count > 0}
          dengan <strong class="text-rose-300">{festivalFormat.finished_match_count} partai yang sudah selesai</strong>{/if}.
        Format Roster tidak memakai bagan, jadi semua bagan festival beserta hasilnya akan dihapus.
      </p>
      <div class="flex items-center justify-end gap-2 mt-6">
        <button
          type="button"
          onclick={() => (festivalSwitchConfirm = false)}
          disabled={isSwitchingFestival}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={() => applyFestivalFormat("roster", true)}
          disabled={isSwitchingFestival}
          class="h-9 px-4 bg-rose-500 hover:bg-rose-400 text-white font-bold text-xs rounded-xl shadow-md disabled:opacity-50"
        >
          {isSwitchingFestival ? "Menghapus..." : "Hapus Bagan & Pakai Roster"}
        </button>
      </div>
    </div>
  </div>
{/if}
